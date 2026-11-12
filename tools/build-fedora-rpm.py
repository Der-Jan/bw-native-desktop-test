#!/usr/bin/env python3
"""Build a native Fedora/Nobara RPM, including generated ELF dependencies."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent
APP = 'bitwarden-desktop-next'
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--skip-build', action='store_true', help='Package release binaries just built from this source tree')
parser.add_argument('--jobs', default='8')
parser.add_argument('--cargo', help='Cargo executable (default: PATH, then target/build-tools/cargo/bin/cargo)')
args = parser.parse_args()
for tool in ('rpm', 'rpmbuild', 'desktop-file-validate', 'strip'):
    if not shutil.which(tool):
        parser.error(f'Missing build tool: {tool}')

def output(*command):
    return subprocess.check_output([str(value) for value in command], cwd=ROOT, text=True).strip()

def run(*command, **kwargs):
    subprocess.run([str(value) for value in command], cwd=ROOT, check=True, **kwargs)

fedora = output('rpm', '-E', '%{fedora}')
arch = output('rpm', '-E', '%{_arch}')
if not fedora.isdigit() or arch != 'x86_64':
    parser.error('This recipe requires an x86_64 Fedora/Nobara build host')
revision = output('git', 'rev-parse', '--short=12', 'HEAD')
epoch = output('git', 'log', '-1', '--format=%ct')
timestamp = datetime.fromtimestamp(int(epoch), timezone.utc).strftime('%Y%m%d%H%M%S')
version = tomllib.loads((ROOT / 'crates/desktop/Cargo.toml').read_text())['package']['version']
dirty = subprocess.run(['git', 'diff', '--quiet', 'HEAD', '--', 'Cargo.toml', 'Cargo.lock', 'crates', 'bitwarden_license', 'vendor'], cwd=ROOT).returncode
release = f'0.{timestamp}.git{revision}' + ('.dirty' if dirty else '')
if not args.skip_build:
    cargo = args.cargo or shutil.which('cargo')
    environment = os.environ.copy()
    if cargo is None:
        cargo = str(ROOT / 'target/build-tools/cargo/bin/cargo')
        if not Path(cargo).exists():
            parser.error('Install a Rust toolchain or specify --cargo')
        environment.setdefault('CARGO_HOME', str(ROOT / 'target/build-tools/cargo'))
        environment.setdefault('RUSTUP_HOME', str(ROOT / 'target/build-tools/rustup'))
    environment.setdefault('CARGO_BUILD_JOBS', args.jobs)
    environment['CARGO_TARGET_DIR'] = str(ROOT / 'target')
    # Local preview builds may have this shim when the xdotool-devel package is absent.
    if (ROOT / 'target/native-libs/libxdo.so').exists():
        shim = str(ROOT / 'target/native-libs')
        environment['LIBRARY_PATH'] = shim + (':' + environment['LIBRARY_PATH'] if environment.get('LIBRARY_PATH') else '')
    run(cargo, 'build', '--release', '--workspace', '--locked', env=environment)
for name in (APP, 'bitwarden-browser-proxy'):
    if not (ROOT / 'target/release' / name).is_file():
        parser.error(f'Missing target/release/{name}')
if output('git', 'rev-parse', '--short=12', 'HEAD') != revision:
    parser.error('HEAD changed while building; rebuild before packaging')

out = ROOT / f'dist/fedora-{fedora}'
out.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix='rpm-', dir=ROOT / 'target') as temporary:
    top = Path(temporary)
    for directory in ('SOURCES', 'SPECS', 'BUILD', 'BUILDROOT', 'RPMS', 'SRPMS'):
        (top / directory).mkdir()
    sources = top / 'SOURCES'
    for name in (APP, 'bitwarden-browser-proxy'):
        shutil.copy2(ROOT / 'target/release' / name, sources / name)
        run('strip', '--strip-unneeded', sources / name)
        (sources / name).chmod(0o755)
    entry = sources / (APP + '.desktop')
    entry.write_text('[Desktop Entry]\nType=Application\nName=Bitwarden [Next]\n'
                     'Comment=Native desktop vault client\n'
                     'Exec=/usr/bin/bitwarden-desktop-next\n'
                     'TryExec=/usr/bin/bitwarden-desktop-next\n'
                     'Icon=bitwarden-desktop-next\nTerminal=false\n'
                     'Categories=Utility;\nStartupWMClass=bitwarden-desktop-next\n')
    run('desktop-file-validate', entry)
    shutil.copyfile(ROOT / 'assets/icon.png', sources / 'icon.png')
    (sources / 'README.RPM').write_text(
        f'Unofficial native desktop vault client; built for Fedora/Nobara {fedora} {arch}.\n'
        f'Application source revision: {revision}' + (' (modified)' if dirty else '') + '\n'
        'Launch /usr/bin/bitwarden-desktop-next or use the applications menu.\n'
        'Quit any older/development instance before launching the installed build.\n'
        'A per-user desktop launcher in ~/.local/share/applications can override the system entry.\n'
        'A working D-Bus user session and Secret Service wallet are required.\n'
        'Use the existing KWallet or GNOME Keyring provider; avoid competing providers.\n'
        'KWallet encrypted sessions may need qca-qt6-ossl; GPG wallets need graphical pinentry.\n'
        'Browser registration is enabled explicitly from the application settings.\n'
        'Installation does not start the app or create autostart entries.\n'
        'User vault data is outside this package and is retained on removal.\n')
    licenses = []
    for name in ('LICENSE.txt', 'LICENSE_GPL.txt', 'LICENSE_BITWARDEN.txt'):
        shutil.copyfile(ROOT / name, sources / name)
        licenses.append(name)
    for dependency in ('reqwest', 'bitwarden-core', 'bitwarden-ipc'):
        for original in sorted((ROOT / 'vendor' / dependency).glob('LICENSE*')):
            name = dependency + '-' + original.name
            shutil.copyfile(original, sources / name)
            licenses.append(name)
    source_names = [APP, 'bitwarden-browser-proxy', APP + '.desktop', 'icon.png', 'README.RPM', *licenses]
    source_fields = '\n'.join(f'Source{index}: {name}' for index, name in enumerate(source_names))
    license_install = '\n'.join(f'install -m 0644 "%{{SOURCE{index + 5}}}" "%{{buildroot}}%{{_licensedir}}/%{{name}}/{name}"' for index, name in enumerate(licenses))
    # These libraries are loaded at runtime rather than all appearing in DT_NEEDED.
    graphics = ('libEGL.so.1', 'libGL.so.1', 'libxkbcommon.so.0', 'libxkbcommon-x11.so.0',
                'libXcursor.so.1', 'libXi.so.6', 'libXrandr.so.2', 'libwayland-client.so.0',
                'libwayland-cursor.so.0', 'libwayland-egl.so.1')
    graphics_requires = '\n'.join(f'Requires: {library}()(64bit)' for library in graphics)
    spec = f'''%global debug_package %{{nil}}
Name: {APP}
Version: {version}
Release: {release}%{{?dist}}
Summary: Unofficial native desktop client for Bitwarden and Vaultwarden
License: GPL-3.0-only AND LicenseRef-Bitwarden
URL: https://github.com/Der-Jan/bw-native-desktop-test
BuildArch: {arch}
{source_fields}
Requires: ca-certificates
Requires: hicolor-icon-theme
{graphics_requires}
Recommends: (gnome-keyring or kf6-kwallet)
Suggests: qca-qt6-ossl
Suggests: pinentry-qt

%description
Native egui desktop vault client with encrypted local storage, synchronization,
mTLS, SSH agent and browser integration. This is an unofficial development build.
A working desktop Secret Service provider is required for protected local secrets.

%prep
# Binaries are compiled by Cargo before rpmbuild on this target distribution.

%build

%install
install -D -m 0755 "%{{SOURCE0}}" "%{{buildroot}}%{{_bindir}}/{APP}"
install -D -m 0755 "%{{SOURCE1}}" "%{{buildroot}}%{{_bindir}}/bitwarden-browser-proxy"
install -D -m 0644 "%{{SOURCE2}}" "%{{buildroot}}%{{_datadir}}/applications/{APP}.desktop"
install -D -m 0644 "%{{SOURCE3}}" "%{{buildroot}}%{{_datadir}}/icons/hicolor/1024x1024/apps/{APP}.png"
install -D -m 0644 "%{{SOURCE4}}" "%{{buildroot}}%{{_docdir}}/%{{name}}/README.RPM"
install -d "%{{buildroot}}%{{_licensedir}}/%{{name}}"
{license_install}

%files
%{{_bindir}}/{APP}
%{{_bindir}}/bitwarden-browser-proxy
%{{_datadir}}/applications/{APP}.desktop
%{{_datadir}}/icons/hicolor/1024x1024/apps/{APP}.png
%doc %{{_docdir}}/%{{name}}
%license %{{_licensedir}}/%{{name}}
'''
    specfile = top / 'SPECS' / (APP + '.spec')
    specfile.write_text(spec)
    environment = os.environ.copy()
    environment['SOURCE_DATE_EPOCH'] = epoch
    run('rpmbuild', '-bb', '--define', f'_topdir {top}', '--define', '_buildhost localhost',
        '--define', 'use_source_date_epoch_as_buildtime 1', '--define', 'clamp_mtime_to_source_date_epoch 1',
        specfile, env=environment)
    packages = list((top / 'RPMS' / arch).glob('*.rpm'))
    if len(packages) != 1:
        raise RuntimeError(f'Expected one RPM, got {packages}')
    package = out / packages[0].name
    shutil.copyfile(packages[0], package)
    digest = hashlib.sha256(package.read_bytes()).hexdigest()
    package.with_suffix('.rpm.sha256').write_text(digest + '  ' + package.name + '\n')
    package.with_suffix('.rpm.build-info.json').write_text(json.dumps({
        'revision': revision, 'source_epoch': int(epoch), 'modified_application_sources': bool(dirty),
        'fedora': fedora, 'architecture': arch, 'sha256': digest,
        'requires': output('rpm', '-qp', '--requires', package).splitlines(),
    }, indent=2) + '\n')
    print(f'RPM: {package}\nSHA256: {digest}')

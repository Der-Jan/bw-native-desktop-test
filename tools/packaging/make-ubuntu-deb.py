#!/usr/bin/env python3
"""Package binaries built on Ubuntu 24.04; run inside the build container."""
import argparse
from datetime import datetime, timezone
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--source', type=Path, required=True)
parser.add_argument('--target', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--revision', required=True)
parser.add_argument('--maintainer', required=True)
args = parser.parse_args()
if 'VERSION_ID="24.04"' not in Path('/etc/os-release').read_text():
    parser.error('Run this packager on Ubuntu 24.04')
app = 'bitwarden-desktop-next'
version = tomllib.loads((args.source / 'crates/desktop/Cargo.toml').read_text())['package']['version']
# A hash is an identity, not a version ordering: git625... sorted above git07...
# despite containing older source. The separator also sorts above legacy hashes.
try:
    timestamp = datetime.fromtimestamp(int(os.environ['SOURCE_DATE_EPOCH']), timezone.utc)
except (KeyError, ValueError, OverflowError, OSError):
    parser.error('SOURCE_DATE_EPOCH must contain the source commit timestamp')
version += f"+git.{timestamp:%Y%m%d%H%M%S}.{args.revision}-1ubuntu24.04"
arch = subprocess.check_output(['dpkg', '--print-architecture'], text=True).strip()
args.output.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix='bw-deb-') as temporary:
    work = Path(temporary)
    root = work / 'debian' / app
    binary_dir = root / 'usr/bin'
    binary_dir.mkdir(parents=True)
    for name in (app, 'bitwarden-browser-proxy'):
        shutil.copy2(args.target / 'release' / name, binary_dir / name)
        subprocess.run(['strip', '--strip-unneeded', str(binary_dir / name)], check=True)
        (binary_dir / name).chmod(0o755)
    applications = root / 'usr/share/applications'
    applications.mkdir(parents=True)
    entry = applications / (app + '.desktop')
    entry.write_text('[Desktop Entry]\nType=Application\nName=Bitwarden [Next]\n'
                     'Comment=Native desktop vault client\n'
                     'Exec=/usr/bin/bitwarden-desktop-next\n'
                     'TryExec=/usr/bin/bitwarden-desktop-next\n'
                     'Icon=bitwarden-desktop-next\nTerminal=false\n'
                     'Categories=Utility;\nStartupWMClass=bitwarden-desktop-next\n')
    subprocess.run(['desktop-file-validate', str(entry)], check=True)
    icon = root / 'usr/share/icons/hicolor/1024x1024/apps' / (app + '.png')
    icon.parent.mkdir(parents=True)
    shutil.copyfile(args.source / 'assets/icon.png', icon)
    docs = root / 'usr/share/doc' / app
    docs.mkdir(parents=True)
    for name in ('LICENSE.txt', 'LICENSE_GPL.txt', 'LICENSE_BITWARDEN.txt'):
        shutil.copyfile(args.source / name, docs / name)
    (docs / 'copyright').write_text((args.source / 'LICENSE.txt').read_text())
    for dependency in ('reqwest', 'bitwarden-core', 'bitwarden-ipc'):
        for license_file in (args.source / 'vendor' / dependency).glob('LICENSE*'):
            shutil.copyfile(license_file, docs / (dependency + '-' + license_file.name))
    (docs / 'README.Debian').write_text(
        'Unofficial development build from github.com/Der-Jan/bw-native-desktop-test.\n'
        'Built on Ubuntu 24.04 for ' + arch + ', source revision ' + args.revision + '.\n'
        'Start from the applications menu or run bitwarden-desktop-next.\n'
        'A working Secret Service wallet (GNOME Keyring or compatible KWallet) is required.\n'
        'KWallet 5 on Ubuntu 24.04 needs libqca-qt5-2-plugins for encrypted sessions.\n'
        'GPG-protected wallets need a graphical pinentry, such as pinentry-qt.\n'
        'On GNOME, tray support needs an AppIndicator extension; the window works without it.\n'
        'Browser integration is enabled and registered explicitly in the app settings.\n'
        'This package does not enable autostart or modify browser profiles at installation.\n')
    debian = work / 'debian'
    debian.mkdir(exist_ok=True)
    (debian / 'control').write_text('Source: ' + app + '\nSection: utils\nPriority: optional\n'
        'Maintainer: ' + args.maintainer + '\n\nPackage: ' + app + '\nArchitecture: any\n'
        'Description: Native desktop vault client\n')
    result = subprocess.check_output(['dpkg-shlibdeps', '-O', *['-e' + str(p) for p in sorted(binary_dir.iterdir())]], cwd=work, text=True)
    depends = result.strip().removeprefix('shlibs:Depends=')
    # These graphics/window-system libraries are loaded at runtime through dlopen.
    depends += ', ca-certificates, libegl1, libgl1, libxkbcommon0, libxkbcommon-x11-0, libxcursor1, libxi6, libxrandr2'
    control = root / 'DEBIAN'
    control.mkdir()
    installed_size = (sum(p.stat().st_size for p in root.rglob('*') if p.is_file()) + 1023) // 1024
    (control / 'control').write_text(
        f'Package: {app}\nVersion: {version}\nArchitecture: {arch}\n'
        f'Maintainer: {args.maintainer}\nSection: utils\nPriority: optional\n'
        f'Installed-Size: {installed_size}\nDepends: {depends}\n'
        'Recommends: gnome-keyring | libkf5wallet-bin\n'
        'Suggests: gnome-shell-extension-appindicator, libqca-qt5-2-plugins, pinentry-qt\n'
        'Homepage: https://github.com/Der-Jan/bw-native-desktop-test\n'
        'Description: Native desktop vault client (unofficial development build)\n'
        ' Native egui client for Vaultwarden and Bitwarden servers, including\n'
        ' encrypted local storage, synchronization, mTLS, SSH agent and browser integration.\n')
    (control / 'md5sums').write_text(''.join(
        hashlib.md5(p.read_bytes()).hexdigest() + '  ' + str(p.relative_to(root)) + '\n'
        for p in sorted(root.rglob('*')) if p.is_file() and control not in p.parents))
    for path in root.rglob('*'):
        if path.is_dir():
            path.chmod(0o755)
        elif path.parent != binary_dir:
            path.chmod(0o644)
    destination = args.output / f'{app}_{version}_{arch}.deb'
    subprocess.run(['dpkg-deb', '--root-owner-group', '-Zxz', '--build', str(root), str(destination)], check=True)
    checksum = hashlib.sha256(destination.read_bytes()).hexdigest()
    destination.with_suffix('.deb.sha256').write_text(checksum + '  ' + destination.name + '\n')
    print(destination)

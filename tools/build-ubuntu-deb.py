#!/usr/bin/env python3
"""Build an Ubuntu 24.04 .deb in Podman/Docker, without mounting app profiles."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--engine', default=shutil.which('podman') or shutil.which('docker'))
parser.add_argument('--toolchain', type=Path, help='Rust toolchain directory containing bin/rustc and bin/cargo')
parser.add_argument('--cargo-cache', type=Path, help='Seed registry/git caches from this Cargo home')
parser.add_argument('--jobs', default='8')
parser.add_argument('--package-only', action='store_true', help='Package a previously completed build with build-info.json')
args = parser.parse_args()
if not args.engine:
    parser.error('Install Podman or Docker first')

def run(command, **kwargs):
    subprocess.run([str(v) for v in command], check=True, **kwargs)

def git(*arguments):
    return subprocess.check_output(['git', '-C', str(ROOT), *arguments], text=True).strip()

build = ROOT / 'target/ubuntu-24.04'
build.mkdir(parents=True, exist_ok=True)
output = ROOT / 'dist/ubuntu-24.04'
output.mkdir(parents=True, exist_ok=True)
image = 'localhost/bw-ubuntu24-build'
run([args.engine, 'build', '-t', image, '-f', ROOT / 'tools/packaging/ubuntu-24.04.Dockerfile', ROOT / 'tools/packaging'])
if not args.package_only:
    toolchain = args.toolchain
    if toolchain is None:
        toolchain = Path(subprocess.check_output(['rustup', 'which', 'rustc'], text=True).strip()).parent.parent
    toolchain = toolchain.resolve(strict=True)
    if not (toolchain / 'bin/cargo').is_file():
        parser.error('The toolchain must contain bin/cargo')
    source = build / 'source'
    if source.exists():
        shutil.rmtree(source)
    source.mkdir()
    # Only tracked source files; no .git, vault profiles, local logs or previews.
    for relative in subprocess.check_output(['git', '-C', str(ROOT), 'ls-files', '-z']).split(b'\0'):
        if not relative:
            continue
        relative = Path(os.fsdecode(relative))
        original = ROOT / relative
        destination = source / relative
        if original.is_file():
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(original, destination)
    cache = build / 'cargo-cache'
    cache.mkdir(exist_ok=True)
    if args.cargo_cache:
        for name in ('registry', 'git'):
            origin = args.cargo_cache / name
            if origin.is_dir() and not (cache / name).exists():
                shutil.copytree(origin, cache / name, symlinks=True)
    # Remove old metadata before attempting a rebuild; failed builds cannot be packaged.
    metadata = build / 'build-info.json'
    metadata.unlink(missing_ok=True)
    epoch = git('log', '-1', '--format=%ct')
    run([args.engine, 'run', '--rm', '-v', f'{build}:/build:rw', '-v', f'{toolchain}:/toolchain:ro',
         '-e', 'PATH=/toolchain/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin',
         '-e', 'CARGO_HOME=/build/cargo-cache', '-e', 'CARGO_TARGET_DIR=/build/target',
         '-e', f'CARGO_BUILD_JOBS={args.jobs}', '-e', f'SOURCE_DATE_EPOCH={epoch}',
         image, 'cargo', 'build', '--release', '--workspace', '--locked'])
    revision = git('rev-parse', '--short=12', 'HEAD')
    if subprocess.run(['git', 'diff', '--quiet', 'HEAD'], cwd=ROOT).returncode:
        revision += '.dirty'
    information = {'revision': revision,
                   'maintainer': git('log', '-1', '--format=%an <%ae>'), 'epoch': epoch}
    metadata.write_text(json.dumps(information, indent=2) + '\n')
else:
    information = json.loads((build / 'build-info.json').read_text())
run([args.engine, 'run', '--rm', '-v', f'{build}:/build:rw', '-v', f'{output}:/output:rw',
     '-v', f'{ROOT / "tools/packaging/make-ubuntu-deb.py"}:/make-deb.py:ro',
     '-e', f'SOURCE_DATE_EPOCH={information["epoch"]}', image,
     'python3', '/make-deb.py', '--source', '/build/source', '--target', '/build/target',
     '--output', '/output', '--revision', information['revision'], '--maintainer', information['maintainer']])
print(f'Package and SHA-256 checksum: {output}')

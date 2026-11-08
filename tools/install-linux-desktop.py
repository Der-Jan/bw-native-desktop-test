#!/usr/bin/env python3
"""Register a local build/preview with the desktop, without installing a package.

Run in the normal desktop environment, before a preview launcher overrides XDG
profile directories. Installed packages get their entry from cargo-packager.
"""

import argparse
import os
from pathlib import Path
import shutil
import subprocess


def desktop_string(value):
    return (value.replace("\\", "\\\\").replace("\n", "\\n")
            .replace("\r", "\\r").replace("\t", "\\t"))


def main():
    repo = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("launcher", type=Path,
                        help="Executable app binary or preview launcher script")
    args = parser.parse_args()
    launcher = args.launcher.resolve(strict=True)
    if not launcher.is_file() or not os.access(launcher, os.X_OK):
        parser.error("launcher must be an executable file")
    data = Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local/share")
    app_id = "bitwarden-desktop-next"
    icon = data / "icons/hicolor/1024x1024/apps" / f"{app_id}.png"
    entry = data / "applications" / f"{app_id}.desktop"
    icon.parent.mkdir(parents=True, exist_ok=True)
    entry.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(repo / "assets/icon.png", icon)
    # Exec has a second quoting layer after the desktop string escapes.
    executable = str(launcher).replace("%", "%%")
    for char in ('\\', '"', '`', '$'):
        executable = executable.replace(char, '\\' + char)
    quoted_executable = desktop_string('"' + executable + '"')
    entry.write_text(
        "[Desktop Entry]\nType=Application\nName=Bitwarden [Next]\n"
        f"Exec={quoted_executable}\n"
        f"Icon={app_id}\nStartupWMClass={app_id}\n"
        "Categories=Utility;\nTerminal=false\nStartupNotify=true\n"
    )
    if shutil.which("update-desktop-database"):
        subprocess.run(["update-desktop-database", str(entry.parent)], check=True)
    if shutil.which("kbuildsycoca6"):
        subprocess.run(["kbuildsycoca6", "--noincremental"], check=True)
    print(entry)


if __name__ == "__main__":
    main()

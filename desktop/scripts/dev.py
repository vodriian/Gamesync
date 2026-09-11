#!/usr/bin/env python3
"""Build the native demo. On macOS, create a development app bundle."""

import argparse
import os
from pathlib import Path
import plistlib
import subprocess
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--build-only", action="store_true")
parser.add_argument("--empty", action="store_true")
parser.add_argument("--stress", action="store_true")
parser.add_argument("--missing-covers", action="store_true")
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
subprocess.run(["cargo", "build", "--manifest-path", str(root / "Cargo.toml"), "--target-dir", str(root / "target")], check=True)
binary = root / "target/debug/gamesync-desktop"

if sys.platform == "darwin":
    contents = root / "target/GameSync.app/Contents"
    (contents / "MacOS").mkdir(parents=True, exist_ok=True)
    executable = contents / "MacOS/gamesync-desktop"
    if not executable.is_symlink():
        executable.symlink_to("../../../debug/gamesync-desktop")
    info = {
        "CFBundleExecutable": "gamesync-desktop",
        "CFBundleIdentifier": "local.gamesync.desktop",
        "CFBundleName": "GameSync",
        "CFBundlePackageType": "APPL",
        "CFBundleVersion": "0.1.0",
        "NSHighResolutionCapable": True,
    }
    (contents / "Info.plist").write_bytes(plistlib.dumps(info))
    binary = executable

print(f"Built: {binary}", flush=True)
if not args.build_only:
    flags = [f"--{name.replace('_', '-')}" for name in ("empty", "stress", "missing_covers") if getattr(args, name)]
    os.execv(str(binary), [str(binary), *flags])

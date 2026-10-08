#!/usr/bin/env python3
"""Build a standalone, signed macOS app and ZIP for local testing."""

import argparse
import os
import platform
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tomllib

if sys.platform != "darwin":
    raise SystemExit("Run this script on macOS.")


def signing_identity():
    """A code signing certificate, or "-" for an ad-hoc signature.

    Keychain trusts an app by its designated requirement. An ad-hoc signature
    pins the exact build, so macOS asks for the keychain password again after
    each rebuild. A certificate keeps the same requirement across builds.
    """
    chosen = os.environ.get("GAMESYNC_SIGN_IDENTITY")
    if chosen:
        return chosen
    listing = subprocess.run(
        ["security", "find-identity", "-v", "-p", "codesigning"],
        capture_output=True, text=True,
    ).stdout
    for kind in ("Developer ID Application", "Apple Development"):
        found = re.search(rf'^\s*\d+\) ([0-9A-F]{{40}}) "{kind}: ', listing, re.MULTILINE)
        if found:
            return found.group(1)
    return "-"


root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
# A separate folder keeps a test build apart from the current local release.
parser.add_argument("--output", type=Path, default=root / "target/macos")
parser.add_argument("--name", default="GameSync", help="App display name and bundle filename")
args = parser.parse_args()
if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9 _-]*", args.name):
    parser.error("--name must use letters, digits, spaces, underscores, or hyphens")
version = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
subprocess.run([
    "cargo", "build", "--release", "--locked", "--manifest-path",
    str(root / "Cargo.toml"), "--target-dir", str(root / "target"),
], check=True)
binary = root / "target/release/gamesync-desktop"
# Match the linked executable, rather than claiming support for an older OS.
load_commands = subprocess.check_output(["otool", "-l", str(binary)], text=True)
minimum_os = re.search(r"\bminos\s+(\S+)", load_commands)
if not minimum_os:
    raise SystemExit("Could not read the executable's minimum macOS version.")

output = args.output.resolve()
output.mkdir(parents=True, exist_ok=True)
app = output / f"{args.name}.app"
if app.exists():
    shutil.rmtree(app)
contents = app / "Contents"
(contents / "MacOS").mkdir(parents=True)
(contents / "Resources").mkdir()
shutil.copy2(binary, contents / "MacOS/gamesync-desktop")
shutil.copy2(root / "icon/app-icon.icns", contents / "Resources/app-icon.icns")
info = {
    "CFBundleExecutable": "gamesync-desktop",
    # Named test builds retain the normal Keychain identity and library paths.
    "CFBundleIdentifier": "local.gamesync.desktop",
    "CFBundleName": args.name,
    "CFBundleDisplayName": args.name,
    "CFBundleIconFile": "app-icon.icns",
    "CFBundlePackageType": "APPL",
    "CFBundleShortVersionString": version,
    "CFBundleVersion": version,
    "LSMinimumSystemVersion": minimum_os.group(1),
    "NSHighResolutionCapable": True,
}
(contents / "Info.plist").write_bytes(plistlib.dumps(info))
(contents / "PkgInfo").write_bytes(b"APPL????")
identity = signing_identity()
# A local test build needs no secure timestamp; this keeps signing offline.
subprocess.run(
    ["codesign", "--force", "--timestamp=none", "--sign", identity, str(app)], check=True
)
subprocess.run(["codesign", "--verify", "--strict", "--verbose=2", str(app)], check=True)
archive_name = args.name.replace(" ", "-")
archive = output / f"{archive_name}-{version}-macos-{platform.machine()}.zip"
if archive.exists():
    archive.unlink()
subprocess.run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(app), str(archive)], check=True)
if identity == "-":
    signature = ("Ad-hoc signature: Keychain asks again after each rebuild. "
                 "Add a code signing certificate to avoid this.")
else:
    signature = f"Signed with certificate {identity}."
print(f"App: {app}\nArchive: {archive}\n{signature} Not notarized.")

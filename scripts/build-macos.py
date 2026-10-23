#!/usr/bin/env python3
"""Build a universal Mac app on macOS; validate or prepare its source kit anywhere."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parent.parent
TARGETS = ("aarch64-apple-darwin", "x86_64-apple-darwin")


def version():
    return re.search(r'^version = "([0-9]+\.[0-9]+\.[0-9]+)"$', (ROOT / "Cargo.toml").read_text(), re.M)[1]


def info():
    return {
        "CFBundleName": "Cikung", "CFBundleDisplayName": "Cikung",
        "CFBundleIdentifier": "com.cikung.desktop", "CFBundleExecutable": "cikung",
        "CFBundleIconFile": "Cikung.icns", "CFBundlePackageType": "APPL",
        # Notch + right-click menu only, no Dock icon (matches Windows hiding from the taskbar).
        "LSUIElement": True,
        "CFBundleInfoDictionaryVersion": "6.0", "CFBundleDevelopmentRegion": "en",
        "CFBundleShortVersionString": version(), "CFBundleVersion": version(),
        "LSMinimumSystemVersion": "11.0", "NSHighResolutionCapable": True,
        "NSPrincipalClass": "NSApplication",
        "NSAppleEventsUsageDescription": "Cikung reads track information and controls playback in Spotify, Music and VLC when you connect them.",
    }


def validate():
    icon = (ROOT / "assets/icons/Cikung.icns").read_bytes()
    assert icon[:4] == b"icns" and struct.unpack_from(">I", icon, 4)[0] == len(icon), "Invalid ICNS header"
    offset, sizes = 8, set()
    while offset < len(icon):
        kind, length = struct.unpack_from(">4sI", icon, offset)
        assert length >= 8 and offset + length <= len(icon), "Invalid ICNS block"
        if kind == b"ic10":
            payload = icon[offset + 8:offset + length]
            assert payload[:8] == b"\x89PNG\r\n\x1a\n"
            assert struct.unpack_from(">II", payload, 16) == (1024, 1024)
        sizes.add(kind)
        offset += length
    assert offset == len(icon) and b"ic10" in sizes, "Missing 1024px icon"
    ico = (ROOT / "assets/icons/cikung.ico").read_bytes()
    reserved, kind, count = struct.unpack_from("<HHH", ico)
    assert (reserved, kind) == (0, 1) and count >= 6
    for index in range(count):
        length, start = struct.unpack_from("<II", ico, 6 + index * 16 + 8)
        assert length > 0 and start >= 6 + count * 16 and start + length <= len(ico)
    data = plistlib.loads(plistlib.dumps(info()))
    assert data["CFBundleIconFile"] == "Cikung.icns" and data.get("LSUIElement") is True
    assert data["NSAppleEventsUsageDescription"] and data["CFBundleVersion"] == version()
    for file in ("Cargo.lock", "LICENSE", "README.md", "docs/macos.md", "Build-Cikung.command"):
        assert (ROOT / file).is_file(), file
    assert b"\r" not in (ROOT / "Build-Cikung.command").read_bytes(), "Mac launcher requires LF line endings"
    print(json.dumps({"version": version(), "icon_and_bundle_checks": "passed", "native_mac_build": "not run by --check"}))


def source_kit():
    validate()
    files = [ROOT / name for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "README.md", "LICENSE", "THIRD_PARTY_NOTICES.md", "Build-Cikung.command", "scripts/build-macos.py", "scripts/generate-app-icons.py", ".gitattributes", ".github/workflows/macos.yml", "scripts/cikung-hook.ps1", "scripts/launch-agent.ps1", "scripts/cikung-codex-hook.cjs", "docs/macos.md", "docs/asset-provenance.md", "docs/feature-coverage.md")]
    for folder in ("src", "assets", "adapters"):
        files.extend(path for path in (ROOT / folder).rglob("*") if path.is_file())
    output = ROOT / "dist/Cikung-Mac-Build-Kit.zip"
    output.parent.mkdir(exist_ok=True)
    with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(files):
            assert not path.is_symlink(), "Source kit cannot include symlinks"
            entry = zipfile.ZipInfo("Cikung-Mac-Build-Kit/" + path.relative_to(ROOT).as_posix())
            entry.create_system = 3
            executable = path.suffix in (".command", ".py")
            entry.external_attr = (0o100755 if executable else 0o100644) << 16
            content = (ROOT / "docs/macos.md").read_bytes() if path == ROOT / "README.md" else path.read_bytes()
            archive.writestr(entry, content, compress_type=zipfile.ZIP_DEFLATED)
    with zipfile.ZipFile(output) as archive:
        assert archive.testzip() is None
        assert not any("/.tools/" in name or "/reference/" in name or name.endswith(".exe") for name in archive.namelist())
    print("Mac source build kit (not a compiled app):", output)


def run(*args, env=None):
    return subprocess.run(args, cwd=ROOT, env=env, check=True, text=True, stdout=subprocess.PIPE).stdout


def licenses(destination):
    destination.mkdir()
    shutil.copy2(ROOT / "assets/fonts/OFL-Figtree.txt", destination)
    rust_docs = Path(run("rustc", "--print", "sysroot").strip()) / "share/doc/rust"
    shutil.copy2(rust_docs / "COPYRIGHT.html", destination / "Rust-runtime-COPYRIGHT.html")
    for path in (rust_docs / "licenses").glob("*.txt"):
        shutil.copy2(path, destination / path.name)
    packages = {}
    for target in TARGETS:
        data = json.loads(run("cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target))
        for package in data["packages"]:
            packages[package["id"]] = package
    inventory = []
    for package in packages.values():
        identity = package["name"] + "-" + package["version"]
        inventory.append(identity + ": " + (package.get("license") or "See package notices"))
        folder = Path(package["manifest_path"]).parent
        for path in folder.iterdir():
            if path.is_file() and re.match(r"^(licen[cs]e|copying|copyright|notice|readme)([._-]|$)", path.name, re.I):
                target = destination / identity
                target.mkdir(exist_ok=True)
                shutil.copy2(path, target / path.name)
    (destination / "dependency-inventory.txt").write_text("\n".join(sorted(inventory)) + "\n")


def build():
    validate()
    if sys.platform != "darwin":
        raise SystemExit("A Mac with Xcode Command Line Tools and Rust is required. This Windows/Linux check did not build a Mac app.")
    for tool in ("cargo", "rustup", "xcrun", "lipo", "codesign", "ditto"):
        if not shutil.which(tool):
            raise SystemExit("Missing required Mac build tool: " + tool)
    run("xcrun", "--show-sdk-path")
    env = dict(os.environ, MACOSX_DEPLOYMENT_TARGET="11.0", CARGO_TARGET_DIR=str(ROOT / "target"))
    for target in TARGETS:
        run("rustup", "target", "add", target)
        run("cargo", "build", "--release", "--locked", "--target", target, env=env)
    output = ROOT / "dist"
    output.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="cikung-mac-", dir=output) as temp:
        app = Path(temp) / "Cikung.app"
        contents = app / "Contents"
        binary = contents / "MacOS/cikung"
        resources = contents / "Resources"
        binary.parent.mkdir(parents=True)
        resources.mkdir()
        inputs = [str(ROOT / "target" / target / "release/cikung") for target in TARGETS]
        run("lipo", "-create", *inputs, "-output", str(binary))
        binary.chmod(0o755)
        run("lipo", str(binary), "-verify_arch", "arm64", "x86_64")
        (contents / "Info.plist").write_bytes(plistlib.dumps(info()))
        shutil.copy2(ROOT / "assets/icons/Cikung.icns", resources)
        for name in ("LICENSE", "THIRD_PARTY_NOTICES.md", "Cargo.lock"):
            shutil.copy2(ROOT / name, resources)
        shutil.copy2(ROOT / "docs/macos.md", resources / "README-macOS.md")
        licenses(resources / "licenses")
        run("codesign", "--force", "--sign", "-", str(app))
        run("codesign", "--verify", "--strict", "--verbose=2", str(app))
        help_text = run(str(binary), "--help")
        assert "Cikung " + version() in help_text, "Mac executable smoke check failed"
        archive = Path(temp) / "Cikung-macOS-universal.zip"
        run("ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(app), str(archive))
        final = output / archive.name
        os.replace(archive, final)
    digest = hashlib.sha256(final.read_bytes()).hexdigest()
    final.with_suffix(".zip.sha256").write_text(digest + "  " + final.name + "\n")
    print("Universal Mac app (ad-hoc signed, not notarized):", final)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="Validate icons/bundle metadata without building")
    mode.add_argument("--source-kit", action="store_true", help="Prepare a source kit; this is not a Mac binary")
    args = parser.parse_args()
    if args.check:
        validate()
    elif args.source_kit:
        source_kit()
    else:
        build()

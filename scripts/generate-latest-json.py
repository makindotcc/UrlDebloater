#!/usr/bin/env python3
"""Build the update manifest from the artifacts produced by cargo-packager."""

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import quote


def artifact(dist: Path, pattern: str) -> Path:
    matches = sorted(dist.glob(pattern))
    if len(matches) != 1:
        raise SystemExit(f"expected exactly one {pattern!r} in {dist}, found {len(matches)}")
    return matches[0]


def signature(path: Path) -> str:
    sig = Path(f"{path}.sig")
    if not sig.is_file():
        raise SystemExit(f"missing signature for {path.name}")
    value = sig.read_text().strip()
    if not value:
        raise SystemExit(f"empty signature for {path.name}")
    return value


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("dist", type=Path)
    parser.add_argument("tag")
    parser.add_argument("server")
    parser.add_argument("repository")
    parser.add_argument("--macos-arch", default="aarch64")
    args = parser.parse_args()

    version = args.tag.removeprefix("v")
    base = f"{args.server}/{args.repository}/releases/download/{args.tag}"

    platforms = {}
    windows = artifact(args.dist, "*.exe")
    platforms["windows-x86_64"] = {
        "url": f"{base}/{quote(windows.name)}",
        "signature": signature(windows),
        "format": "nsis",
    }

    macos = artifact(args.dist, "*.app.tar.gz")
    platforms[f"macos-{args.macos_arch}"] = {
        "url": f"{base}/{quote(macos.name)}",
        "signature": signature(macos),
        "format": "app",
    }

    appimage = artifact(args.dist, "*.AppImage")
    platforms["linux-x86_64"] = {
        "url": f"{base}/{quote(appimage.name)}",
        "signature": signature(appimage),
        "format": "appimage",
    }

    manifest = {
        "version": version,
        "pub_date": datetime.now(timezone.utc).replace(microsecond=0).isoformat(),
        "platforms": platforms,
    }
    (args.dist / "latest.json").write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()

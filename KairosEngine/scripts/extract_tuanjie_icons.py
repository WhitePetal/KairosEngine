#!/usr/bin/env python3
"""Extract the Project panel's row icons from a local Unity/Tuanjie editor install.

The engine ships no art for these: Unity's Project browser draws its folder and
asset icons from the editor resources bundle, and the Kairos Project panel is
specified against that look (docs/plan/project-panel). The bundle is a Unity
asset file, so extraction needs UnityPy:

    python -m pip install UnityPy

Usage (from the workspace root):

    python scripts/extract_tuanjie_icons.py
    python scripts/extract_tuanjie_icons.py --editor "D:/UnityInstalls/2022.3.62t6"

The script discovers an editor install under the usual Windows locations, reads
the named Texture2D entries out of the editor resources bundle, and writes them
downscaled to 64x64 (a 2x-4x source for an 18 px row icon; the full-size editor
textures are 256-2048 px and would cost megabytes of texture memory per icon).

Outputs are committed under Preferences/Textures/project/ so the editor needs no
Python at runtime. Provenance: the extracted artwork is Unity/Tuanjie editor
artwork (see Preferences/Textures/project/README.md).
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

# (dark-skin source name, light-skin source name, dark output, light output)
# Relative to the textures root; the two skins stay in sibling folders, the same
# layout the Console panel's icons use.
ICONS = [
    ("d_Folder Icon", "Folder Icon", "project/folder.png", "project/light/folder.png"),
    (
        "d_FolderOpened Icon",
        "FolderOpened Icon",
        "project/folder_open.png",
        "project/light/folder_open.png",
    ),
    (
        "d_DefaultAsset Icon",
        "DefaultAsset Icon",
        "project/file.png",
        "project/light/file.png",
    ),
]

SIZE = 64

# Bundle file names used by the Unity and Tuanjie editors respectively.
BUNDLES = ("tuanjie editor resources", "unity editor resources")

SEARCH_ROOTS = (
    r"D:\UnityInstalls",
    r"C:\Program Files\Unity\Hub\Editor",
    r"C:\Program Files\Tuanjie\Hub\Editor",
    r"D:\Unity",
    r"C:\Program Files\Unity",
)


def find_editor(explicit: str | None) -> Path:
    """The editor install root (the folder holding Editor/Data/Resources)."""
    candidates: list[Path] = []
    if explicit:
        candidates.append(Path(explicit))
    for root in SEARCH_ROOTS:
        base = Path(root)
        if base.is_dir():
            candidates.extend(sorted(p for p in base.iterdir() if p.is_dir()))
    for editor in candidates:
        resources = editor / "Editor" / "Data" / "Resources"
        if any((resources / name).is_file() for name in BUNDLES):
            return editor
    raise SystemExit(
        "no Unity/Tuanjie editor found; pass --editor <install root> "
        "(the folder containing Editor/Data/Resources)"
    )


def bundle_path(editor: Path) -> Path:
    resources = editor / "Editor" / "Data" / "Resources"
    for name in BUNDLES:
        path = resources / name
        if path.is_file():
            return path
    raise SystemExit(f"no editor resources bundle under {resources}")


def extract(editor: Path, textures: Path) -> int:
    import UnityPy  # noqa: PLC0415 - optional, only needed by this tool
    from PIL import Image

    bundle = bundle_path(editor)
    print(f"reading {bundle}")
    env = UnityPy.load(str(bundle))

    wanted: dict[str, tuple[Path, str]] = {}
    for dark_name, light_name, dark_out, light_out in ICONS:
        wanted[dark_name] = (textures / dark_out, "dark")
        wanted[light_name] = (textures / light_out, "light")

    written: set[str] = set()
    for obj in env.objects:
        if obj.type.name != "Texture2D":
            continue
        data = obj.read()
        name = data.m_Name
        target = wanted.get(name)
        if target is None:
            continue
        image = data.image
        if image is None:
            print(f"  {name}: no decodable image")
            continue
        image = image.convert("RGBA").resize((SIZE, SIZE), Image.LANCZOS)
        out, skin = target
        out.parent.mkdir(parents=True, exist_ok=True)
        image.save(out, optimize=True)
        print(f"  {name:24s} {skin:5s} -> {out.relative_to(textures.parent.parent)}")
        written.add(name)

    missing = sorted(set(wanted) - written)
    if missing:
        raise SystemExit(f"missing textures: {missing}")
    return len(written)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--editor", help="editor install root (contains Editor/Data)")
    parser.add_argument(
        "--textures",
        default=str(Path(__file__).resolve().parent.parent / "Preferences" / "Textures"),
        help="textures root to write into",
    )
    args = parser.parse_args()
    editor = find_editor(args.editor)
    count = extract(editor, Path(args.textures))
    print(f"{count} icons written from {editor}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

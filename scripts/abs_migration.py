#!/usr/bin/env python3
import argparse
import json
import os
import re
import shutil
import sys
from dataclasses import dataclass, asdict
from typing import Dict, List, Optional, Tuple

from mutagen import File as MutagenFile
from mutagen.mp4 import MP4
from mutagen.id3 import ID3, TXXX

AUDIO_EXTS = {".m4b", ".m4a", ".mp3"}
COVER_NAMES = {"cover", "folder", "front", "art"}
EXCLUDED_DIRS = {"temp_shrink"}


@dataclass
class BookGroup:
    source_files: List[str]
    source_dir: str
    author: Optional[str]
    title: Optional[str]
    series: Optional[str]
    series_part: Optional[str]
    book_folder: Optional[str]
    dest_dir: Optional[str]
    dest_files: List[str]
    issues: List[str]
    companion_files: List[str]


@dataclass
class Plan:
    source_root: str
    dest_root: str
    books: List[BookGroup]
    conflicts: List[str]
    skipped_companions: Dict[str, List[str]]


def _collapse_spaces(value: str) -> str:
    return re.sub(r"\s+", " ", value).strip()


def strip_unabridged(value: str) -> str:
    if not value:
        return value
    pattern = re.compile(r"\s*[\[(](?i:unabridged)[\])]\s*$")
    value = re.sub(pattern, "", value)
    return _collapse_spaces(value)


def sanitize_component(value: str, preserve_commas: bool) -> str:
    if value is None:
        return value
    value = value.replace(":", " - ")
    value = value.replace("/", " ").replace("\\", " ")
    if preserve_commas:
        pass
    else:
        value = value.replace(",", " - ")
    value = re.sub(r"[\?\*\<\>\|]", "", value)
    value = value.replace('"', "'")
    return _collapse_spaces(value)


def split_sequence_prefix(title: str) -> Tuple[Optional[str], str]:
    if not title:
        return None, title
    patterns = [
        r"^(?:Book|Vol\.?|Volume)\s*(\d+)\s*[\-\.]\s*(.+)$",
        r"^(?:#)?(\d{1,3})\s*[\-\.]\s*(.+)$",
    ]
    for pat in patterns:
        match = re.match(pat, title, flags=re.IGNORECASE)
        if match:
            return match.group(1), match.group(2).strip()
    return None, title


def decode_mp4_freeform(value) -> Optional[str]:
    if not value:
        return None
    if isinstance(value, list):
        if not value:
            return None
        value = value[0]
    if isinstance(value, bytes):
        try:
            return value.decode("utf-8").strip()
        except Exception:
            return value.decode("latin-1").strip()
    return str(value).strip()


def get_mp4_text(tags, key: str) -> Optional[str]:
    if not tags or key not in tags:
        return None
    value = tags.get(key)
    if isinstance(value, list) and value:
        value = value[0]
    if isinstance(value, bytes):
        try:
            return value.decode("utf-8").strip()
        except Exception:
            return value.decode("latin-1").strip()
    if value is None:
        return None
    return str(value).strip()


def get_id3_text(tags: ID3, key: str) -> Optional[str]:
    if not tags or key not in tags:
        return None
    frame = tags.get(key)
    if not frame:
        return None
    if hasattr(frame, "text") and frame.text:
        return str(frame.text[0]).strip()
    return None


def get_id3_txxx(tags: ID3, desc: str) -> Optional[str]:
    if not tags:
        return None
    for frame in tags.getall("TXXX"):
        if isinstance(frame, TXXX) and frame.desc.lower() == desc.lower():
            if frame.text:
                return str(frame.text[0]).strip()
    return None


def read_tags(path: str) -> Dict[str, Optional[str]]:
    data: Dict[str, Optional[str]] = {
        "title": None,
        "album": None,
        "author": None,
        "series": None,
        "series_part": None,
    }
    audio = MutagenFile(path)
    if audio is None:
        return data

    if isinstance(audio, MP4):
        tags = audio.tags or {}
        data["title"] = get_mp4_text(tags, "\xa9nam")
        data["album"] = get_mp4_text(tags, "\xa9alb")
        data["author"] = get_mp4_text(tags, "\xa9ART") or get_mp4_text(tags, "aART")
        data["series"] = decode_mp4_freeform(tags.get("----:com.apple.iTunes:SERIES"))
        data["series_part"] = decode_mp4_freeform(tags.get("----:com.apple.iTunes:SERIES-PART"))
        return data

    tags = audio.tags if hasattr(audio, "tags") else None
    if isinstance(tags, ID3):
        data["title"] = get_id3_text(tags, "TIT2")
        data["album"] = get_id3_text(tags, "TALB")
        data["author"] = get_id3_text(tags, "TPE1")
        data["series"] = get_id3_txxx(tags, "SERIES")
        data["series_part"] = get_id3_txxx(tags, "SERIES-PART")
    return data


def parse_series_part(value: Optional[str]) -> Optional[str]:
    if not value:
        return None
    match = re.search(r"(\d+)", value)
    if match:
        return str(int(match.group(1)))
    return value.strip()


def detect_audio_files(source_root: str) -> Dict[str, List[str]]:
    files_by_dir: Dict[str, List[str]] = {}
    for root, dirs, files in os.walk(source_root):
        dirs[:] = [d for d in dirs if d not in EXCLUDED_DIRS]
        audio_files = []
        for name in files:
            ext = os.path.splitext(name)[1].lower()
            if ext in AUDIO_EXTS:
                audio_files.append(os.path.join(root, name))
        if audio_files:
            files_by_dir[root] = sorted(audio_files)
    return files_by_dir


def group_files(source_root: str) -> List[BookGroup]:
    groups: List[BookGroup] = []
    files_by_dir = detect_audio_files(source_root)

    for directory, files in files_by_dir.items():
        mp3_files = [f for f in files if os.path.splitext(f)[1].lower() == ".mp3"]
        other_files = [f for f in files if os.path.splitext(f)[1].lower() != ".mp3"]

        for path in other_files:
            tags = read_tags(path)
            groups.append(
                BookGroup(
                    source_files=[path],
                    source_dir=directory,
                    author=tags.get("author"),
                    title=tags.get("title") or tags.get("album"),
                    series=tags.get("series"),
                    series_part=tags.get("series_part"),
                    book_folder=None,
                    dest_dir=None,
                    dest_files=[],
                    issues=[],
                    companion_files=[],
                )
            )

        if not mp3_files:
            continue

        mp3_tags = [(path, read_tags(path)) for path in mp3_files]
        albums = [t[1].get("album") for t in mp3_tags if t[1].get("album")]
        if not albums:
            groups.append(
                BookGroup(
                    source_files=mp3_files,
                    source_dir=directory,
                    author=None,
                    title=None,
                    series=None,
                    series_part=None,
                    book_folder=None,
                    dest_dir=None,
                    dest_files=[],
                    issues=["mp3_album_missing"],
                    companion_files=[],
                )
            )
        else:
            by_album: Dict[str, List[Tuple[str, Dict[str, Optional[str]]]]] = {}
            for path, tags in mp3_tags:
                album = tags.get("album") or "__missing__"
                by_album.setdefault(album, []).append((path, tags))
            for album, items in by_album.items():
                author = None
                series = None
                series_part = None
                for _path, tags in items:
                    author = author or tags.get("author")
                    series = series or tags.get("series")
                    series_part = series_part or tags.get("series_part")
                groups.append(
                    BookGroup(
                        source_files=[p for p, _ in items],
                        source_dir=directory,
                        author=author,
                        title=album if album != "__missing__" else None,
                        series=series,
                        series_part=series_part,
                        book_folder=None,
                        dest_dir=None,
                        dest_files=[],
                        issues=[],
                        companion_files=[],
                    )
                )

    return groups


def infer_from_path(group: BookGroup, source_root: str) -> None:
    if group.author:
        return
    parts = os.path.relpath(group.source_dir, source_root).split(os.sep)
    if parts and parts[0] != ".":
        group.author = parts[0]
        group.issues.append("author_from_path")


def infer_from_filename(group: BookGroup) -> None:
    if group.title:
        return
    if not group.source_files:
        return
    filename = os.path.basename(group.source_files[0])
    stem = os.path.splitext(filename)[0]
    if " - " in stem and not group.author:
        first, rest = stem.split(" - ", 1)
        group.author = first.strip()
        group.title = rest.strip()
        group.issues.append("author_title_from_filename")
        return
    group.title = stem.strip()
    group.issues.append("title_from_filename")


def infer_series_from_path(group: BookGroup, source_root: str) -> None:
    if group.series:
        return
    rel_dir = os.path.relpath(group.source_dir, source_root)
    parts = rel_dir.split(os.sep)
    if len(parts) >= 3:
        series_candidate = parts[1]
        if series_candidate and series_candidate != group.author:
            group.series = series_candidate
            group.issues.append("series_from_path")


def build_destination(
    group: BookGroup,
    dest_root: str,
    strip_unabridged_flag: bool,
) -> None:
    if strip_unabridged_flag and group.title:
        group.title = strip_unabridged(group.title)
    if group.series:
        group.series = strip_unabridged(group.series) if strip_unabridged_flag else group.series

    if group.title:
        seq_from_title, title_clean = split_sequence_prefix(group.title)
        if seq_from_title and not group.series_part:
            group.series_part = seq_from_title
            group.issues.append("series_part_from_title")
        group.title = title_clean

    group.series_part = parse_series_part(group.series_part)

    if not group.author:
        group.issues.append("author_missing")
    if not group.title:
        group.issues.append("title_missing")

    author = sanitize_component(group.author or "Unknown Author", preserve_commas=True)
    title = sanitize_component(group.title or "Unknown Title", preserve_commas=False)
    series = sanitize_component(group.series, preserve_commas=False) if group.series else None

    if group.series_part:
        book_label = f"Book {group.series_part} - {title}"
    else:
        book_label = title

    group.book_folder = book_label

    dest_parts = [dest_root, author]
    if series:
        dest_parts.append(series)
    dest_parts.append(book_label)

    group.dest_dir = os.path.join(*dest_parts)

    group.dest_files = []
    if len(group.source_files) == 1:
        ext = os.path.splitext(group.source_files[0])[1].lower()
        filename = f"{book_label}{ext}"
        group.dest_files.append(os.path.join(group.dest_dir, filename))
    else:
        for path in group.source_files:
            name = os.path.basename(path)
            group.dest_files.append(os.path.join(group.dest_dir, name))


def collect_companions(group: BookGroup, groups_in_dir: int) -> None:
    companion_files: List[str] = []
    skipped: List[str] = []
    for name in os.listdir(group.source_dir):
        path = os.path.join(group.source_dir, name)
        if os.path.isdir(path):
            continue
        ext = os.path.splitext(name)[1].lower()
        if ext in AUDIO_EXTS:
            continue
        if groups_in_dir == 1:
            companion_files.append(path)
            continue

        stem = os.path.splitext(name)[0].lower()
        if stem in COVER_NAMES:
            companion_files.append(path)
            continue
        if len(group.source_files) == 1:
            audio_stem = os.path.splitext(os.path.basename(group.source_files[0]))[0].lower()
            if stem == audio_stem:
                companion_files.append(path)
                continue
        skipped.append(path)

    group.companion_files = sorted(companion_files)
    if skipped:
        group.issues.append("companions_skipped_shared_dir")


def build_plan(source_root: str, dest_root: str, strip_unabridged_flag: bool) -> Plan:
    groups = group_files(source_root)
    groups_by_dir: Dict[str, List[BookGroup]] = {}
    for group in groups:
        groups_by_dir.setdefault(group.source_dir, []).append(group)

    for group in groups:
        infer_from_path(group, source_root)
        infer_from_filename(group)
        infer_series_from_path(group, source_root)
        build_destination(group, dest_root, strip_unabridged_flag)
        collect_companions(group, len(groups_by_dir[group.source_dir]))

    conflicts: List[str] = []
    seen_paths: Dict[str, str] = {}
    for group in groups:
        for dest in group.dest_files:
            key = os.path.normpath(dest).lower()
            if key in seen_paths:
                conflicts.append(f"{dest} <- conflicts with {seen_paths[key]}")
            else:
                seen_paths[key] = dest
        for companion in group.companion_files:
            dest_path = os.path.join(group.dest_dir, os.path.basename(companion))
            key = os.path.normpath(dest_path).lower()
            if key in seen_paths:
                conflicts.append(f"{dest_path} <- conflicts with {seen_paths[key]}")
            else:
                seen_paths[key] = dest_path

    skipped_companions: Dict[str, List[str]] = {}
    for group in groups:
        if "companions_skipped_shared_dir" in group.issues:
            skipped = []
            for name in os.listdir(group.source_dir):
                path = os.path.join(group.source_dir, name)
                if os.path.isfile(path) and os.path.splitext(name)[1].lower() not in AUDIO_EXTS:
                    if path not in group.companion_files:
                        skipped.append(path)
            if skipped:
                skipped_companions[group.source_dir] = sorted(skipped)

    return Plan(
        source_root=source_root,
        dest_root=dest_root,
        books=groups,
        conflicts=conflicts,
        skipped_companions=skipped_companions,
    )


def write_plan(plan: Plan, output_path: str) -> None:
    serializable = {
        "source_root": plan.source_root,
        "dest_root": plan.dest_root,
        "conflicts": plan.conflicts,
        "skipped_companions": plan.skipped_companions,
        "books": [asdict(book) for book in plan.books],
    }
    with open(output_path, "w", encoding="utf-8") as handle:
        json.dump(serializable, handle, indent=2)


def write_report(plan: Plan, report_path: str) -> None:
    total_books = len(plan.books)
    total_files = sum(len(group.source_files) for group in plan.books)
    missing_author = [g for g in plan.books if "author_missing" in g.issues]
    missing_title = [g for g in plan.books if "title_missing" in g.issues]
    skipped_comp = sum(len(v) for v in plan.skipped_companions.values())

    lines = [
        "Audiobookshelf Migration Dry Run Report",
        "======================================",
        f"Source root: {plan.source_root}",
        f"Destination root: {plan.dest_root}",
        f"Books detected: {total_books}",
        f"Audio files detected: {total_files}",
        f"Conflicts: {len(plan.conflicts)}",
        f"Books missing author: {len(missing_author)}",
        f"Books missing title: {len(missing_title)}",
        f"Skipped companion files (shared dirs): {skipped_comp}",
        "",
    ]

    if plan.conflicts:
        lines.append("Conflicts (resolve before execute):")
        lines.extend(f"  - {item}" for item in plan.conflicts)
        lines.append("")

    if missing_author:
        lines.append("Missing author (first 25):")
        for group in missing_author[:25]:
            lines.append(f"  - {group.source_files[0]}")
        lines.append("")

    if missing_title:
        lines.append("Missing title (first 25):")
        for group in missing_title[:25]:
            lines.append(f"  - {group.source_files[0]}")
        lines.append("")

    if plan.skipped_companions:
        lines.append("Skipped companion files (shared dirs):")
        for directory, files in list(plan.skipped_companions.items())[:10]:
            lines.append(f"  - {directory} ({len(files)} files)")
            for file in files[:10]:
                lines.append(f"      * {file}")
        lines.append("")

    with open(report_path, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines))


def execute_plan(plan: Plan) -> None:
    if plan.conflicts:
        raise RuntimeError("Conflicts detected. Resolve before execute.")

    for group in plan.books:
        if not group.dest_dir:
            raise RuntimeError(f"Missing destination for {group.source_files}")
        os.makedirs(group.dest_dir, exist_ok=True)

        if len(group.source_files) == 1:
            src = group.source_files[0]
            dst = group.dest_files[0]
            shutil.copy2(src, dst)
        else:
            for src, dst in zip(group.source_files, group.dest_files):
                shutil.copy2(src, dst)

        for companion in group.companion_files:
            dst = os.path.join(group.dest_dir, os.path.basename(companion))
            if not os.path.exists(dst):
                shutil.copy2(companion, dst)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Audiobookshelf migration helper")
    parser.add_argument("--source", required=True, help="Source library root")
    parser.add_argument("--dest", required=True, help="Destination library root")
    parser.add_argument(
        "--plan-json",
        default=None,
        help="Write plan JSON to this path (defaults to source root)",
    )
    parser.add_argument(
        "--report",
        default=None,
        help="Write report text to this path (defaults to source root)",
    )
    parser.add_argument(
        "--execute",
        action="store_true",
        help="Execute copy (default is dry run)",
    )
    parser.add_argument(
        "--strip-unabridged",
        action="store_true",
        default=True,
        help="Strip (Unabridged) from names (default: true)",
    )
    parser.add_argument(
        "--no-strip-unabridged",
        action="store_false",
        dest="strip_unabridged",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    source_root = os.path.abspath(args.source)
    dest_root = os.path.abspath(args.dest)

    plan_json = args.plan_json or os.path.join(source_root, "!abs-migration-move-plan.json")
    report = args.report or os.path.join(source_root, "!abs-migration-report.txt")

    plan = build_plan(source_root, dest_root, args.strip_unabridged)
    write_plan(plan, plan_json)
    write_report(plan, report)

    if args.execute:
        execute_plan(plan)

    print(f"Plan JSON: {plan_json}")
    print(f"Report: {report}")
    if plan.conflicts:
        print(f"Conflicts: {len(plan.conflicts)} (resolve before execute)")
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

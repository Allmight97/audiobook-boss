#!/usr/bin/env python3
import argparse
import csv
import os
import re
from typing import Dict, Optional, Tuple

from mutagen.mp4 import MP4
from mutagen.mp3 import MP3
from mutagen.id3 import ID3, TXXX

AUDIO_EXTS = {".m4b", ".m4a", ".mp3"}


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
        "author": None,
        "narrator": None,
        "title": None,
    }
    ext = os.path.splitext(path)[1].lower()
    try:
        if ext in {".m4b", ".m4a"}:
            audio = MP4(path)
            tags = audio.tags or {}
        elif ext == ".mp3":
            audio = MP3(path)
            tags = audio.tags
        else:
            return data
    except Exception:
        return data

    if ext in {".m4b", ".m4a"}:
        data["title"] = get_mp4_text(tags, "\xa9nam")
        data["author"] = get_mp4_text(tags, "\xa9ART") or get_mp4_text(tags, "aART")
        data["narrator"] = get_mp4_text(tags, "\xa9wrt") or get_mp4_text(tags, "\xa9com")
        return data

    if isinstance(tags, ID3):
        data["title"] = get_id3_text(tags, "TIT2")
        data["author"] = get_id3_text(tags, "TPE1")
        # Audiobook Boss convention: narrator stored in composer
        data["narrator"] = get_id3_text(tags, "TCOM") or get_id3_txxx(tags, "NARRATOR")
    return data


def normalize(value: Optional[str]) -> str:
    if not value:
        return ""
    return re.sub(r"\s+", " ", value).strip().lower()


def has_multiple_names(value: str) -> bool:
    if not value:
        return False
    separators = [";", " & ", " and ", ","]
    return any(sep in value for sep in separators)


def split_author_names(value: str) -> list:
    if not value:
        return []
    parts = re.split(r"\s*(?:;|&|,|\\band\\b)\\s*", value, flags=re.IGNORECASE)
    return [part.strip() for part in parts if part.strip()]


def looks_like_narrator_in_author(author: str, narrator: str) -> bool:
    if not author or not narrator:
        return False
    return normalize(narrator) in normalize(author) or normalize(author) in normalize(narrator)


def scan(root: str) -> Tuple[int, int, int, int, list]:
    total = 0
    flagged = 0
    multi_author = 0
    author_equals_narrator_count = 0
    author_multi_includes_narrator_count = 0
    missing_narrator = 0
    rows = []

    for dirpath, _dirs, files in os.walk(root):
        for name in files:
            ext = os.path.splitext(name)[1].lower()
            if ext not in AUDIO_EXTS:
                continue
            path = os.path.join(dirpath, name)
            total += 1
            tags = read_tags(path)
            author = tags.get("author") or ""
            narrator = tags.get("narrator") or ""
            title = tags.get("title") or ""

            author_multi = has_multiple_names(author)
            author_equals_narrator = bool(author and narrator) and normalize(author) == normalize(
                narrator
            )
            narrator_missing = not narrator
            author_narrator_overlap = looks_like_narrator_in_author(author, narrator)
            author_multi_includes_narrator = False

            if author_multi and narrator:
                narrator_norm = normalize(narrator)
                author_multi_includes_narrator = any(
                    normalize(name) == narrator_norm for name in split_author_names(author)
                )

            if author_multi:
                multi_author += 1
            if author_equals_narrator:
                author_equals_narrator_count += 1
            if author_multi_includes_narrator:
                author_multi_includes_narrator_count += 1
            if narrator_missing:
                missing_narrator += 1

            if author_multi or author_narrator_overlap or author_equals_narrator:
                flagged += 1
                reasons = []
                if author_multi:
                    reasons.append("AUTHOR_MULTI")
                if author_equals_narrator:
                    reasons.append("AUTHOR_EQUALS_NARRATOR")
                if author_multi_includes_narrator:
                    reasons.append("AUTHOR_INCLUDES_NARRATOR")
                if author_narrator_overlap:
                    reasons.append("AUTHOR_NARRATOR_OVERLAP")
                if narrator_missing:
                    reasons.append("NARRATOR_MISSING")
                rows.append(
                    {
                        "path": path,
                        "title": title,
                        "author": author,
                        "narrator": narrator,
                        "author_equals_narrator": str(author_equals_narrator).lower(),
                        "author_multi": str(author_multi).lower(),
                        "author_multi_includes_narrator": str(
                            author_multi_includes_narrator
                        ).lower(),
                        "author_narrator_overlap": str(author_narrator_overlap).lower(),
                        "flag_reason": "|".join(reasons),
                    }
                )

    return (
        total,
        flagged,
        multi_author,
        author_equals_narrator_count,
        author_multi_includes_narrator_count,
        missing_narrator,
        rows,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Scan audiobook tags for author/narrator issues")
    parser.add_argument("--root", required=True, help="Root library path to scan")
    parser.add_argument("--out", required=True, help="CSV output path")
    args = parser.parse_args()

    (
        total,
        flagged,
        multi_author,
        author_equals_narrator_count,
        author_multi_includes_narrator_count,
        missing_narrator,
        rows,
    ) = scan(args.root)

    with open(args.out, "w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=[
                "path",
                "title",
                "author",
                "narrator",
                "author_equals_narrator",
                "author_multi",
                "author_multi_includes_narrator",
                "author_narrator_overlap",
                "flag_reason",
            ],
        )
        writer.writeheader()
        writer.writerows(rows)

    print(f"Audio files scanned: {total}")
    print(f"Flagged (author multi or narrator overlap): {flagged}")
    print(f"Author has multiple names: {multi_author}")
    print(f"Author equals narrator: {author_equals_narrator_count}")
    print(f"Author list includes narrator: {author_multi_includes_narrator_count}")
    print(f"Missing narrator tag: {missing_narrator}")
    print(f"CSV output: {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

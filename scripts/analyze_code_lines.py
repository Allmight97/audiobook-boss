#!/usr/bin/env python3
"""
Minimal scanner: list source files over 400 lines in ./src and ./src-tauri/src.

Keeps only what is needed to spot oversized modules: total line count per file,
sorted descending, with an over/under flag. Skips common build/test output dirs.
"""

import os
from pathlib import Path
from typing import Iterable, List, Tuple


BASE_DIRS: List[Path] = [Path("./src"), Path("./src-tauri/src")]
SKIP_DIRS = {"node_modules", "target", "dist", "coverage", "build", "out", ".git", ".turbo"}
EXTENSIONS = {".rs", ".ts", ".tsx", ".js", ".jsx"}
THRESHOLD = 400


def iter_source_files() -> Iterable[Path]:
    """Yield source files under BASE_DIRS, pruning common junk directories."""
    cwd = Path.cwd()
    for base in BASE_DIRS:
        base_abs = (cwd / base).resolve()
        if not base_abs.exists():
            continue
        for root, dirs, files in os.walk(str(base_abs)):
            dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
            for name in files:
                path = Path(root) / name
                if path.suffix.lower() in EXTENSIONS:
                    yield path


def count_lines(path: Path) -> int:
    try:
        with path.open("r", encoding="utf-8", errors="ignore") as handle:
            return sum(1 for _ in handle)
    except OSError:
        return 0


def main() -> None:
    results: List[Tuple[str, int]] = []
    cwd = Path.cwd()

    for file_path in iter_source_files():
        line_count = count_lines(file_path)
        try:
            rel_path = str(file_path.relative_to(cwd))
        except ValueError:
            # Fallback if paths don't align
            rel_path = str(file_path)
        results.append((rel_path, line_count))

    results.sort(key=lambda item: item[1], reverse=True)

    print(f"Modules over {THRESHOLD} lines (scanned: {', '.join(str(p) for p in BASE_DIRS)})")
    print("=" * 72)
    print(f"{'Module':<50} {'Lines':>7}  Over {THRESHOLD}?")
    print("-" * 72)

    over_count = 0
    for module, lines in results:
        over = "YES" if lines > THRESHOLD else "NO"
        if lines > THRESHOLD:
            over_count += 1
        display_name = module if len(module) <= 50 else "..." + module[-47:]
        print(f"{display_name:<50} {lines:>7}  {over}")

    print("\n" + "=" * 72)
    print(f"SUMMARY: {over_count} modules exceed {THRESHOLD} lines")


if __name__ == "__main__":
    main()


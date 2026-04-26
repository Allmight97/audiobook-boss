#!/usr/bin/env python3
"""
Minimal scanner: list source files over 400 lines in ./src and ./src-tauri/src.

Keeps only what is needed to spot oversized modules: total line count per file,
sorted descending, with an over/under flag. Skips common build/test output dirs.
"""

import os
import re
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


def is_test_file(path: Path) -> bool:
    """Check if a file is a test file based on path patterns."""
    path_str = str(path)
    test_patterns = [
        "__tests__/",
        ".test.ts",
        ".spec.ts",
        ".test.tsx",
        ".spec.tsx",
        "_test.rs",
        "_tests.rs",
    ]
    return any(pattern in path_str for pattern in test_patterns)


def analyze_test_content(path: Path) -> tuple[str, int] | None:
    """Return (test_type, test_percent) for Rust files.
    
    test_type: 'import' for cfg(test) mod name; — points to external file.
               'inline' for actual test code in a cfg(test) module.
    test_percent: percentage of file that is test code (0-100).
    """
    if path.suffix != ".rs":
        return None
    try:
        with path.open("r", encoding="utf-8", errors="ignore") as handle:
            content = handle.read()

        if "#[cfg(test)]" not in content:
            return None

        total_lines = content.count('\n')
        if total_lines == 0:
            return None

        # Check if it's just importing a test module: #[cfg(test)] mod name;
        if re.search(r'#\[cfg\(test\)\]\s*mod\s+\w+\s*;', content):
            return ("import", 0)

        # Find the first #[cfg(test)] marker and measure from there
        match = re.search(r'^#\[cfg\(test\)\]', content, re.MULTILINE)
        if match:
            test_lines = content[match.start():].count('\n')
            pct = round(test_lines / total_lines * 100)
        else:
            pct = 0

        return ("inline", pct)
    except OSError:
        return None


def main() -> None:
    results: List[Tuple[str, int, str | None, int]] = []  # (path, lines, test_type, test_pct)
    cwd = Path.cwd()

    for file_path in iter_source_files():
        line_count = count_lines(file_path)
        test_info = analyze_test_content(file_path)
        if test_info:
            test_type, test_pct = test_info
        else:
            test_type, test_pct = None, 0
        try:
            rel_path = str(file_path.relative_to(cwd))
        except ValueError:
            rel_path = str(file_path)
        results.append((rel_path, line_count, test_type, test_pct))

    results.sort(key=lambda item: item[1], reverse=True)

    # Separate source modules from test files
    source_modules = [(m, l, t, p) for m, l, t, p in results if not is_test_file(Path(m))]
    test_files = [(m, l, t, p) for m, l, t, p in results if is_test_file(Path(m))]

    # Print source modules
    print(f"Source modules >= {THRESHOLD} lines (scanned: {', '.join(str(p) for p in BASE_DIRS)})")
    print("=" * 72)
    print(f"{'Module':<50} {'Lines':>7}  Flag")
    print("-" * 72)

    source_over_count = 0
    for module, lines, test_type, test_pct in source_modules:
        flag = "OVER" if lines >= THRESHOLD else ""
        if test_type == "import":
            flag = f"{flag} [test import]".strip()
        elif test_type == "inline":
            flag = f"{flag} [{test_pct}% tests]".strip()
        if lines >= THRESHOLD:
            source_over_count += 1
        display_name = module if len(module) <= 50 else "..." + module[-47:]
        print(f"{display_name:<50} {lines:>7}  {flag}")

    # Print test files
    print("\n" + "=" * 72)
    print(f"Test files >= {THRESHOLD} lines")
    print("=" * 72)
    print(f"{'Module':<50} {'Lines':>7}  Flag")
    print("-" * 72)

    test_over_count = 0
    for module, lines, test_type, test_pct in test_files:
        flag = "OVER" if lines >= THRESHOLD else ""
        if test_type == "import":
            flag = f"{flag} [test import]".strip()
        elif test_type == "inline":
            flag = f"{flag} [{test_pct}% tests]".strip()
        if lines >= THRESHOLD:
            test_over_count += 1
        display_name = module if len(module) <= 50 else "..." + module[-47:]
        print(f"{display_name:<50} {lines:>7}  {flag}")

    print("\n" + "=" * 72)
    print(f"SUMMARY: {source_over_count} source modules, {test_over_count} test files >= {THRESHOLD} lines")


if __name__ == "__main__":
    main()


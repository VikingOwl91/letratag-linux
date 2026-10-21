#!/usr/bin/env python3
"""Check that relative links in the project documentation resolve.

Only repository-relative markdown links are checked. External URLs, anchor
links and images with an absolute path are ignored. The check is
dependency-free so it can run in CI without extra packages.
"""

import re
import sys
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parent.parent
SKIP_DIRS = {".git", ".research", "target", "test-output", "node_modules"}
LINK = re.compile(r"!?\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")


def markdown_files():
    for path in sorted(ROOT.rglob("*.md")):
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        yield path


def check(path):
    problems = []
    text = path.read_text(encoding="utf-8")
    for number, line in enumerate(text.splitlines(), 1):
        for match in LINK.finditer(line):
            target = match.group(1).strip()
            if not target or target.startswith(("#", "http://", "https://", "mailto:")):
                continue
            if target.startswith("<") and target.endswith(">"):
                target = target[1:-1]
            target = unquote(target.split("#", 1)[0])
            if not target:
                continue
            resolved = (path.parent / target).resolve()
            if resolved.exists():
                continue
            problems.append((number, target, str(path.relative_to(ROOT))))
    return problems


def main():
    problems = []
    for path in markdown_files():
        problems.extend(check(path))

    if problems:
        for number, target, source in problems:
            print(f"{source}:{number}: broken link: {target}")
        print(f"FAIL: {len(problems)} broken documentation link(s)")
        return 1

    print("PASS: all relative documentation links resolve")
    return 0


if __name__ == "__main__":
    sys.exit(main())

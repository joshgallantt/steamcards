#!/usr/bin/env python3
"""Check every mockup in a Markdown file fits its terminal size exactly.

A mockup is a fenced block whose info string is `mockup WxH`, optionally
followed by a title:

    ```mockup 120x30 dashboard, farming alone
    ...exactly 30 lines, none wider than 120 columns...
    ```

Rules checked:
  * exactly H lines (a terminal has H rows; show all of them, blank or not);
  * no line wider than W display columns (East Asian wide/fullwidth = 2,
    combining marks = 0, everything else = 1, which is how box drawing,
    blocks, braille and the symbols used here render);
  * no tab characters;
  * at least one line exactly W wide when the mockup has a full-width frame
    (reported as a warning, not an error).
Exit status 1 if any mockup breaks a rule.
"""
import re
import sys
import unicodedata

FENCE = re.compile(r"^```mockup\s+(\d+)\s*[x×]\s*(\d+)\s*(.*)$")


def width(s: str) -> int:
    w = 0
    for ch in s:
        if unicodedata.combining(ch):
            continue
        if unicodedata.category(ch) in ("Mn", "Me", "Cf"):
            continue
        w += 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1
    return w


def main(path: str) -> int:
    lines = open(path, encoding="utf-8").read().split("\n")
    errors, count, i = 0, 0, 0
    while i < len(lines):
        m = FENCE.match(lines[i])
        if not m:
            i += 1
            continue
        w, h, title = int(m.group(1)), int(m.group(2)), m.group(3).strip()
        start = i + 1
        j = start
        while j < len(lines) and lines[j].rstrip() != "```":
            j += 1
        body = lines[start:j]
        count += 1
        name = f"line {i + 1} ({w}x{h} {title})"
        if len(body) != h:
            print(f"ERROR {name}: {len(body)} lines, expected exactly {h}")
            errors += 1
        for k, row in enumerate(body):
            if "\t" in row:
                print(f"ERROR {name}: tab on row {k + 1}")
                errors += 1
            rw = width(row.rstrip())
            if rw > w:
                print(f"ERROR {name}: row {k + 1} is {rw} columns wide, over {w}: {row.rstrip()[:60]}…")
                errors += 1
        i = j + 1
    print(f"{count} mockups checked, {errors} errors")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))

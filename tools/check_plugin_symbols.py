#!/usr/bin/env python3
"""Every libnickel symbol the plugin relies on must exist in the firmware.

- `.sym = "..."` (nh_hook): hooked through libnickel's GOT, so libnickel must
  define it (D) or import it (U).
- `.name = "..."` (nh_dlsym): resolved with dlsym, so it must be defined (D).

The source differs between the Qt 6 build (firmware 5.x/6.x) and the Qt 5
build (Kobo 4.x) only in `#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)` /
`#if QT_VERSION < QT_VERSION_CHECK(6, 0, 0)` blocks; `--qt` picks the branch
(default 6).

A reference lists `_Z` dynamic symbols of `libnickel.so.1.0.0`, one
"D name" / "U name" per line, gzipped:
- Qt 6: the Tolino build 5.18.264769, every symbol. It comes from firmware,
  so it is not in this repository: CI downloads it from the owner's
  NickelTolinoShopRedirect repository at a fixed commit.
- Qt 5: `tools/libnickel-4.38.23852.syms.gz`, the Kobo build 4.38.23852,
  only the classes the plugin and the Kobo client use (made by
  tools/nickel_symbol_reference.py, see docs/nickel-4.38-symbols.md).

Usage: check_plugin_symbols.py [--qt 5|6] <reference.syms.gz> [plugin source]
"""

import gzip
import re
import sys
from pathlib import Path

SOURCE = Path(__file__).resolve().parent.parent / "plugin" / "src" / "nickelinksync.cc"
# `\s*=` right after the field name excludes `.sym_new = "..."`.
HOOK_RE = re.compile(r'\.sym\s*=\s*"([^"]+)"')
DLSYM_RE = re.compile(r'\.name\s*=\s*"(_Z[^"]+)"')
QT6_IF = re.compile(r"#\s*if\s+QT_VERSION\s*(>=|<)\s*QT_VERSION_CHECK\(\s*6\s*,\s*0\s*,\s*0\s*\)")


def for_qt(src: str, qt: int) -> str:
    """The source as the preprocessor sees it for Qt `qt`, as far as the
    Qt 5/Qt 6 blocks go; other `#if` blocks are kept whole."""
    kept = []
    # One entry per open #if: None for other conditions, else whether the
    # current branch is taken.
    stack: list = []
    for line in src.splitlines():
        stripped = line.strip()
        m = QT6_IF.match(stripped)
        if m:
            stack.append((m.group(1) == ">=") == (qt >= 6))
            continue
        if re.match(r"#\s*if", stripped):
            stack.append(None)
        elif re.match(r"#\s*else", stripped) and stack and stack[-1] is not None:
            stack[-1] = not stack[-1]
            continue
        elif re.match(r"#\s*endif", stripped) and stack:
            if stack.pop() is not None:
                continue
        if all(taken is not False for taken in stack):
            kept.append(line)
    return "\n".join(kept)


def main() -> int:
    args = sys.argv[1:]
    qt = 6
    if args[:1] == ["--qt"] and len(args) > 1 and args[1] in ("5", "6"):
        qt, args = int(args[1]), args[2:]
    if len(args) not in (1, 2):
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2
    reference = Path(args[0])
    firmware = re.search(r"\d+\.\d+(\.\d+)?", reference.name)
    label = f"libnickel {firmware.group(0) if firmware else reference.name}"
    src = for_qt(Path(args[1] if len(args) == 2 else SOURCE).read_text(encoding="utf-8"), qt)
    defined, imported = set(), set()
    with gzip.open(reference, "rt", encoding="utf-8") as f:
        for line in f:
            kind, _, name = line.strip().partition(" ")
            (defined if kind == "D" else imported).add(name)
    hooks, dlsyms = HOOK_RE.findall(src), DLSYM_RE.findall(src)
    errors = [f"hook symbol not in {label}: {s}" for s in hooks if s not in defined | imported]
    errors += [f"dlsym symbol not defined in {label}: {s}" for s in dlsyms if s not in defined]
    if not hooks or not dlsyms:
        errors.append("no hooks or dlsym entries found in the plugin source")
    for e in errors:
        print(e, file=sys.stderr)
    if errors:
        return 1
    print(f"Qt {qt}: {len(hooks)} hooks and {len(dlsyms)} dlsym symbols found in {label}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

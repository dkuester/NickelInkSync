#!/usr/bin/env python3
"""Make a gzipped libnickel symbol reference for tools/check_plugin_symbols.py.

Reads `llvm-nm -D libnickel.so.1.0.0` output and writes the `_Z` symbols of
the classes the plugin and the Kobo client use (CLASSES), one "D name" /
"U name" per line, sorted. Only these classes, not the whole library: the
list stays small and reproducible from the firmware (docs/nickel-4.38-symbols.md).
A symbol of a class not listed here reads as missing; add the class and
make the reference again.

Usage: nickel_symbol_reference.py <nm output> <out.syms.gz>
"""

import gzip
import re
import sys

CLASSES = {
    "AbstractMenuController",
    "AbstractNickelMenuController",
    "ActionProxyMixin",
    "Bookmark",
    "BookmarkSyncManager",
    "ConfirmationDialog",
    "ConfirmationDialogFactory",
    "Content",
    "ContentLocation",
    "DatabaseAdapter",
    "Device",
    "EventEngine",
    "MainWindowController",
    "N3Dialog",
    "N3DialogFactory",
    "N3ReaderOpener",
    "N3ToasterController",
    "ReadingController",
    "ReadingMenuView",
    "ReadingView",
    "Repository",
    "Volume",
    "VolumeManager",
    "WirelessManager",
    "WirelessWorkflowManager",
}
# `_ZN` or `_ZNK`, then the length-prefixed outermost name.
NESTED = re.compile(r"_ZNK?(\d+)")


def outer_class(name: str):
    m = NESTED.match(name)
    if not m:
        return None
    start, length = m.end(), int(m.group(1))
    return name[start : start + length]


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2
    lines = set()
    with open(sys.argv[1], encoding="utf-8") as f:
        for raw in f:
            fields = raw.split()
            if not fields or not fields[-1].startswith("_Z"):
                continue
            name = fields[-1]
            if outer_class(name) in CLASSES:
                lines.add(("U " if fields[0] == "U" else "D ") + name)
    # mtime=0 keeps the file the same for the same input.
    with gzip.GzipFile(sys.argv[2], "wb", compresslevel=9, mtime=0) as out:
        out.write("".join(line + "\n" for line in sorted(lines)).encode("utf-8"))
    print(f"{len(lines)} symbols of {len(CLASSES)} classes written to {sys.argv[2]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

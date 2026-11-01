#!/usr/bin/env python3
"""Every libnickel symbol the plugin relies on must exist in a firmware:
Tolino 5.18 and Kobo 6.0.

- `.sym = "..."` (nh_hook): hooked through libnickel's GOT, so libnickel must
  define it (D) or import it (U).
- `.name = "..."` (nh_dlsym): resolved with dlsym, so it must be defined (D).

The reference lists the `_Z` dynamic symbols of `libnickel.so.1.0.0`, one
"D name" / "U name" per line, gzipped. It comes from firmware, so it is not
in this repository: for the Tolino build 5.18.264769 CI downloads it from
the owner's NickelTolinoShopRedirect repository at a fixed commit, for the
Kobo 6.0.279225 it makes it from Kobo's update (tools/libnickel_symbols.sh).

Usage: check_plugin_symbols.py [--firmware 5.18] <libnickel.syms.gz> [plugin source]
"""

import gzip
import re
import sys
from pathlib import Path

SOURCE = Path(__file__).resolve().parent.parent / "plugin" / "src" / "nickelinksync.cc"
# `\s*=` right after the field name excludes `.sym_new = "..."`.
HOOK_RE = re.compile(r'\.sym\s*=\s*"([^"]+)"')
DLSYM_RE = re.compile(r'\.name\s*=\s*"(_Z[^"]+)"')


def main() -> int:
    args = sys.argv[1:]
    firmware = "5.18"
    if args[:1] == ["--firmware"] and len(args) > 1:
        firmware, args = args[1], args[2:]
    if len(args) not in (1, 2):
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2
    src = Path(args[1] if len(args) == 2 else SOURCE).read_text(encoding="utf-8")
    defined, imported = set(), set()
    with gzip.open(args[0], "rt", encoding="utf-8") as f:
        for line in f:
            kind, _, name = line.strip().partition(" ")
            (defined if kind == "D" else imported).add(name)
    hooks, dlsyms = HOOK_RE.findall(src), DLSYM_RE.findall(src)
    errors = [f"hook symbol not in libnickel {firmware}: {s}" for s in hooks if s not in defined | imported]
    errors += [f"dlsym symbol not defined in libnickel {firmware}: {s}" for s in dlsyms if s not in defined]
    if not hooks or not dlsyms:
        errors.append("no hooks or dlsym entries found in the plugin source")
    for e in errors:
        print(e, file=sys.stderr)
    if errors:
        return 1
    print(f"{len(hooks)} hooks and {len(dlsyms)} dlsym symbols found in libnickel {firmware}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

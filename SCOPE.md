# Scope

**NickelInkSync** syncs the **reading progress of sideloaded ebooks** between
Kobo and Tolino e-readers (and KOReader) through a
[koreader-sync-server](https://github.com/koreader/koreader-sync-server)
(KOSync protocol).

Status: scope fixed, nothing implemented yet. Findings marked *verified* were
checked against real device databases (see [Verified findings](#verified-findings));
everything else is still an assumption.

## Goals

- Push the current position of a sideloaded book from a Kobo or Tolino to a
  KOSync server, and pull a newer position back onto the device.
- Interoperate with KOReader: a position written by NickelInkSync opens at the
  same place in KOReader, and the other way round.
- One code base for both device families. The Tolino Shine 5, Shine Color and
  Vision Color are built by Kobo and run Nickel with the same
  `.kobo/KoboReader.sqlite`, so they are treated as Kobo devices with a
  Tolino flag, not as a separate target.

## Supported

| | |
|---|---|
| Devices | Kobo on firmware 4.x (Qt5) and 6.x (Qt6, not checked yet), Tolino Shine 5, Shine Color and Vision Color on firmware 5.x (Qt6) |
| Formats | kepub (`.kepub.epub`) and plain EPUB |
| Books | Sideloaded only (`ContentID` starting with `file:///mnt/onboard/`) |
| Server | Any KOSync-compatible server (koreader-sync-server, self-hosted forks) |

Both firmware lines are needed: the reference Kobo (Clara BW) ran 4.46, the
reference Tolino (Shine Color) runs 5.18. The Rust core is unaffected; the
NickelHook plugin is built twice (Qt5 and Qt6).

On 2026-10-28 the reference Clara BW updated itself to firmware
**6.0.279225** (database version 222). There is no 4.x device left: the
4.46 findings below stay verified, but 4.x-only behaviour can be checked
against the fixtures only, and the Kobo pass targets 6.0 (see
[Open technical questions](#open-technical-questions)).

## Book identity

KOSync identifies a book by a hash of its file. **First version: the user puts
the identical file on every device**, and NickelInkSync uses KOReader's default
"binary" hash (MD5 over 1 KiB samples at offsets 0, 1024, 4096, 16384, …
up to 1 GiB, stopping at the end of the file; checked against the
`partial_md5_checksum` KOReader stored for every EPUB fixture). A kepub on one device and the source
EPUB on another are different books to the server.

**Manual mapping** (D1, in progress): a book on a device whose file differs
from the original EPUB is linked to that EPUB, and synced under the EPUB's
checksum. Concept as in
[BookOrbit](https://github.com/bookorbit/bookorbit)
([`docs/KOREADER_HASH_MATCHING.md`](https://github.com/bookorbit/bookorbit/blob/main/docs/KOREADER_HASH_MATCHING.md):
manual links take precedence over automatic hash matches; it also has a
position converter for Kobo spans, XPointer and CFI,
`server/src/modules/position-converter/`), re-implemented, no code taken.

| | Key | Why |
|---|---|---|
| Kobo, Tolino (kepub or EPUB) | partial MD5 of the file on the device | what the sync computes anyway; independent of mount point, folder and file name, so moving or renaming keeps the mapping, and one file on two devices shares it. A re-converted file has a new checksum and loses its mapping: positions in the old file may not fit the new one. |
| Kindle (KFX) | `content_id` from the KFX metadata | nothing in the KFX leads to the EPUB checksum; `title` is the file stem it was sent under and need not be unique; the KFX is rewritten by Amazon, the `content_id` stays with the document. A new conversion shows up anyway: the stored position no longer fits the KFX's position maps. |
| Original EPUB | path and its partial MD5 at the time of mapping | the checksum is what KOReader and the server use; if the file is missing or its checksum changed, the mapping is reported as broken instead of syncing under a checksum KOReader no longer has. |

Path, title and author of the device book are kept for display only. The
mappings live in `mappings.json` in the data directory
(`$NICKELINKSYNC_DATA_DIR`, else `$XDG_DATA_HOME/nickelinksync`, else
`~/.local/share/nickelinksync`), human-readable JSON with a `format`
version, written atomically (temporary file, then rename), shared by the
CLI and the desktop client (D2); a file with a newer format is refused,
not rewritten. Commands: `nickelinksync map add|list|remove`
(`src/mapping.rs`, `src/kindle_device.rs`).

**Library of originals** (`src/library.rs`): `library` in `config.ini` or
`--library`. A folder with a Calibre `metadata.db` is read through that
database only, opened as `file:…?mode=ro&immutable=1` (no journal, no
lock, nothing written), joining `books`, `data` (format `EPUB`; the file is
`<books.path>/<data.name>.epub`) and `authors` via `books_authors_link`;
any other folder is searched for `*.epub` (not `*.kepub.epub`) with title
and authors from the OPF. **`map suggest`** (`src/suggest.rs`) ranks the
library against a device book by title (word overlap with the EPUB's title
or file name: Send-to-Kindle sets the KFX `title` to the stem of the sent
file), authors and, for the five best, chapters (spine paths, or KFX
sections against spine items), and checks their checksums: a library EPUB
with the device file's checksum needs no mapping. The Kindle's
`content_id` is the key of its mapping and how `--content-id` picks the
book; it does not appear in the library, so it adds nothing to the score.

**Syncing through the mapping** (`src/transfer.rs`, `status`/`push`/`pull`
on the desktop; on the device through tables, below): a mapped book syncs
under the original's checksum; a broken mapping, and with a library
configured a book that is neither mapped, nor in the library, nor the
original of a mapping, does not sync at all; without a library every
unmapped book syncs under its own checksum as before. The position moves
between the two files as a KOReader XPointer, by chapter and text, never
by spine index or node path:

- **Chapter:** the spine item's path, with calibre's `_split_NNN` taken
  off, so the parts of a split chapter are one chapter. Calibre's Kobo
  driver (EPUB → kepub on copy, owner's library on the Kobo 6.0,
  2026-10-28) replaced the cover page by `titlepage.xhtml` in all three
  books and split two chapters of one into `…_split_000`/`_split_001`
  (34 spine items against 27).
- **Text:** the chapter's visible characters in document order, without
  whitespace, soft hyphens and zero-width characters (a kepub adds
  wrapper `div`s, spans and whitespace between elements). Same text: the
  same character (*same text*). Otherwise the 32 characters after the
  position (before it, at a chapter end) are looked up, nearest to the
  same relative place (*found by its text*).
- **Fallbacks, shown:** the chapter's start if the text is not found; if
  the chapter is not in the other file, the place at the book's
  percentage, counted in visible characters (*from the percentage only*).

Checked: KOReader's highlights of the same 16 words in the kepub and the
EPUB of Book P and Book Q move to each other exactly, both ways; the
Kobo 6.0's `#kobo.17.1:204` in the Book R kepub lands on
`p[16]/text()[1].204` of the EPUB; Book A's `#kobo.1.1` is KOReader's own
position in the EPUB; a calibre-like conversion of Book R (cover page,
split chapter, changed text) moves both ways (`tests/transfer.rs`,
`tests/sync_mapping.rs`). `pull --write` translates back into a mapped
EPUB on the Tolino and into a mapped kepub on a Kobo 6.x (as a span with
offset, `position::from_xpointer_kepub`). A kepub syncs only through a
mapping, never under its own checksum.

**Mappings on the device** (C3, `src/prepare.rs`, `src/table.rs`):
`device prepare` puts the library index (`library.json`: checksum, title,
authors of each original), the mappings of the books on the device
(`mappings.json`, plus `added`/`removed` for mappings made on the device,
taken over by checksum) and one translation table per mapped book
(`tables/<device checksum>.json`) into `.adds/NickelInkSync/`; never an
EPUB or its text. A table holds per original spine item the text nodes
(KOReader path, visible start and length, collapsed length, paragraph
start, and the collapsed offset of the first character and of each
sentence start), the sentence starts, and for chapters whose text
differs pairs of (device, original) indices found at build time. That is
what `transfer` and `step_back` need for `position = paragraph` and
`sentence`: the device sends exactly what the desktop sends
(`tests/table.rs` compares both ways for Book A, Book R and a
calibre-like conversion). Other places (`exact`, the server's position
on the way back) are put between two anchors in proportion. `--live`
reads these files; without `library.json`, `push` and `sync --pending`
send nothing (`NICKELINKSYNC_NO_INDEX`), and only an original from the
index or a book with a mapping and table is sent.

**Place to send** (`position`, `transfer::step_back`): a device stores
its page start, often mid-sentence; KOReader opens the page holding that
place by its own layout, so it may sit anywhere on the page (Vampire Hunter
D on the Kobo 6.0, 2026-10-28: the Kobo's page started at "the events of
the day", which KOReader showed near the bottom of its page, though the
XPointer was exact). By default `push` sends the start of the innermost
block element (paragraph) instead, at most 600 visible characters back,
else the sentence start (after `.`, `!`, `?`, `…` and closing quotes),
else the place itself; `sentence` and `exact` are the other settings.
The local position used for `status` and `confirm` is the same moved one,
so the device does not look newer because of it.

## License

NickelInkSync is MIT-licensed. KOReader, koreader-sync-server and BookOrbit are
AGPL-3.0, so their code is a reference only: re-implement the documented
behaviour (KOSync protocol, partial MD5, XPointer format), never copy or
translate their source. NickelHook is MIT and can be used as is.

## Out of scope

- Kobo Store, Tolino/Thalia store and Onleihe books, and anything with DRM
- PDF, CBZ and other non-EPUB formats
- Highlights, notes and bookmarks (position only)
- Reading statistics
- A sync server of our own: NickelInkSync is a client only

## Architecture

```
┌──────────────── Kobo / Tolino ────────────────┐
│ Nickel ── NickelHook plugin (C++, Qt5 / Qt6)   │
│              │  book opened / closed           │
│              ▼                                 │
│        nickelinksync CLI (Rust, ARM) ──────────┼──► KOSync server ◄── KOReader
└────────────────────────────────────────────────┘          ▲
                                                            │
  Desktop: nickelinksync CLI (Rust) on a USB-mounted device ┘
```

### Rust core and CLI

One crate, built for the desktop (macOS, Linux, Windows) and cross-compiled
for the device. It owns all logic:

- **Device database access**: read positions from `KoboReader.sqlite`, opened
  read-only unless a write is explicitly requested.
- **Document identity**: compute the KOSync document hash for a book file.
- **Position translation**: Nickel position ↔ KOReader XPointer, using the
  EPUB itself (OPF spine, chapter files, Kobo span IDs, Adobe point paths).
- **KOSync client**: `users/auth`, `GET /syncs/progress/:document`,
  `PUT /syncs/progress`.
- **Conflict rule**: the newer side wins (device `DateLastRead` against the
  server's timestamp), with a confirmation set by `confirm` in the
  configuration: `never`, `backwards` (default: ask when the position that
  would be overwritten lies further on in the book than the new one) or
  `always` (ask whenever the positions differ). "Backwards" compares the
  place in the book, never the timestamps: spine index (`DocFragment[n]`)
  first, then, in the same chapter, the place in the text (paragraph, then
  offset, where the two XPointer paths can be compared), and the
  percentage only where neither decides. Each reader computes its own
  percentage: on the Tolino, Nickel's page start in R2-133 said 57 % while
  KOReader's place further on in R2-134 said 56 %. Not by time: Nickel sets `DateLastRead` when a book
  is merely opened, so newest-wins alone would let a device that was only
  opened overwrite a position read further on in KOReader. In a terminal
  the CLI asks "device or server?"; without one (the plugin, later) it
  overwrites nothing and reports the book. `--force` skips the question.

The run itself (open the device and the server, then one step per book)
is in the library too (`session`), not in the CLI: a step says what
`push` or `pull` would do, and a question from `confirm` is part of it
(`Action::Ask`). The CLI asks in the terminal, the desktop client in a
dialog; both apply the same steps.

Desktop use: `nickelinksync push|pull|status --device /Volumes/KOBOeReader`.
On-device use: the same binary, called by the hook.

Device build: a statically linked `armv7-unknown-linux-musleabihf` binary
(`cargo-zigbuild`), independent of the device glibc, so one file serves Kobo
firmware 4.x and Tolino firmware 5.x. HTTPS to the KOSync server must use
`rustls`, not OpenSSL: no system library to link against or ship, and nothing
that breaks the static build. C dependencies such as SQLite are compiled in
(`rusqlite` with `bundled`).

### NickelHook plugin

Thin C++ layer, no sync logic of its own:

- On leaving a book (`ReadingView::leavingReadingExperience`, after Nickel
  has saved the position): call the CLI to push, in the background.
- On opening a book (`ReadingController::setVolume`), before Nickel loads
  it: call the CLI to pull (`pull --live --write --emit`, time-limited),
  which writes a position to take into the book's row, and hand Nickel a
  copy of the `Volume` with the same position. Nickel takes the chapter
  from the `Volume` and the place in it from the row it loads again; both
  have to agree. Changing only what Nickel holds in memory, or jumping with
  `ReadingView::goToBookmark`, did not work on the Tolino
  ([docs/nickel-5.18-symbols.md](docs/nickel-5.18-symbols.md)).
- While Nickel runs the CLI reads the database with `--live` (`mode=ro`,
  so it sees what is still in `-wal`). It writes only with `--write` and
  `--content-id`, for the one book about to be opened, through SQLite's
  normal locking, leaving Nickel's WAL alone, after a backup made with
  SQLite's backup API (one a day, the two newest kept).
- A book opened without a pull (no network, or Nickel did not open the
  pulled chapter) is not pushed when it is left: its device position
  would overwrite a newer one on the server.
- Kobalt (MIT, a reader inside Nickel that writes positions itself and,
  on its development branch, syncs with KOSync itself): while it is
  installed the plugin only calls the originals and the CLI refuses
  `--live` (`NICKELINKSYNC_ERROR` kind `kobalt`) and any `--write`; over
  USB `push` only warns. Installed means: `libkobalt.so` in
  `/usr/local/Kobo/imageformats/` (or `libkobalt.so.failsafe` one folder
  up, where NickelHook moves it for the first seconds of a start; not
  visible over USB), `.adds/kobalt/uninstall` present and
  `.adds/kobalt/uninstall-now` absent, Kobalt's own uninstall flags. The
  rule is in `src/kobalt.rs`, the plugin repeats its paths (a test keeps
  them equal); `.adds/NickelInkSync/ignore-kobalt` overrides it for tests.
  The plugin file stays and works again once Kobalt is gone.
- Settings (server URL, user, device name) in `.adds/NickelInkSync/config.ini`,
  in the same format as the desktop configuration (see the README), passed
  to the CLI with `--config`; the CLI itself lives next to it.
- Packaging: `KoboRoot.tgz` for 4.x, `Kobo.tgz` for 5.x (hook in `imageformats/`).
  Built for 5.x (Qt 6.5.2) in `ghcr.io/dkuester/nickelstorygraph-toolchain`;
  the Qt 5 image `strayrose/nickeltc` is for the 4.x build.

## Verified findings

Reference devices, both read from a copy of the database, never written:

- **Kobo Clara BW**, firmware 4.46.23836, database version 176, two sideloaded kepubs
- **Tolino Shine Color**, firmware 5.18.264769, database version 220, two sideloaded plain EPUBs
- Both again after loading the placeholder EPUB fixtures (a kepub and two plain
  EPUBs), plus **KOReader 2026.07** (macOS) on the same files

| Question | Kobo 4.46 | Tolino 5.18 |
|---|---|---|
| Book row | `ContentType = 6`, `ContentID = file:///mnt/onboard/<path>` | same |
| MimeType | `application/x-kobo-epub+zip` (kepub), `application/epub+zip` (plain EPUB) | `application/epub+zip`, **also for a `.kepub.epub`** |
| Chapter rows | kepub: `ContentType = 9`, one per spine item, `ContentID = /mnt/onboard/<path>!<opf dir>!<href>`; plus `ContentType = 899`, one per ToC entry. Plain EPUB: `ContentType = 9`, one per **ToC entry**, `ContentID = file:///mnt/onboard/<path>#(<ToC index>)<target>`, target in `adobe_location` | `ContentType = 9`, one per **ToC entry** (no spine rows), `ContentID` is a **UUID**, target in `adobe_location` |
| Chapter → book | `BookID` = book `ContentID`, `VolumeIndex` = spine index (kepub, type 9), ToC index (plain EPUB) | `BookID` = book `ContentID`, `VolumeIndex` = **ToC index**, not spine index |
| Position field | `ChapterIDBookmarked`, `___PercentRead` (integer percent) | same |
| Position format | `<href>#kobo.N.M` (kepub span ID), `<href>#point(...)` (plain EPUB); the fragment can be missing (`<href>` only) | `<href>#point(/1/2/1/1:0)` (Adobe point), also for a kepub |
| `adobe_location` (book row) | empty, even for plain EPUB | empty, even for plain EPUB |
| Extra tables | — | `tolino_*` (cloud, DRM, Skoobe); `BookReader.sqlite` is encrypted and not needed |

In the chapter-row `ContentID`, `<href>` is relative to the OPF directory;
the chapter path in `ChapterIDBookmarked` and `adobe_location` is relative to
the EPUB root (`OEBPS/xhtml/…`). Never rely on the chapter-row `ContentID`
format: it differs between firmware lines. On the Kobo, `BookID` +
`VolumeIndex` of the `ContentType = 9` rows gives the spine item. On the
Tolino it does not: the rows are ToC entries (`NumShortcovers` counts them),
so in *Heat 2* `xhtml-0-81.xhtml` (spine index 81) has `VolumeIndex` 80, and
the *Titan 2* position lies in a spine item without a ToC entry and so
without a row. A plain EPUB on the Kobo 4.46 behaves the same way. Find the
chapter through the OPF spine there. All cases are in the
[test fixtures](tests/fixtures/README.md).

The fragment of `ChapterIDBookmarked` is optional: the Kobo stores the bare
chapter path at the start of a plain-EPUB chapter and on kepub pages without
`koboSpan` spans.

**Kepub span → XPointer** (Kobo): `OEBPS/xhtml/xhtml-0-81.xhtml#kobo.1.172`
→ spine index 81 → KOReader `DocFragment[82]`; span `id="kobo.1.172"` exists
in the chapter file. Resolve the span by its ID; don't derive
paragraph/sentence from `N.M` (calibre's kepub output numbers differently from
kepubify).

**Adobe point → DOM node** (Tolino): `point(/a/b/c…:offset)` is a path of
1-based indices over **all** child nodes, whitespace text nodes included,
where `/1` is the `html` element; `offset` is a **UTF-8 byte offset** into
the final text node (a no-break space or soft hyphen counts 2, so does every
umlaut), with whitespace counted as is. Comments differ by firmware: Kobo
4.46 skips them and joins the text around them, Tolino 5.18 counts them as
nodes. Checked on two books (`/1/2/1/1:0` hits the chapter heading in
*Heat 2*, `/1/4/16/1:356` mid-paragraph text in *Star Trek Titan 2*) and with
highlights in the probe book Book P on both devices.

**Offsets inside a text node** (highlights of the same eight words in the
probe books Book P and Book Q on every device, see the
[test fixtures](tests/fixtures/README.md)): node paths agree
everywhere; only the offsets need converting.

| | Unit | Whitespace | Comment |
|---|---|---|---|
| Kobo span (`StartOffset`) | UTF-16 units | as is | — (spans split there) |
| Adobe point, Kobo 4.46 | UTF-8 bytes | as is | skipped, text joined |
| Adobe point, Tolino 5.18 | UTF-8 bytes | as is | counted as a node |
| KOReader XPointer | code points | collapsed (`\n    ` = 1, leading spaces = 1) | splits `text()` |
| KOReader CFI | UTF-16 units | collapsed | skipped, text joined |

**XPointer → Adobe point** (back to the Tolino, for `pull`): walk the
XPointer below `DocFragment[n]` through spine item *n*−1 (elements by
tag name, `text()[m]` among text and CDATA nodes, comments skipped), then
count all child nodes from `/1` = `html` for the point. KOReader's offset
counts a run of whitespace as one character; going back, an offset after
such a run lands behind the whole run, on the next visible character. A
path that ends on an element, or does not resolve, gives the first
non-whitespace text (of the element, or of the chapter). All 16 Tolino
highlights in Book P and Book Q come back unchanged, KOReader's highlights
of the same words give exactly the Tolino's points, and so do the Book R
reading positions. Back into a kepub (`position::from_xpointer_kepub`): the text node
the XPointer names, its `koboSpan` ancestor, and the UTF-16 units of the
span's text before the place, stored as `#kobo.N.M:offset` as Kobo 6.0
does. A place outside every span (whitespace between paragraphs) moves to
the next span. All 16 Kobo kepub highlights in Book P and Book Q come back
unchanged, KOReader's highlights of the same words give exactly the
Kobo's spans, and so do the Book R reading positions. Not done yet: the
4.x point that joins text around comments, and the 4.x kepub span (no 4.x
device left to check either).

CDATA behaves like a comment in the Adobe point (skipped on Kobo 4.46, a
node on Tolino 5.18), is a `text()` node of its own in KOReader's XPointer,
and is joined into the text by KOReader's CFI. A character outside the BMP
(`𝄞`) is 4 in the Adobe point, 2 in a Kobo span and KOReader's CFI, and 1 in
KOReader's XPointer; a combining accent is one character everywhere.

**KOReader positions** (KOReader 2026.07, `cre_dom_version` 20260812):
`metadata.epub.lua` holds `last_xpointer` and also `last_epubcfi`. Spine
index *i* is `DocFragment[i+1]` and CFI step `/6/(2i+2)`, for kepub and plain
EPUB. At the start of the same chapter heading:

| Device | Stored | KOReader |
|---|---|---|
| Kobo, kepub | `xhtml-0-81.xhtml#kobo.1.1` | `/body[1]/DocFragment[82]/body[1]/div[1]/div[1]/div[1]/span[1]/text()[1].0` |
| Tolino, plain EPUB | `xhtml-0-81.xhtml#point(/1/2/1/1:0)` | `/body[1]/DocFragment[82]/body[1]/div[1]/text()[1].0` |

KOReader keeps the kepub wrappers `div#book-columns` / `div#book-inner` and
the `koboSpan` spans as elements, so a kepub and its source EPUB give
different XPointers for the same text. Element indices match the source DOM.
Positions taken by paging (end of Book C) are page starts and differ between
renderers; only chapter starts line up exactly.

**Kobo firmware 6.0** (Clara BW, 6.0.279225, database version 222, Book R
as kepub and EPUB, read 2026-10-28): the plain EPUB stores
`chapter3.xhtml#point(/1/4/66/1:0)`, the Adobe point as on 4.46 and the
Tolino, exactly the start of `R3-32` at the top of the page. The kepub
stores **the offset into the span** as well:
`chapter2.xhtml#kobo.17.1:204`, where 4.46 stored `#kobo.N.M` only. Span
`kobo.17.1` is `R2-16`, and character 204 of its text is the first word
on the page ("alt See hören …", after umlauts, so characters, not UTF-8
bytes; UTF-16 units are assumed, as in Kobo highlights). Chapter rows
(`ContentType = 9`) have UUID `ContentID`s for the kepub too, as on the
Tolino. The database is in WAL mode. Not checked yet: comments and CDATA
in the 6.0 Adobe point, and highlights.

**Store vs. sideloaded**: store books have UUID `ContentID`s on both
firmware lines; filtering on `file:///mnt/onboard/` is enough.

**Cross-device sample**: *Heat 2* is on both reference devices, but as two
different files (calibre kepub on the Kobo, a converted EPUB on the Tolino).
Both have 109 spine items and the same chapter hrefs, yet different hashes:
exactly the case the later manual mapping is for.

## Write-back over USB

`pull --write` (Tolino so far) changes only the book row
(`ContentType = 6`) of each pulled book, all books in one transaction,
after a byte-for-byte checked backup copy of the database on the desktop:

| Column | Set to |
|---|---|
| `ChapterIDBookmarked` | the server XPointer as an Adobe point (`href#point(/1/…:offset)`) |
| `___PercentRead` | the server percentage, rounded to whole percent |
| `DateLastRead` | unchanged, unless it is newer than the server position: then one second before the server's timestamp |

`DateLastRead`: the device has not been read since, so a fresh "now"
would be wrong, and it would make the device look newer than the server.
If the stored point then does not round-trip exactly (a chapter-only
fallback), the next `push` would send it back as a new position. Keeping
the date behind the server's timestamp keeps the server the newer side:
`push` leaves it alone and `pull` finds nothing to change. When `--force`
or the answer "server" takes the server position although the device was
read later, the date is moved to one second before the server's timestamp
for the same reason. Opening the book sets it again anyway.

The Tolino database is in **WAL mode** (file format bytes 18–19 are 2),
which `immutable=1` hides (`journal_mode` reports `delete` there). The
write keeps WAL mode, takes an exclusive lock first so SQLite needs no
`-shm` file on the FAT file system, and checkpoints after the commit so
the change is in `KoboReader.sqlite` itself; it refuses to start while a
`-journal`, `-wal` or `-shm` file exists and fails if one is left after.
Checked against a copy of the Shine Color database: integrity check
passes, no extra files, WAL mode kept.

Not changed: `CurrentChapterProgress` (position within the chapter, used
for the chapter estimate), `ReadStatus`, and `.kobo/BookReader.sqlite`,
which is encrypted. If Nickel ignores the new position, these are the
first suspects.

**Device test** (Shine Color 5.18, Book R, 2026-10-24, *verified*): KOReader
left Book R on a page starting near the end of R2-134 and synced
(`p[134]/text()[1].256`, 56 %). `pull --write` stored
`chapter2.xhtml#point(/1/4/270/1:269)` and 56 %; no `-wal`, `-shm` or
`-journal` file was left. After eject, Nickel opened Book R on the page
that holds that point (its top in R2-133) and, when the book was closed,
stored its own page start, `#point(/1/4/268/1:46)` (R2-133, 57 %), with
`DateLastRead` set to the time of opening and `CurrentChapterProgress`
updated. So Nickel takes the written position and does not overwrite it
with the old one; it snaps to the start of the page it renders, which can
lie up to a page before the written point. The database stays in WAL mode.

**Plugin on the device** (Shine Color 5.18, *verified* 2026-10-24): the
same holds while Nickel runs, if the row is written just before Nickel
opens the book (`pull --live --write`, SQLite's normal locking, one backup
a day with SQLite's backup API): Nickel opened Book R on the page with
the pulled point. Changing only what Nickel holds in memory (the
`Volume`s it opens from) moved the chapter but not the place in it, and
`ReadingView::goToBookmark` did not move the open book at all; the
renderer is TENG (see
[docs/nickel-5.18-symbols.md](docs/nickel-5.18-symbols.md)).

macOS left an AppleDouble file `._KoboReader.sqlite` next to the database
after the write; it was gone after Nickel had run, and Nickel did not
mind it.

## Kindle (feasibility check, outside scope)

Checked on a **Kindle Paperwhite 11 (PW5), firmware 5.19.2** (jailbroken,
but everything below needs only the files under `documents/`, so it holds
for a stock Kindle), with Book A, C, P, Q and R
sent through **Send-to-Kindle** (2026-10-24). Read from copies only; the
format was decoded with our own reader, no Amazon or calibre code.
Answer: **reading the position over USB and mapping it to KOReader works,
character-exact; the book identity and writing back do not work yet.**

| Question | Kindle 5.19.2, Send-to-Kindle EPUB |
|---|---|
| File | `documents/Downloads/Items01/<file stem>_<32 hex>.kfx`, KFX container (`CONT` v2), **no DRM**, `cde_content_type` PDOC, `YJConversionTools` 2.15.0 |
| Book metadata | `title` = **file stem** (`book-p`, not `dc:title`), `author` kept, `content_id` = `ASIN` = the 32 hex digits; `book_id` is not the EPUB's `dc:identifier` |
| Position store | `.sdr/<name><device hash>.yjf` (KRDS key/value file), visible over USB |
| Keys | `lpr` (last page read, with a millisecond timestamp), `fpr`, `page.history.record` (earlier pages, timestamped) |
| Highlights | `.sdr/….yjr`, `annotation.personal.highlight` with start and end position |
| Position format | `base64(01, eid u32le, offset u32le):pid`, e.g. `AScAAADBAAAA:49698` |
| `cc.db` | not on the USB volume (root only); not needed for the position |

**Position → text.** The KFX holds the text as Ion data: one string per
block (`$145` content), blocks with an `eid` in storylines (`$259`), one
storyline per section (`$260`), one section per spine item. `eid` + offset
name a character in that block's string; `pid` is the linear position in the
book, and the book's own position maps (`$265` section ranges, `$609`
per-section runs) give exactly the stored `pid` for every position and
highlight start we read (29 in five books). Sections follow the spine, so
section *n* is `DocFragment[n+1]`, and the blocks of a section are the
EPUB's block elements in order (Book R: `eid` 361 is `R2-99`, `p[99]` of
`chapter2.xhtml`).

**Layout details** (as read by `src/kfx.rs` and `src/krds.rs`; symbol
numbers from the shared `YJ_symbols` table, book-local symbols such as
section names stay numeric):

| Entity / file | Layout |
|---|---|
| container | `CONT`, u16 version 2, u32 header length, u32 offset and length of the container info (Ion struct: `$413`/`$414` = entity index); index entries of 24 bytes: u32 entity id, u32 type, u64 offset (after the header), u64 length; each entity `ENTY`, u16 version, u32 header length, then Ion values |
| `$145` content | `{$4: name, $146: [string, …]}`; a block points to one string with `$145: {$4: name, $403: index}` |
| `$259` storyline | `{$176: name, $146: [{$155: eid, $145: text, $142: [{$143: offset, $144: length}], $146: nested}]}` |
| `$260` section | `{$174: name, $141: [{$155: eid of the page template, $176: storyline}]}` |
| `$538` document data | `$169: [{$170: [section, …]}]`, the reading order = spine order |
| `$264` position map | per section the `eid`s it holds; `[first, count]` is a range (`[6, 8]` = 6 to 13) |
| `$265` position ID map | per section `$184` first `pid`, `$144` number of positions |
| `$609` section position IDs | runs as `[pid delta, eid]`, or a bare `pid delta` for the next `eid` (previous + 1); the last run, `eid` 0, is the section end. Block `eid` 6 of Book P starts at `pid` 48 + 12 |
| `.yjf`, `.yjr` (KRDS) | magic `00 00 00 00 00 1A B1 26`, `long` version 1, `int` number of top-level objects; values typed by one byte (`00`/`07` byte, `01` int BE, `02` long BE, `03` string: flag 1 = absent, else u16 BE length + UTF-8, `04` double), objects `FE` + name … `FF`, nested |
| `lpr` | byte 2, position, timestamp (ms); `fpr` the same position without a time |
| `annotation.personal.highlight` | start, **end = last highlighted character** (inclusive), created, modified, `0\ufffc0`; inside `annotation.cache.object` → `saved.avl.interval.tree`, not in time order |

The page template of a section has its own `eid` (one position at `pid`
0 of the section, before the first block). The lost CDATA text looks like
an HTML parser at work: `<![CDATA[` read as a bogus comment that ends at
the first `>` (after `<q5w03`), leaving `]]>` as text.

**Offsets** (highlights of the same words as on Kobo and Tolino, see the
[test fixtures](tests/fixtures/README.md#kindle-kfx-via-send-to-kindle)):
**code points** (`𝄞` = 1, a combining accent = 1) in the block's text
**after the conversion**, which differs from the EPUB source:

| Source | In the KFX text | Kindle offset vs. KOReader XPointer |
|---|---|---|
| whitespace runs, line breaks, tabs | one space | same |
| leading whitespace of a block | **dropped** | one less (q6w03: 12, KOReader 13) |
| no-break space | plain space | same (one character) |
| soft hyphen | kept | same |
| inline element (`<em>`) | flattened into the block, with a style run | counted from the block start, not from the `em` text (p5w03: 12, KOReader `em[1]/text()[1].6`) |
| comment | dropped, text joined | one `text()` instead of two (p7w03: 12, KOReader `text()[2].7`) |
| CDATA | **text lost**: `<![CDATA[q5w02 <q5w03>]]>` became `]]>` | joined, and the block text differs from the EPUB |

The stored reading position (`lpr`) is the **first character of the page**,
to the character: Book R gave `R2-99` offset 192 ("Häuser Schatten
Straße") and `R3-85` offset 187 ("Stuhl Garten Seite"), exactly the words
the owner read at the top of the page; the same offsets in the EPUB's
collapsed paragraph text give the KOReader XPointer
(`…/DocFragment[3]/body[1]/p[99]/text()[1].192`). The Kindle merges two
adjacent highlights into one (p5w03 + p5w04).

What it would take, and what is open:

- **Book identity:** nothing in the KFX or the `.sdr` leads back to the EPUB
  file, so KOReader's partial MD5 cannot be found. Needs the manual book
  mapping (title + author from the KFX, or the 32-hex `content_id`, linked
  to an EPUB hash), or a text comparison with the EPUB.
- **Translation (done, library only):** `src/kfx.rs` reads the container,
  text and position maps, `src/krds.rs` the `.yjf`/`.yjr`, and
  `src/kindle.rs` aligns KFX section *n* block by block with the block
  elements of spine item *n*: the EPUB block's text is converted like the
  Kindle does it (whitespace collapsed, leading whitespace and comments
  dropped, inline elements flattened), each character remembering its
  text node, so the offset is split at inline elements and comments and
  leading whitespace is added back. Where the texts differ (CDATA), the
  offset is matched from the start or the end of the block and the result
  is marked not exact; q5w04 still lands on KOReader's XPointer that way.
  Tests: `tests/kindle.rs`.
- **Writing back** is not checked: the `.yjf` is on the USB volume, but the
  reader also keeps the position in `cc.db` and syncs it through Amazon
  (`sync_lpr`), so a changed `.yjf` may be ignored or overwritten. The
  Kindle also left `.yjr.bad_file` copies next to the `.yjr` files.
- **Jailbreak:** assume users have **none**. The test Kindle is
  jailbroken, but every finding above used only the files under
  `documents/`, which a stock Kindle offers over USB (MTP on recent
  ones), so it holds for a stock Kindle. Any Kindle feature set is split
  in two:

  | | Stock Kindle (default) | Jailbroken Kindle (extra) |
  |---|---|---|
  | Read the position | desktop CLI over USB (a copy, where only MTP is offered), `.yjf` | same, plus `cc.db` (`p_location`, `p_percentFinished`) as root |
  | Book mapping | KFX metadata + manual link to the EPUB hash | same |
  | Write back | desktop CLI over USB, `.yjf` only, if the reader takes it (unchecked) | also `cc.db`, while the framework is stopped |
  | Sync without a cable | no | an on-device daemon or KUAL action (no in-process hook exists on the Kindle) |

  Nothing in the stock feature set may need the jailbreak; the jailbreak
  column only adds to it.
- Only Send-to-Kindle (PDOC KFX) was checked; books sideloaded as AZW3 or
  KFX via calibre store their position in other files (`.azw3f`/`.azw3r`).

**In the CLI** (D1, 2026-10-28): `status` and `push` read a Kindle, or a
copy of its books, and send each mapped KFX's `lpr` under the mapped
EPUB's checksum (`src/kindle_device.rs`, `KindleSession` in
`src/main.rs`); `pull` refuses. Checked on the owner's Paperwhite with
real books sent from the Calibre library:

- **No USB mount:** the Paperwhite (USB product `0x9981`) offers only
  **MTP**, which macOS does not mount. The CLI reads a copy instead: a
  folder with `documents/`, or one with the `.kfx` files and `.sdr`
  folders up to two levels down (OpenMTP copies). Direct MTP access is
  open (libmtp).
- **Store books have DRM:** a book bought in the Kindle store is a
  `DRMION` container (`\xeaDRMION\xee`), its text unreadable; it is
  skipped. Only Send-to-Kindle documents work, and those are separate
  books on the Kindle with their own position.
- **`content_id`** of a real Send-to-Kindle document is 32 base32
  characters (`HJ5YDU…`), not hex; read from the metadata.
- **Embedded fonts** are entities of type `$418` (OpenType data, not
  Ion) and are skipped like images (`$417`).
- **Position runs** in `$609` may be `[pid delta, eid, offset]`: a block
  that goes on after a nested one, from that character.
- **Text:** the KFX keeps **no-break spaces** (before `–` and `…`, in
  dates), where the probe books had plain spaces; both count as a space.
  A `<br/>` is `\n` in the block, a space before it kept, whitespace after
  it dropped (`that <br/>I` → `that \nI`). A paragraph may be split into
  two blocks at `<br/><br/>`.
- **Sections ≠ spine items:** Send-to-Kindle put a **cover section**
  without text in front and **split a chapter** into two sections (Good
  Boy: 16 sections, 14 spine items). With differing counts, a block is
  found by its text in every chapter (the neighbouring blocks deciding
  between equal texts, then the relative place), also as a part of a
  longer EPUB block; a position on the cover is the start of the first
  chapter with text, marked not exact.
- Result: every text block of *Der wiedergefundene Freund* (292 of 292)
  and nearly all of *Good Boy* (1203 of 1207; the rest are blurbs and
  links in the front and back matter, found but not exact) translate
  exactly; the finished book's `lpr` (`pid` 107449) is the last page.

## Open technical questions

1. **Shine 5 and Vision Color.** Assumed to match the Shine Color; not checked.
2. **Whitespace at the end of a block** and other cases the probe books do
   not cover (named entities from a DTD, `xml:space`, `<pre>`), and whether
   KOReader counts a whitespace-only text node in front of the target as a
   `text()` (the converter assumes it does). The offset rules in
   [Verified findings](#verified-findings) hold for everything Book P and
   Book Q test.
3. **Write-back.** Writing the position into the database only works safely
   while Nickel is not running (USB mode); see [Write-back over
   USB](#write-back-over-usb) for whether Nickel takes it. In the plugin
   the database is not written (Nickel writes its own copy back); on 5.18
   the plugin sets the pulled position in the `Volume` passed to
   `setVolume` and in the one `VolumeManager::getById` loads while opening
   ([docs/nickel-5.18-symbols.md](docs/nickel-5.18-symbols.md)). The Qt5
   symbols for 4.46 are still open, and on hold without a 4.x device.
4. **Kobo firmware 6.0.** Not checked yet: the position format (kepub
   span, and whether the Adobe point counts comments and CDATA as nodes
   like the Tolino; the CLI assumes so from firmware 5 on), the chapter
   rows, and the plugin symbols. Reference device: Clara BW, 6.0.279225.

## Test data still needed

- Optional: a database from a Shine 5 or Vision Color

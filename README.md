<div align="center">

# NickelInkSync

**Sync the reading progress of sideloaded ebooks between Kobo, Tolino and KOReader.**

[Scope](SCOPE.md) · [Roadmap](ROADMAP.md) · [Deutsch](README.de.md)

</div>

NickelInkSync keeps your place in a sideloaded book in step across e-readers.
It talks to a [koreader-sync-server](https://github.com/koreader/koreader-sync-server)
(the KOSync protocol), so Kobo, Tolino and KOReader devices can all share
the same progress.

> [!IMPORTANT]
> **Status:** early development. With a USB-mounted Kobo or Tolino,
> `nickelinksync push` sends the reading position of each sideloaded book
> to a KOSync server, `status` shows the device and server positions side by
> side, and `pull` shows what a newer server position would change; on a
> Tolino, `pull --write` writes it to the device (experimental, with a
> backup first). The scope and the device-database findings it
> rests on are in [SCOPE.md](SCOPE.md), the next steps are in
> [ROADMAP.md](ROADMAP.md).

## Usage

```sh
nickelinksync status --device /Volumes/KOBOeReader
nickelinksync push --device /Volumes/KOBOeReader
nickelinksync pull --device /Volumes/KOBOeReader
```

| Command | What it does |
|---|---|
| `status` | Lists the sideloaded books with hash, stored position, KOReader XPointer and, if configured, the position on the server. |
| `push` | Sends each book's position to the server. A position the server got after the book was last read on the device is never overwritten; by default it also asks before moving the server back to an earlier place in the book (see `confirm` below). `--force` sends without asking, `--dry-run` only shows what would be sent. |
| `pull` | Shows which books have a newer position on the server, how the device would store it and where it would ask first. Only with `--write` is it written to the device (Tolino so far, experimental, see below); `--force` takes the server position without asking. |

All three take `--book <text>` to limit them to books whose title or file
path contains the text, `--config <file>` for another configuration
file and `--library <folder>` for the library of originals (see [Book
mapping](#book-mapping-in-progress)), which decides which books are
synced. The device is only read, never written, except by `pull --write`
(and by the plugin, see below).

`pull --write` first copies `.kobo/KoboReader.sqlite` to
`~/.local/share/nickelinksync/backups/KoboReader-<UTC time>.sqlite` (or
`backups` in the data directory, see below, or `$NICKELINKSYNC_BACKUP_DIR`),
then sets the position of all books in one transaction. It refuses to
write without `.kobo/version` (not a mounted device) or while a SQLite
rollback journal (`-journal`) lies next to the database. The `-wal` and
`-shm` files that Tolino 5.18.279364 leaves on the USB partition are folded
into the database (the backup then holds their content too). Eject the device before you open
the book. On Kobo 6.0 and Tolino 5.18.279364 Nickel opens a book from its
cache of rendered pages and, when no cached page holds the written place,
at the first cached page of that chapter. So the write also drops the
cached pages of that chapter (only of that chapter, and only when none
holds the place); it then says so, and the device has to be restarted, not
only ejected, because Nickel keeps those pages in memory. To undo, copy the backup back to `.kobo/KoboReader.sqlite` while
the device is mounted.

### Book mapping (in progress)

The sync server knows a book by a checksum of its file. A kepub on the
Kobo, a differently converted EPUB on the Tolino or a KFX on a Kindle has
another checksum than the original EPUB that KOReader opens. A mapping
links such a book to its original EPUB:

```sh
nickelinksync map suggest --device /Volumes/KOBOeReader --library ~/Calibre\ Library
nickelinksync map add --device /Volumes/KOBOeReader --book "Heat 2" --epub ~/Books/heat-2.epub
nickelinksync map add --device ~/Kindle-copy --book heat-2
nickelinksync map list
nickelinksync map remove 1a2b3c4d
```

| Command | What it does |
|---|---|
| `map suggest` | For each book on the device (or those `--book`/`--content-id` select): already mapped, the same file as one in the library (no mapping needed), or up to three candidates from the library with their score, and the `map add` command for the best one. Stores nothing. |
| `map add` | Maps one book on the device (`--book <text>` or `--content-id`) to the original EPUB given with `--epub` and says how many chapters of the two files match. Without `--epub` it shows the suggestions from the library and asks which one to take (only in a terminal). A file identical to the EPUB needs no mapping and is not stored. Mapping the same device book again replaces the old mapping. |
| `map list` | Shows every mapping and checks its EPUB: a mapping whose EPUB is missing or has changed since is reported as broken. |
| `map remove` | Removes the mapping with this checksum or Kindle `content_id` (or a unique start of it). |

A Kobo or Tolino book is recognized by the checksum of its file, so moving
or renaming it on the device keeps the mapping; a Kindle book by its
`content_id`. Neither the device nor the EPUB is changed.

**Syncing through the mapping.** `status`, `push` and `pull` decide per
book under which checksum it syncs (shown as `sync as`):

| Book | Synced |
|---|---|
| mapped, EPUB unchanged | under the original EPUB's checksum, the position translated (below) |
| mapped, EPUB missing or changed | not at all, until it is mapped again |
| the same file as in the library, or the original EPUB of a mapping | under its own checksum |
| none of these, with a library configured | not at all ("not mapped"; `map suggest` helps) |
| no library configured | under its own checksum, as before |

The position is translated between the device file and the original by
chapter (the file's path; calibre's split chapters `ch1_split_000.xhtml`,
`_split_001` … count as `ch1.xhtml`) and by the chapter's visible text
(whitespace and soft hyphens left out). Where something does not match,
the output says so: *found by its text* (the text differs, the position was
found by the 32 characters after it), *chapter start only* (the place in
the chapter was not found), *from the percentage only* (the chapter is not
in the other file, for example calibre's `titlepage.xhtml`). `pull
--write` moves the server position back into a mapped EPUB on the Tolino
the same way; into a kepub it writes nothing yet.

**Kindle.** `status` and `push` also read a Kindle's books, for every KFX
that is mapped to its EPUB (`map add`): the last page read (`lpr` in the
`.sdr` folder) is translated into the EPUB and sent under its checksum.
Nothing is written to the Kindle, and `pull` refuses. A recent Kindle
offers only MTP, which a Mac does not mount: copy the `documents` folder,
or only a book's `.kfx` file with its `.sdr` folder, with
[OpenMTP](https://openmtp.ganeshrvel.com/) to a folder and give that as
`--device`. Only books sent with **Send-to-Kindle** can be read; books
bought in the Kindle store are encrypted (DRM) and are skipped. A
jailbreak is not needed.

```sh
nickelinksync map add --device ~/Kindle-copy --book "Good Boy" --library ~/Calibre\ Library
nickelinksync push --device ~/Kindle-copy
```

The mappings are only read on the computer: the Tolino plugin syncs every
book under its own checksum, as before.

**Library.** The original EPUBs come from a folder given with `--library`
or as `library` in the configuration file. If it holds a Calibre library
(`metadata.db`), titles, authors and EPUB paths come from Calibre's
database, opened read-only as an immutable file; otherwise every `*.epub`
below the folder (not `*.kepub.epub`) is read, with title and authors from
its OPF. NickelInkSync never writes into the library.

**Score.** Suggestions compare the title (word overlap with the EPUB's
title or file name; a Kindle book's title is the name of the file it was
sent as), the authors, and for the best candidates the chapters (spine
items found by path, or KFX sections against spine items): title × 0.7 +
author × 0.3, then 0.8 × that + 0.2 × chapters. Accents and umlauts are
folded, numbers count, so "Volume 1" and "Volume 2" differ. Candidates
below 50 % are not shown; below 80 % the best one is marked "uncertain".

The mappings are kept in `mappings.json` in the data directory
`~/.local/share/nickelinksync/` (or `$XDG_DATA_HOME/nickelinksync`, or
`$NICKELINKSYNC_DATA_DIR`), shared with the desktop client.

### Desktop client (in progress)

A window for the same work, built on the same library (`gui/`, egui):
`just gui` or `cargo run -p nickelinksync-gui`. It finds a Kobo or
Tolino connected over USB (macOS: `/Volumes`), takes a copy of a
Kindle's books as a folder, and lists the books by how they sync (not
mapped, no reference EPUB, mapped, own checksum) with their position on
the device; without a library it warns that it cannot check which books
KOReader knows. A book is
mapped from there: the library's suggestions with their scores, or an
EPUB chosen by hand; a mapping can be changed or removed. With a server
in `config.ini` it shows the server's position next to the device's and
sends positions per book or all at once; a `confirm` question comes as a
dialog (keep the server's, or send the device's). On a Tolino or Kobo
(not into kepubs yet, and not with Kobalt installed) "Get" and "Get all"
write the server's position into the device database, after a
backup of it in the data directory, as `pull --write` does; a `confirm`
question comes as the same dialog (keep the device's, or take the
server's), and the device is read again afterwards. Eject the device
before you open the book, and restart it when the notice says so. "History" lists what the CLI and the window
sent and wrote (`history.jsonl` in the data directory). "Add from
library" copies books of the library to `Books/` on a Kobo or Tolino,
never over a file that is there or an EPUB already on the device; on a
Kobo optionally as kepubs made by
[kepubify](https://github.com/pgaskin/kepubify) (looked up in the `PATH`,
Homebrew's folders, `~/go/bin` or `$NICKELINKSYNC_KEPUBIFY`), mapped to
their EPUB at once. "Settings" edits
`config.ini` (server, user, device name, `confirm`, `position`, library)
line by line, keeping comments, after a backup (`config.ini.bak`); a
password typed there is kept only as its key in the keychain, and a
password or key found in the file can be moved there. "Overview"
shows all connected devices side by side: one line per book, one column
per device with its position, and the server's. To a Kobo or a
Kindle it writes nothing, only `mappings.json` and the history.
Nothing is synced when a device is connected. German or English after the
system language, switchable. It shares `config.ini` and `mappings.json`
with the CLI; its own settings (language, Kindle folder) are in
`gui.json` in the data directory.

## Configuration

`push` and `pull` need a configuration file, `status` uses it if it exists.
Default location: `~/.config/nickelinksync/config.ini` (or
`$XDG_CONFIG_HOME/nickelinksync/config.ini`, or the path in
`$NICKELINKSYNC_CONFIG`).

```ini
# KOSync server and account
server = https://kosync.eu
user = alice
password = your password
# or, instead of the password, its MD5 hash:
# key = 5ebe2294ecd0e0f08eab7690d2a6ee69

# How the device appears on the server (optional)
# device = Tolino Shine Color
# device_id = 0123456789ABCDEF0123456789ABCDEF

# When to ask before overwriting a position: never, backwards (default), always
# confirm = backwards

# Messages of the Tolino plugin: all (default), errors, none
# notify = all

# Folder with the original EPUBs, a Calibre library or any folder (book mapping)
# library = ~/Calibre Library

# Which place push sends: paragraph (default), sentence, exact
# position = paragraph
```

- The password is only read from this file, never from the command line.
  Keep the file private (`chmod 600`); NickelInkSync warns if others can
  read it. Only the MD5 of the password is sent, as the KOSync protocol
  requires.
- On macOS and Windows the key can live in the system keychain
  (Credential Manager on Windows) instead: without `password` and `key` in
  the file, the CLI looks there (service "NickelInkSync", account
  `<user>@<server>`). The desktop client's settings put it there; the
  device and Linux keep it in the file.
- Without `device`, each e-reader appears under its model name (for example
  "Tolino Shine Color"), with a `device_id` derived from that name. Set both
  only if one configuration should always report the same device.
- `confirm` decides when `push` and `pull` ask "device or server?" instead
  of letting the newer position win:

  | Value | Asks |
  |---|---|
  | `never` | never: the newer position wins |
  | `backwards` (default) | when the position that would be overwritten lies further on in the book than the new one |
  | `always` | whenever the two positions differ |

  "Further on" is decided by the place in the book (chapter, then the
  place in the chapter's text, the percentage only if that does not
  decide), not by time: Nickel marks a book as read the moment you open
  it, so a book you only opened on the e-reader looks newer than the page
  you reached in KOReader. In a terminal you answer `d` (device) or `s`
  (server), Enter skips the book. Without a terminal nothing is overwritten
  and the book is reported. `--force` overwrites without asking.
- `notify` is only read by the Tolino plugin (see below): which messages
  it shows on the screen.
- `library` is used by the `map` commands and decides which books
  `status`, `push` and `pull` sync (see [Book
  mapping](#book-mapping-in-progress)). A leading `~/` is your home
  folder. The `map` commands need no server settings: a file with only
  `library` (and `position`) is enough; `status` then shows no server.
- `position` decides which place `push` sends. The device stores where
  its page starts, often mid-sentence, and KOReader opens the page that
  holds that place by its own page layout, so it can be anywhere on the
  page; the start of a paragraph or sentence is easier to find there.

  | Value | Sends |
  |---|---|
  | `paragraph` (default) | the start of the paragraph; if that is more than about half a page (600 characters without spaces) back, the start of the sentence; if that is too, the place itself |
  | `sentence` | the start of the sentence, at most half a page back |
  | `exact` | the place itself, to the character |

  The output says how far it moved ("moved back 103 characters to the
  paragraph start"), and `status` and `confirm` compare with the same
  place, so a book is not reported as newer on the device just because
  of it. `pull` takes the server position as it is.
- Lines starting with `#` or `;` are comments; unknown keys are an error.

## Plugin for the Tolino (experimental)

On a Tolino with firmware 5.x, a NickelHook plugin syncs without a
computer, over Wi-Fi: when you open a sideloaded book it fetches a newer
position from the server and the book opens there, when you leave the
book it sends yours. It
calls the same `nickelinksync` program on the device and follows the same
`confirm` rule; it cannot ask yet, so a position that needs an answer is
left alone and noted in the log. On a Kobo the plugin stays inactive for
now.

1. Get `NickelInkSync-tolino` from the latest CI run (or `just package`,
   see [Build](#build)). It holds `Kobo.tgz` and a `.adds/NickelInkSync/`
   folder with the program and `config.ini.example`.
2. Connect the Tolino over USB. Copy the `NickelInkSync` folder into the
   hidden `.adds/` folder and save `config.ini.example` there as
   `config.ini` with your account (same format as
   [above](#configuration)).
3. Copy `Kobo.tgz` into the hidden `.kobo/` folder and eject. The Tolino
   installs the plugin and restarts.

The log is `.adds/NickelInkSync/NickelInkSync.log`. The plugin switches
Wi-Fi on when it needs it; there is no need for the Tolino's own
"Sync now" (that is the Tolino cloud). Opening a book waits at most about
10 seconds for Wi-Fi (no dialog) and 15 seconds for the server. To open
at the server's position, the plugin writes it into the book's row in the
device database just before Nickel opens the book, while Nickel runs; it
keeps one backup of the database a day in `.adds/NickelInkSync/backups/`
(the two newest). Without Wi-Fi the book opens unchanged, and leaving it
then sends nothing, so an older Tolino position never overwrites a newer
one on the server. Leaving a book never waits: the push follows when
Wi-Fi is up, after 15 seconds with the Tolino's Wi-Fi dialog.

If Nickel does not open a book at the position the plugin wrote (seen on
the first opening after the Tolino starts), the plugin notes it in
`.adds/NickelInkSync/not-in-line.tsv`. Such a book is never sent, not by
the plugin and not by `push` on the computer (unless `--force`), because
the place Nickel saved is not yours; the next `pull`, on opening or over
USB, takes the server's position again, even though the Tolino's
"last read" now looks newer.

Each outcome shows as a short message at the bottom of the screen (in
German for now), for example "Fortschritt übernommen" with the percentage
and the device that sent it, or "Laden fehlgeschlagen" with the reason (no
connection, timeout, login refused). The texts are short because the
Tolino cuts longer ones off. `notify` in `config.ini` picks which:

| Value | Shows |
|---|---|
| `all` (default) | every outcome, also "Fortschritt ist aktuell" when there was nothing to take or send (for example when you leave a book on the page the server's position opened it on) |
| `errors` | only what went wrong or was left undone: server not reached, position not taken, position not sent |
| `none` | nothing; the log still has everything |

**With [Kobalt](https://github.com/nicoverbruggen/kobalt):** Kobalt, a
reader that runs inside Nickel, writes the reading positions itself and
is getting a KOSync client of its own. While Kobalt is installed, the
plugin switches itself off: it writes nothing into the device database and
sends nothing, says so once in a message ("NickelInkSync ist aus", about
20 seconds after the Tolino starts) and notes it in the log at every
start. The plugin stays installed and works
again once Kobalt is removed (after the next start). On the computer,
`pull --write` and "Get" in the desktop client refuse a device with Kobalt
on it; `push` and "Send" only warn. A leftover `.adds/kobalt` folder
without its file `uninstall` (Kobalt's own way to remove it) does not
count.

**Uninstall:** create an empty file `.adds/NickelInkSync/uninstall`, eject
and power the Tolino off and on again (the file is only checked when the
Tolino starts).

## Devices and books

- Kobo on firmware 4.x and Tolino Shine 5, Shine Color and Vision Color on
  firmware 5.x
- Sideloaded kepub and EPUB books without DRM
- For now, put the **same file** on every device. The sync server tells
  books apart by a hash of the file, so a kepub on one device and the original
  EPUB on another count as two different books. The manual mapping between
  them is being built (see [Book mapping](#book-mapping-in-progress)).

## History

This repository used to hold `inksync`, a Kobo-to-Obsidian export of
highlights and reading time. It was removed when the project changed scope.
The last version is in the Git history before the scope change.

## Build

Needs a stable [Rust](https://rustup.rs) toolchain.

```sh
cargo build --release
```

The binary is `target/release/nickelinksync`. HTTPS uses `rustls` with
built-in root certificates, so no OpenSSL is needed, also not on the device.

For the device (Kobo and Tolino, 32-bit ARM), build a statically linked binary
with [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild):

```sh
rustup target add armv7-unknown-linux-musleabihf
pip install cargo-zigbuild   # also installs Zig
just build-device
```

The binary is `target/armv7-unknown-linux-musleabihf/release/nickelinksync`.
`just check-device` also runs `--version` in an ARM container (needs Docker).

The plugin (`plugin/`, NickelHook as a submodule) is built for Qt 6.5.2 in
the toolchain image `ghcr.io/dkuester/nickelstorygraph-toolchain` (Docker,
amd64): `just plugin` gives `plugin/Kobo.tgz`, `just package` puts it
together with the device binary into `dist/NickelInkSync/`. Clone with
`--recurse-submodules`, or run `git submodule update --init`.
Each CI run uploads the same file as the artifact `nickelinksync-armv7-musleabihf`.

## Support

| | |
|---|---|
| Bugs and ideas | [Issues](https://github.com/dkuester/NickelInkSync/issues). Please include your device model and firmware version. |
| Contributing | Pull requests against `main`. |

## Credits

NickelInkSync builds on the work of these projects and their maintainers.

**Code it uses**

| Project | By | License | What for |
|---|---|---|---|
| [NickelHook](https://github.com/pgaskin/NickelHook) | Patrick Gaskin | MIT | Hooking Nickel; the plugin is built on it |
| [NickelHook, branch `qt6-support`](https://github.com/nicoverbruggen/NickelHook/tree/qt6-support) | Nico Verbruggen | MIT | Qt6 port of NickelHook for firmware 5.x, the plugin's submodule |
| Rust crates: [clap](https://github.com/clap-rs/clap), [md-5](https://github.com/RustCrypto/hashes), [quick-xml](https://github.com/tafia/quick-xml), [rusqlite](https://github.com/rusqlite/rusqlite) (with [SQLite](https://sqlite.org)), [serde](https://github.com/serde-rs/serde), [ureq](https://github.com/algesten/ureq) (with [rustls](https://github.com/rustls/rustls)), [zip](https://github.com/zip-rs/zip2) | their authors | MIT / Apache-2.0, SQLite public domain | The CLI (see `Cargo.toml`) |

**Ideas and documentation, no code** (re-implemented from the described
behavior; the AGPL-3.0 projects are references only, see
[SCOPE.md](SCOPE.md#license))

| Project | By | License | What for |
|---|---|---|---|
| [KOReader](https://github.com/koreader/koreader) | the KOReader developers | AGPL-3.0 | The partial MD5 book hash, XPointers, the reading-progress sync |
| [koreader-sync-server](https://github.com/koreader/koreader-sync-server) | the KOReader developers | AGPL-3.0 | The KOSync protocol |
| [BookOrbit](https://github.com/bookorbit/bookorbit) | the BookOrbit developers | AGPL-3.0 | Matching books by the KOReader hash; the planned manual book mapping |
| [Kobalt](https://github.com/nicoverbruggen/kobalt) | Nico Verbruggen | MIT | How Nickel caches books (`Cache<Volume>`) and how to write progress through Nickel itself |
| [Amazon Ion 1.0](https://amazon-ion.github.io/ion-docs/) | Amazon | specification | Reading KFX books (Kindle feasibility check) |

## License

[MIT](LICENSE)

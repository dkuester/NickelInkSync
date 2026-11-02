# libnickel symbols for firmware 6.0 (Qt6)

What the Kobo client (stage C0 in [TODO.md](../TODO.md)) needs from Nickel
on the Kobo 6.0.279225: the symbols the plugin uses today, the way Kobalt
writes a reading position through Nickel itself, and the symbols for a
dialog and a book menu entry of our own. Checked on 2026-11-01 against the
library and in the Kobo [Nickel VM](nickel-vm.md). The 5.18 analysis, with
the history of the pull path, is in
[nickel-5.18-symbols.md](nickel-5.18-symbols.md).

## Sources

| | |
|---|---|
| Library | `libnickel.so.1.0.0` from the Kobo firmware 6.0.279225 (`qt6-update-6.0.279225.zip`, the `kobo12` package the VM uses; the reference Clara BW runs the same firmware version): 41,977,320 bytes, 98,256 defined dynamic symbols. Copied out of the VM image with `docker create --platform linux/arm/v7 localhost/nickel-vm-rootfs:6.0.279225` and `docker cp`. Not in this repository. |
| Disassembly | As for 5.18: Thumb-2, `llvm-objdump -d --triple=thumbv7a-linux-gnueabihf` from the even address for the size `llvm-nm -S` gives. |
| Kobalt | [Kobalt](https://github.com/nicoverbruggen/kobalt) `main` @ `3d8514c`, its own code under MIT: `platforms/kobo/qt6/native_progress.cpp` (write path), `platforms/kobo/qt6/hooks.cpp` and `platforms/kobo/common/book_menu.cpp` (book menu). Read for the concept only; no code taken over. |

Calling convention as in the 5.18 document: `this` in `r0`; a class
returned by value goes through a result pointer in `r0`, and the arguments
move up by one; a `QString` or `QVariantMap` passed by value goes by
invisible reference.

## The plugin's symbols

All 23 symbols of `plugin/src/nickelinksync.cc` (two hooks, 21 `dlsym`)
are exported by 6.0.279225 under the same names as on the Tolino 5.18.
Disassembled again on 6.0, unchanged:

| Symbol | Signature (checked on 6.0) |
|---|---|
| `_ZNK7Content5getIdERK7QString` | `QString Content::getId(QString const&) const`, by value |
| `_ZN13VolumeManager7getByIdERK7QStringS2_` | `static Volume VolumeManager::getById(QString const& id, QString const& db)`, by value |
| `_ZNK8Bookmark16getStartLocationEv` | `ContentLocation const& Bookmark::getStartLocation() const`, still a reference (`d + 0x104`); 6.0 first fills it in lazily when a flag at `d + 0x114` is set |
| `_ZN20MainWindowController5toastERK7QStringS2_i` | `void`, still `adds r0, #0x14` and a tail call to the toaster |
| `_ZN11ReadingView26bookmarkForCurrentLocationEv` | `Bookmark ReadingView::bookmarkForCurrentLocation()`, by value |

`Repository::purgeCache<Volume>(QString const&)`
(`_ZN10Repository10purgeCacheI6VolumeEEvRK7QString`), missing in the Tolino
build 5.18.264769, is exported by 6.0; the write path below does not need
it.

## Kobalt's write path

Kobalt never writes Nickel's database with a connection of its own. It
changes a `Volume` with Nickel's setters and lets Nickel's
`VolumeManager` store the changed columns inside a transaction on Nickel's
own connection. Every symbol is exported by 6.0.279225:

| Purpose | Symbol | Signature (checked) |
|---|---|---|
| Database name | `_ZN6Device16getCurrentDeviceEv`, `_ZNK6Device9getDbNameEv` | `QString const& Device::getDbName() const`, a reference (`d + 0x70`); `/mnt/onboard/.kobo/KoboReader.sqlite` |
| Begin | `_ZN15DatabaseAdapter16beginTransactionE7QString` | `static bool DatabaseAdapter::beginTransaction(QString db)`: an empty name means `getDbName()`; **false** if Nickel's connection already has a transaction (`hasTransaction`), so no nesting; otherwise runs the `BEGIN` statement and starts Nickel's transaction timer |
| End | `_ZN15DatabaseAdapter14endTransactionE7QStringb` | `static bool DatabaseAdapter::endTransaction(QString db, bool commit)`: commit or roll back, stops the timer |
| Nickel's connection | `_ZN15DatabaseAdapter12dbConnectionERK7QStringb` | `static QSqlDatabase DatabaseAdapter::dbConnection(QString const&, bool)`, by value |
| The book | `_ZN13VolumeManager7getByIdERK7QStringS2_` | as above |
| Its columns | `_ZNK6Volume11getDbValuesEv` | `QVariantMap Volume::getDbValues() const`, by value (keys are column names) |
| Build the place | `_ZN15ContentLocation6createE7QStringS0_`, `_ZNK15ContentLocation7isValidEv`, `_ZNK15ContentLocation8toStringEv`, `_ZN15ContentLocationD1Ev` | `static ContentLocation create(QString position, QString spinePath)` by value (8 bytes), `bool isValid() const`, `QString toString() const` by value |
| Set the place | `_ZN13VolumeManager14updateBookmarkER6VolumeRK7QStringiRK9QDateTimei` | `static void VolumeManager::updateBookmark(Volume&, QString const& position, int, QDateTime const& time, int)`. **Only in memory:** sets `ChapterIDBookmarked` and the `int` (Kobalt passes the row's `adobe_location`) through virtual setters, `DateLastRead`, `ReadStateSynced = false`, and `HideFromHomePage = false` for an unread book; on a Tolino it then tail-calls into the `tolino_bookmark` code with the `Volume`, the position and the last `int` (a page number). It writes nothing to the database |
| Set the percentage | `_ZN7Content14setPercentReadERK8QVariant` | `void`, stored through `QVariant::toInt` |
| Store | `_ZN13VolumeManager12updateFieldsERK7QStringRK4QMapIS0_8QVariantES7_S2_` | `static bool VolumeManager::updateFields(QString const& id, QVariantMap const& changed, QVariantMap const& where, QString const& db)`: `getById(id, db)`, `DatabaseAdapter::updateFields(Volume::getTableName(), changed, where, db)`; on success it copies the `Volume`, applies `changed` (`ScObject::setAttributes`) and puts it into Nickel's `Cache<Volume>` (`Cache<Volume>::update`, under the repository mutex), then emits `VolumeManager::onVolumeChanged`. **The cache follows the write**, so no purge is needed |
| After a failure | `_ZN10Repository12refreshCacheI6VolumeEEP5CacheIT_ERK7QString` | `static Cache<Volume>* Repository::refreshCache<Volume>(QString const& db)`; Kobalt calls it only when the commit failed or the stored row does not match |
| Layout check | `_ZN9QtPrivate25QMetaTypeInterfaceWrapperI6VolumeE8metaTypeE` | Kobalt refuses to run unless `QMetaType(Volume).sizeOf() == 12` |

The order, as the throwaway plugin ran it in the VM: `beginTransaction(db)`;
`getById(id, db)`; `getDbValues()` before; `updateBookmark(volume,
create(position, "").toString(), before["adobe_location"], now (UTC), 0)`;
`setPercentRead(volume, percent)`; `getDbValues()` after; the columns that
differ (here `ChapterIDBookmarked`, `DateLastRead`, `___PercentRead`) go
into `updateFields(id, changed, {ContentID: id}, db)`; `endTransaction(db,
ok)`; on failure `refreshCache<Volume>(db)`. Kobalt also compares the
committed row through a read-only connection of its own afterwards, and on
the Kobo hands the book to `BookmarkSyncManager::sendBookmark` (Kobo's
cloud, not relevant for sideloaded books).

## Dialog and book menu

| Purpose | Symbol | Signature (checked) |
|---|---|---|
| Book menu (hook) | `_ZN16ActionProxyMixin17setupBookListMenuEP28AbstractNickelMenuControllerRK6VolumeP15NickelTouchMenuRK7QStringS9_bRK3Tab` | `void ActionProxyMixin::setupBookListMenu(AbstractNickelMenuController*, Volume const&, NickelTouchMenu*, QString const&, QString const&, bool, Tab const&)`: `this`, controller, `Volume`, menu in `r0`–`r3`, then the two strings, the `bool` and the `Tab` on the stack. `NickelTouchMenu` is a `QMenu` |
| Menu item | `_ZN28AbstractNickelMenuController18createMenuTextItemEP5QMenuRK7QStringbbS4_` | `QWidget* createMenuTextItem(QMenu*, QString const& text, bool, bool, QString const& objectName)` |
| Its action | `_ZN22AbstractMenuController12createActionEP5QMenuP7QWidgetbbb` | `QAction* createAction(QMenu*, QWidget* item, bool, bool, bool)`, adds the item to the menu |
| Dialog | `_ZN25ConfirmationDialogFactory21getConfirmationDialogEP7QWidget` | `static ConfirmationDialog* getConfirmationDialog(QWidget* parent)`: `new ConfirmationDialog(parent)`, a `QDialog` (`qobject_cast<QDialog*>` works) |
| Its texts | `_ZN18ConfirmationDialog8setTitleERK7QString`, `…7setTextERK7QString`, `…19setAcceptButtonTextERK7QString`, `…19setRejectButtonTextERK7QString` | `void` setters |
| More content | `_ZN18ConfirmationDialog9addWidgetEP7QWidget` | `void ConfirmationDialog::addWidget(QWidget*)` |

Others for C5/C6, exported, not tried: `ConfirmationDialogFactory::
showOKDialog`/`showErrorDialog(QString const&, QString const&)`, `N3Dialog`
(`N3Dialog(QWidget*, bool)`, `setTitle`, `setContent`, `showLoading`) and
`N3DialogFactory::getDialog(QWidget*, bool)` for a full-screen view of our
own.

In the VM (hooked after the original, as Kobalt does): the item "C0 probe"
appears as the last native entry of a book's menu (after "Remove"), and
tapping it fires the action. The dialog shows Nickel's frame, title,
close button and two native buttons; "Take server" finished it with 1
(accept). Two things for C5: a plain `QPushButton` added with `addWidget`
is drawn, unstyled, but never gets a tap (Nickel's own touch handling), so
a third button has to be a Nickel widget; and `setText` showed `\n` as a
space.

### The conflict dialog (C5)

| Purpose | Symbol | Signature (checked) |
|---|---|---|
| Its buttons | `_ZNK18ConfirmationDialog12acceptButtonEv`, `…12rejectButtonEv` | `QWidget* acceptButton() const`, `rejectButton() const`: the two `N3ButtonLabel`s (object names `acceptButton`, `rejectButton`), in a `QHBoxLayout` of their own row widget |
| Third button | `_ZN13N3ButtonLabelC1EP7QWidget` | `N3ButtonLabel(QWidget* parent)`; `Ui_ConfirmationDialog::setupUi` allocates 0x88 bytes for one (5.18 and 6.0; 0x98 on 4.38). A `TouchLabel` and so a `QLabel`; its signal `tapped(bool)` connects by name |

The plugin builds the third button itself (`operator new` with room to
spare, then the constructor, parented to the buttons' row) and inserts it
in front of the reject button. Checked in the Kobo VM on 2026-11-02:

- The `N3ButtonLabel` gets taps: `tapped(bool)`, connected by name to a
  `QAction`'s `trigger()`, ends the dialog with our own result (2).
  Accept gives 1, reject and the close button 0.
- The dialog's own style sheet makes its two buttons primary buttons
  (`#acceptButton,#rejectButton{qproperty-primaryButton:true}`), which
  Nickel's application style sheet draws grey and centred; the property
  `primaryButton` set on ours does the same. Copying the reject button's
  dynamic properties instead is wrong: they are Qt's style sheet caches
  (`_q_stylesheet_minh` and so on).
- `setText` takes HTML: `<br>` breaks the line and `<b>` is bold.
- Nickel shrinks a button's text to fit; with three buttons "Server
  übernehmen" is drawn smaller than "Später".
- `getConfirmationDialog(nullptr)` parents the dialog to the main window;
  `QDialog::exec()` inside the `setVolume` hook shows it over the library
  and blocks the opening until it is answered.

## The write path in the VM

A throwaway plugin (not in this repository) did Kobalt's sequence above in
the Kobo VM with Book R (`book-r.kepub.epub` and `book-r.epub`), either on
a command while the book was closed or inside the hooked
`ReadingController::setVolume`, before the original, handing Nickel the
`Volume` from a fresh `getById`. It logged the row and Nickel's cache
before and after, the book's cached pages (`CachePersistenceRenderedPageData`)
and, after opening, where Nickel stood (`bookmarkForCurrentLocation`), and
on request dropped the target chapter's cached pages the way
`drop_stale_pages` in `src/db.rs` does, on Nickel's connection inside
Nickel's transaction.

On 6.0 both books are drawn by the TENG renderer ("TolinoEngineRenderer";
position format "KoboSpan" for the kepub). Nickel stores the kepub as
`OEBPS/text/chapter1.xhtml#kobo.17.1:212` and the EPUB as
`OEBPS/text/chapter1.xhtml#point(/1/4/40/1:0)`; `ContentLocation::create`
accepts both and `toString` gives them back unchanged.

Every write committed, and the row, Nickel's `Volume` cache and the
`Volume` passed to `setVolume` held the written place. Where Nickel then
opened (each "after a start" is the first opening of that book after
Nickel started):

| | Book | Written | Opening | Target chapter in the page cache | Pages dropped | Nickel opened at |
|---|---|---|---|---|---|---|
| A | kepub | outside Nickel, Nickel stopped (as over USB) | after a start | `kobo.17`–`76`, gap at the place, chapter start not cached | no | `kobo.36.1:104` for `kobo.38.1:0`, right |
| B | kepub | through Nickel, in the hook | same session | from `kobo.17`, chapter start not cached | no | `kobo.9.1:249` for `kobo.10.1:0`, right |
| C | EPUB | through Nickel, before opening | after a start | `/1/4/2`–`/1/4/76`, chapter start cached | no | **`/1/4/2/1:0`, chapter start** for `/1/4/120/1:0` |
| D | EPUB | as C | after a start | as C | 14 | `/1/4/118/1:168` for `/1/4/120/1:0`, right |
| E | EPUB | through Nickel, in the hook | same session | from `/1/4/118` | no | `/1/4/40/1:0` for `/1/4/40/1:0`, right |
| F | EPUB | through Nickel, in the hook | after a start | `/1/4/40`–`/1/4/60` and from `/1/4/118`, chapter start not cached | no | `/1/4/86/1:208` for `/1/4/90/1:0`, right |
| G | kepub | through Nickel, before opening | after a start | `kobo.1`–`28`, chapter start cached | no | **`kobo.1.1:0`, chapter start** for `kobo.40.1:0` |
| H | kepub | through Nickel, in the hook | after a start | as G | 10 | `kobo.47.1:202` for `kobo.50.1:0`, right |

"Right" means that `bookmarkForCurrentLocation` gave the start of the page
that holds the written place, after opening and again 30 s later. The
screen showed the chapter heading in C, and the right markers in two
earlier runs of the same kind (R3-67, R1-39/R1-40).

What Nickel does (syslog, and
`SinglePageCacheEngine::initializeBookFromPersistentCache` disassembled):
on the first opening after a start it asks its page cache for a page that
starts at the place (`PersistedCacheManager::getEntryByStartLocation`),
then for one that holds it (`getEntryWrappingLocation`), and then for the
first page of the chapter (`getByChapterInPersistence` on
`ContentLocation::toSpine()`), logging "inferred position with
getByChapterInPersistence". Only if that page is cached does Nickel open
there (C, G). Without it, it renders the place live and opens right (A, F);
in the same session it went through `openBookFromPersistentCache` and
found the place (B, E). This is the N5 failure, and it explains the old
device pattern "the first opening after a start fails, the second works".

**Result:** Kobalt's write path works on 6.0 for kepub and EPUB, also on
the first opening after a start, but it does **not** get around the page
cache: Nickel opens from its cache whatever wrote the row. Dropping the
target chapter's cached pages when none holds the place, as the CLI has
done since N5, fixes it (D, H), here on Nickel's own connection inside the
same transaction and without a restart.

Limits of the VM: everything is emulated and slow; before this test the
VM lost the rendered page images at every restart, so after a restart
Nickel always rendered live and could not show N5 (now
`/mnt/aux` is kept, see [nickel-vm.md](nickel-vm.md)); and the device test
of 31.10. on the Kobo, which dropped the rows of all three cache tables
without a restart, still opened at the chapter start. That was not Nickel
writing pages back from memory; see
[Restart after a USB write](#restart-after-a-usb-write).

## Restart after a USB write

Checked on 1 and 2 November 2026 in the `kobo` VM (6.0.279225), Book R as
EPUB. Until then the CLI and the desktop client asked for a restart after
dropping pages over USB, "because Nickel keeps those pages in memory".
That does not hold.

**Where Nickel looks.** `openBookFromPersistentCache` always calls
`initializeBookFromPersistentCache`, which asks
`PersistedCacheManager::getEntryByStartLocation`, then
`getEntryWrappingLocation`, then `getByChapterInPersistence` on
`ContentLocation::toSpine()` (through `getFirstEntryFound`), and else logs
"openBookFromPersistentCache could not find page with location at …" and
renders the place live. Each of these is a query on
`CachePersistenceRenderedPageData` at the moment of opening; the
in-memory cache (`populateMemoryCache`) is filled only after a hit. The
same call chain is in 5.18.279364. The chapter fallback hits only when the
chapter's **first** page is cached: with chapter 2 cached only from
`/1/4/114`, a place at `/1/4/60` opened right (E3 below, and F above).

**USB in the VM.** `usb plug add` on `/run/kobo/udev/fifo` and "Connect":
Nickel closes `KoboReader.sqlite` (the WAL is checkpointed, `-wal` and
`-shm` are gone, no file of it stays open); a book that is open is closed
first and its bookmark saved. Then the position was written from the host
with the same SQL as `write_positions` and `drop_stale_pages`, and
`usb plug remove` ejected.

| | Before | Written | Pages dropped | Restart | Nickel opened at |
|---|---|---|---|---|---|
| E5 | book opened earlier in this session, chapter 2 cached from its start | `/1/4/100` | no | no | **chapter start**, "inferred position with getByChapterInPersistence" |
| E1 | as E5 | `/1/4/120` | yes | no | R2-59, right; "could not find page", live |
| E1b | as E5 | `/1/4/100` | yes | no | R2-49, right |
| E3 | after a start, chapter 2 cached only from `/1/4/114` | `/1/4/60` | no | no | R2-29, right |
| E4 | book open while plugging in, chapter 2 cached from its start | `/1/4/140` | yes | no | Nickel went to the home screen, the dropped rows stayed gone; opening: R2-69, right |

So the failure also comes on a later opening in the same session (E5), not
only on the first after a start, and dropping the target chapter's pages
fixes it without a restart (E1, E1b, E4).

**The Kobo test of 31.10.** The backups of that test
(`kobo-cachetest`, 12:06, and `kobo-progresstest`, 12:13) show Book R's
chapter 2 pages afterwards with new ids and `lastAccess` 12:11:44–52
(before: 12:00:21–30): rendered fresh from the chapter start, not written
back. `ChapterIDBookmarked` named the chapter start before and after, so
Nickel opened where the row said; why the written place was not in the row
then is open.

**Consequence.** The CLI and the desktop client say "eject" after dropping
pages, with a restart only as the fallback if the book still opens at the
chapter start. Restoring a backup still asks for a restart (the whole
database is replaced; not checked).

**Device test, Tolino 5.18.279364 (2 November).** Book R EPUB, plugin off,
Kobalt removed. Chapter 2 read from its start, KOReader at R2-178;
`pull --write` dropped 27 cached pages of chapter 2 (its first page among
them), the device was only ejected, and Book R opened at R2-178. An
earlier run with R2-121, which lay in a cached page, dropped nothing and
opened right too.

**Device test, Clara BW 6.0.279225 (2 November).** Same steps, plugin
off: chapter 2 read from its start (its first page cached), KOReader at
the same place. `pull --write` wrote the EPUB and the mapped kepub and
dropped 41 cached pages of their chapter 2; the device was only ejected.
The EPUB opened at the written place (`/1/4/358/1:158`, the page with
R2-179 at the top, as in KOReader); Nickel kept that bookmark and cached
fresh pages from `/1/4/356` on.
Kobo 4.x has no page cache: the Libra 2 on 4.38.23852 (database version
174) has no `CachePersistence*` tables, so the CLI drops nothing there
([nickel-4.38-symbols.md](nickel-4.38-symbols.md)).

## The Kobalt guard on the Kobo

Kobalt installs the same way on Kobo 6.0 as on the Tolino (`Kobo.tgz` with
`imageformats/libkobalt.so`; NickelHook parks it one folder up for the
first seconds after a start), so the paths in `src/kobalt.rs` and the
plugin hold. In the Kobo VM the device CLI (`pull --live --emit`) refused
with `NICKELINKSYNC_ERROR kobalt` with the library in `imageformats/` or
parked as `libkobalt.so.failsafe` plus `.adds/kobalt/uninstall`, and ran
normally without Kobalt, with `ignore-kobalt` and with `uninstall-now`.
The plugin's own check is not reached on a Kobo today: `init()` returns at
the Tolino check before it.

## Recommendation for C4

1. **Opening:** in the hooked `setVolume`, before the original, write the
   pulled place through Nickel as above: `beginTransaction` (false: write
   nothing, open as it is, mark the book pending), `getById`,
   `updateBookmark` with `ContentLocation::create(…).toString()` (refuse a
   position for which `isValid()` is false or `toString()` differs),
   `setPercentRead`, `updateFields` with only the changed columns, then
   **drop the target chapter's cached pages** if none holds the place
   (`drop_stale_pages`' rule, on `dbConnection(db, false)` in the same
   transaction), `endTransaction`; on any failure roll back and
   `refreshCache<Volume>`. Then hand the original a `Volume` from a fresh
   `getById`. This replaces the CLI's `--write`, the `Volume` copy and the
   setters on Nickel's `Volume`; `purgeCache` is not needed.
2. **CLI:** `pull --live --emit` decides and names the place (a machine
   line with position, percentage and the decision) without writing; the
   day's backup of the database must still be made before the plugin
   writes, so the CLI makes it on a decision to take (or a flag asks for
   it). Since C2 `pull --write` also sets the book's base in `state.json`
   (`device` = the written place); when the plugin writes, the base has to
   be set only once its commit succeeded, so the plugin reports back (a
   second CLI call, or the plugin updates `state.json` with the same
   re-read, tmp and rename rule as the CLI).
3. **Kobalt guard:** the Tolino check in `init()` goes; `kobalt_blocks()`
   runs first in every entry point (opening, leaving, book menu,
   NickelMenu trigger, pending list, dialog).
4. **Book menu and dialog** as above; the conflict dialog's third button
   ("later") needs a Nickel widget, not a `QPushButton`.
5. **Device test:** the case the VM can only approximate is the first
   opening after a restart with the chapter start cached (C, G): Book R,
   read the start of a chapter, leave, have KOReader move further into
   that chapter, restart the Kobo, open. And one opening of a book already
   opened in that session, to see whether Nickel's in-memory pages undo the
   drop (the device test of 31.10.).

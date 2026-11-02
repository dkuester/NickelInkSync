# libnickel symbols for Kobo firmware 4.38 (Qt5)

What the Qt5 build of the plugin (stage C9 in [TODO.md](../TODO.md),
replaces N1) needs from Nickel on Kobo firmware **4.38.23852**, the
firmware of the Kobo Libra 2 we test 4.x on: the symbols the plugin uses
today, Kobalt's Qt5 write path, and the symbols for toasts, Wi-Fi, a
dialog and a book menu entry. Checked on 2026-11-01 against the library
(signatures by disassembly), not on the device yet. The Qt6 analyses are in
[nickel-6.0-symbols.md](nickel-6.0-symbols.md) (Kobo 6.0, stage C0) and
[nickel-5.18-symbols.md](nickel-5.18-symbols.md) (Tolino 5.18).

## Sources

| | |
|---|---|
| Firmware | `kobo-update-4.38.23852.zip` for the Libra 2 (`kobo9`, device ID `…388`), 121,134,636 bytes, from `https://ereaderfiles.kobo.com/firmwares/kobo9/Oct2026/`, the URL Kobo's update API gives for that device (`api.kobobooks.com/1.0/UpgradeCheck/Device/00000000-0000-0000-0000-000000000388/kobo/4.38.23552/N0`). Downloaded with the owner's consent; not in this repository. |
| Library | `usr/local/Kobo/libnickel.so.1.0.0` from its `KoboRoot.tgz`: 24,019,884 bytes, 70,873 `_Z` dynamic symbols. |
| Qt | Qt 5.2.1 (`usr/local/Qt-5.2.1-arm/lib/libQt5*.so.5` are links to `usr/local/Trolltech/QtEmbedded-4.6.2-arm/lib/libQt*.so.4.6.2`, a historical name). Kobo's build has some later additions, for example `QTimer::singleShotImpl` (the functor `singleShot` of Qt 5.4). |
| C library | `lib/libc-2.11.1.so`, by name glibc 2.11.1, but it exports symbol versions up to `GLIBC_2.18` (`pthread_setname_np@GLIBC_2.12`, which NickelHook uses, is there). |
| Reference | [`tools/libnickel-4.38.23852.syms.gz`](../tools/libnickel-4.38.23852.syms.gz): the `_Z` symbols of the 25 classes the plugin and the Kobo client use (1,585 lines), made with [`tools/nickel_symbol_reference.py`](../tools/nickel_symbol_reference.py) from `llvm-nm -D libnickel.so.1.0.0`. CI checks the plugin against it (`check_plugin_symbols.py --qt 5`). |
| Disassembly | Thumb-2, `llvm-objdump -d --triple=thumbv7a-linux-gnueabihf` from the even address for the size `llvm-nm -S` gives, as for 5.18 and 6.0. |
| Kobalt | [Kobalt](https://github.com/nicoverbruggen/kobalt) `main` @ `3d8514c`, its own code under MIT: `platforms/kobo/qt5/` (Kobalt supports 4.38.21908 and later). Read for the concept only; no code taken over. |

Calling convention as in the other two documents: `this` in `r0`; a class
returned by value goes through a result pointer in `r0`, and the arguments
move up by one.

## The plugin's symbols

The plugin has 23 symbols (two hooks, 21 `dlsym`). **18 are exported by
4.38 under the same names**, 5 are not:

| Qt6 symbol (5.18, 6.0) | On 4.38 | For the Qt5 build |
|---|---|---|
| `_ZNK7Content5getIdERK7QString` | missing | `_ZNK7Content5getIdEv`: `QString Content::getId() const`, by value (result pointer in `r0`, `this` in `r1`, ends in `QString::fromUtf8_helper` into the result); no argument. In the Qt5 build now. |
| `_ZNK8Bookmark6isNullEv` | missing | none. On 5.x/6.0 `isNull()` is `*this == Bookmark()` against a static empty `Bookmark` (tail call to `operator==`). 4.38 has both parts: `_ZN8BookmarkC1Ev` and `_ZNK8BookmarkeqERKS_` (`bool operator==(Bookmark const&) const`, compares the shared data pointers, then the fields). C10: compare with one default-constructed `Bookmark`. |
| `_ZN6VolumeC1ERKS_` (copy) | missing (only `C1Ev`, `C2Ev`, `D0`–`D2`) | C4 drops the copy anyway (a fresh `VolumeManager::getById` instead, see the C0 recommendation). Otherwise `QtMetaTypePrivate::QMetaTypeFunctionHelper<Volume, true>::Create(void const*)` copy-constructs onto the heap (`operator new(8)`, vtable, shared data with its reference count raised), `…::Delete(void*)` frees it. |
| `_ZNK8Bookmark16getStartLocationEv` | missing | 4.38 has no `ContentLocation` class at all. A `Bookmark` keeps the place as strings: `getChapterFilePath()` (`QString` by value) and `getStartContainerPath()` (a reference, `d + 0x1c`), plus `getStartContainerChildIndex()`. Which of them gives Nickel's `ChapterIDBookmarked` form (`OEBPS/x.xhtml#point(...)`, kepub `#kobo.N.M` without an offset) is for C10 to log on the Libra 2. |
| `_ZNK15ContentLocation8toStringEv` | missing | as above |

The 18 that stay, with what differs on 4.38:

| Symbol | On 4.38 (checked) |
|---|---|
| `_ZN17ReadingController9setVolumeERK6VolumeRK8Bookmark` (hook) | exported; `void` |
| `_ZN11ReadingView24leavingReadingExperienceEv` (hook) | exported; calls `saveBookmark` and `Content::setDateLastRead` itself, as on 5.18 |
| `_ZN6Device16getCurrentDeviceEv` | `Device&` |
| `_ZNK6Device8isTolinoEv` | **always `false`** (`movs r0, #0; bx lr`): there is no Tolino on 4.x; `init()`'s Tolino check would switch the plugin off on every 4.x device |
| `_ZN6VolumeD1Ev` | exported. **A `Volume` is 8 bytes** on 4.38 (vtable, shared data; `QMetaTypeFunctionHelper<Volume>::Create` allocates 8), 12 on 5.18 and 6.0. The plugin's 64-byte buffers fit both |
| `_ZN7Content22setChapterIdBookmarkedERK8QVariant`, `_ZN7Content14setPercentReadERK8QVariant` | exported |
| `_ZN20MainWindowController14sharedInstanceEv`, `_ZNK20MainWindowController11currentViewEv` | exported; `currentView` returns a field (`ldr r0, [r0, #0x44]`) |
| `_ZN11ReadingView26bookmarkForCurrentLocationEv` | `Bookmark` by value |
| `_ZN8BookmarkD1Ev` | exported. **A `Bookmark` is 8 bytes** (vtable from `ScObject`, shared `BookmarkPrivate`) |
| `_ZN23WirelessWorkflowManager14sharedInstanceEv`, `…20isInternetAccessibleEv`, `…15connectWirelessEbb` | exported; `isInternetAccessible` is `WirelessManager::connected()` and a flag; `connectWireless(bool, bool)` as on 5.18. There is no `connectWirelessSilently` on 4.38 (the plugin never used it) |
| `_ZN20MainWindowController5toastERK7QStringS2_i` | `void`, still `adds r0, #0x14` and a tail call to the toaster (`N3ToasterController::queueMeSomeToast`, exported) |
| `_ZN13VolumeManager7getByIdERK7QStringS2_` | `static Volume getById(QString const&, QString const&)`, by value |
| `_ZNK6Volume7isValidEv` | `Content::idBytes()` not empty |
| `_ZNK7Content11getDbValuesEv` | exported |

## Kobalt's write path on 4.38

Kobalt's Qt5 code does the same sequence as on 6.0 (C0) with three
differences in the symbols, all exported by 4.38:

| Purpose | Symbol on 4.38 | Difference from 6.0 |
|---|---|---|
| Database name | `_ZN6Device16getCurrentDeviceEv`, `_ZNK6Device9getDbNameEv` | none; `getDbName` is a reference (`d + 0x30`, 6.0: `d + 0x70`) |
| Begin, end | `_ZN15DatabaseAdapter16beginTransactionE7QString`, `_ZN15DatabaseAdapter14endTransactionE7QStringb` | none; `beginTransaction` refuses a nested transaction (`hasTransaction`), runs the `BEGIN` statement and starts the transaction timer |
| Nickel's connection | `_ZN15DatabaseAdapter12dbConnectionE7QStringb` | takes the name **by value** (`QString`), 6.0 by reference (`…ERK7QStringb`) |
| The book, its columns | `_ZN13VolumeManager7getByIdERK7QStringS2_`, `_ZNK6Volume11getDbValuesEv` | none |
| Set the place | `_ZN13VolumeManager14updateBookmarkER6VolumeRK7QStringiRK9QDateTime` | **no trailing `int`** (6.0: `…RK9QDateTimei`, the Tolino page). Only in memory, as on 6.0: `ChapterIDBookmarked`, the `int` (Kobalt passes `adobe_location`), `DateLastRead`, `ReadStateSynced = false` |
| Build the place | none | no `ContentLocation::create`; Kobalt passes the position string itself (chapter path `#` element ID, no text offset on 4.x) |
| Set the percentage, status | `_ZN7Content14setPercentReadERK8QVariant`, `_ZN7Content13setReadStatusE13ReadingStatus`, `_ZN7Content19setFirstTimeReadingERK8QVariant` | none |
| Store | `_ZN13VolumeManager12updateFieldsERK7QStringRK4QMapIS0_8QVariantES7_S2_` | none |
| After a failure | `_ZN10Repository12refreshCacheI6VolumeEEP5CacheIT_ERK7QString` | none; `purgeCache<Volume>` is not exported (not needed) |
| Layout check | `_ZN17QtMetaTypePrivate23QMetaTypeFunctionHelperI6VolumeLb1EE6CreateEPKv` | Kobalt requires `QMetaType::sizeOf(Volume) == 8` on Qt5 |

So the write path C4 builds on 6.0 carries over to 4.38 if the code keeps
`dbConnection`, `updateBookmark` and the place string behind the Qt
switch. The page cache question (N5, the target chapter's cached pages) has
to be checked on 4.x again: stage C8 checks it for the USB path.

## Dialog, book menu, toast

| Purpose | Symbol on 4.38 | Difference from 6.0 |
|---|---|---|
| Book menu (hook) | `_ZN16ActionProxyMixin17setupBookListMenuEP28AbstractNickelMenuControllerRK6VolumeP15NickelTouchMenuRK7QStringS9_RK3Tab` | **no `bool`** between the second string and the `Tab` (6.0: `…S9_bRK3Tab`) |
| Menu item, its action | `_ZN28AbstractNickelMenuController18createMenuTextItemEP5QMenuRK7QStringbbS4_`, `_ZN22AbstractMenuController12createActionEP5QMenuP7QWidgetbbb` | none |
| Dialog | `_ZN25ConfirmationDialogFactory21getConfirmationDialogEP7QWidget` | none: `new ConfirmationDialog(parent)` |
| Its texts, more content | `_ZN18ConfirmationDialog8setTitleERK7QString`, `…7setTextERK7QString`, `…19setAcceptButtonTextERK7QString`, `…19setRejectButtonTextERK7QString`, `…9addWidgetEP7QWidget` | none |
| Other dialogs | `ConfirmationDialogFactory::showOKDialog`/`showErrorDialog(QString const&, QString const&)`, `N3Dialog(QWidget*, bool)`, `N3DialogFactory::getDialog(QWidget*, bool)` | none |
| Reading menu | `_ZN15ReadingMenuViewC1EP7QWidgetb` | one `bool` (5.x/6.0: `…EP7QWidgetbb`); the 4.x-only `(QWidget*, QByteArray const&, bool)` constructor is gone on 4.38 too. The plugin does not use it |
| Toast | `MainWindowController::toast`, see above | none |
| Opening a book | `_ZN14N3ReaderOpener10openReaderERK8Bookmark`, `_ZN11ReadingView12goToBookmarkERK8Bookmark` | none |

NickelMenu upstream (pgaskin, Qt5) runs on 4.x, so on the Libra 2 the
NickelMenu entry of C6 can use the original NickelMenu, not the Qt6 fork.

## The Qt5 build

The same `plugin/src/nickelinksync.cc` builds for Qt 5 with
`NH_QT_MAJOR=5` in NickelTC (`strayrose/nickeltc`, prefix
`arm-nickel-linux-gnueabihf-`, GCC 4.9.4, `-std=gnu++11`, Qt 5.2.1 headers
and libraries): `just plugin-qt5` (`tools/build_plugin_qt5.sh`), on a copy
in `target/plugin-qt5` so it never mixes objects with the Qt6 build, gives
`plugin/KoboRoot.tgz` with `./usr/local/Kobo/imageformats/libnickelinksync.so`
(the 4.x layout; 4.x installs `KoboRoot.tgz` from `.kobo/`, not `Kobo.tgz`).
The differences are `#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)` blocks:

- `Content::getId` with or without the `QString` argument, and the
  `dlsym` names: the Qt5 table has the 4.38 name for `getId` and leaves out
  the four symbols 4.38 does not export (see the first table). Until C10
  replaces them, the Qt5 build must not reach code that calls them.
- `Qt::SkipEmptyParts` (Qt 5.14) or `QString::SkipEmptyParts`.
- `QProcess::errorOccurred` (Qt 5.6) or Qt5's `error(QProcess::ProcessError)`
  signal; `finished` through a cast to `(int, QProcess::ExitStatus)`, which
  both have.
- No C++14 lambda capture initialiser (GCC 4.9 in C++11 mode).

On 4.x `init()` runs the Kobalt guard (Kobalt installs into the same
`imageformats/` and parks itself as `/usr/local/Kobo/libkobalt.so.failsafe`
like on 6.0, so `src/kobalt.rs`' paths hold), logs
`inactive: Kobo firmware 4.x (Qt 5.2.1), plugin loaded; syncing on 4.x
follows later` and returns; the hooks only call the originals. Behaviour
(opening, leaving, pending list) follows in C10 after C4.

Every undefined symbol of the built library resolves against 4.38's
`libnickel.so.1.0.0`, `libQt5Core`/`libQt5Gui` and its C library (checked
with `llvm-nm`/`llvm-objdump -T`); it needs `GLIBC_2.4` and `GLIBC_2.12`,
`GLIBCXX_3.4`, `CXXABI_1.3`. CI (job `check`) runs the symbol check
against the reference, builds it, and checks ARM, `NEEDED libQt5Core.so.5`,
no Qt 6, the two exported handlers and the `KoboRoot.tgz` layout (Qt 5.2.1
has no symbol versions, so there is no `@Qt_5` to look for). The Qt6
`Kobo.tgz` is unchanged.

A `KoboRoot.tgz` put on a 6.x Kobo installs a Qt5 library that Nickel
(Qt 6) cannot load; the device package and the READMEs have to say which
file is for which firmware.

## Recommendation for C10

1. Start from C4's plugin; keep every Qt5/Qt6 difference in `#if
   QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)` blocks so
   `tools/check_plugin_symbols.py --qt 5|6` keeps seeing both tables.
2. Replace on Qt5: `Bookmark::isNull` by `operator==` against a default
   `Bookmark`; the `Volume` copy by a fresh `getById` (as C4 does) or the
   `QMetaType` helper; `current_location` by the `Bookmark` strings (log
   `getChapterFilePath`, `getStartContainerPath` and Nickel's
   `ChapterIDBookmarked` once on the Libra 2 to pick the form).
3. The write path: `dbConnection(QString, bool)`, `updateBookmark` without
   the last `int`, the place string without `ContentLocation`; 4.x kepub
   positions have no offset (C8).
4. The book menu hook: the 4.38 symbol without the `bool`.
5. Drop the Tolino check for both builds (C4 does it for 6.0; on 4.x
   `isTolino()` is always false).
6. The release workflow's device package gets `KoboRoot.tgz` next to
   `Kobo.tgz` (`tools/build_plugin_qt5.sh`, see the C9 handover note).

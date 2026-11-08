# Reading a Kindle over MTP: feasibility study

Question (owner, 2026-11-08, release 0.2.0): can NickelInkSync read a
stock Kindle (no jailbreak) directly over USB on macOS and Windows, read
only, instead of asking the user to copy the `documents` folder by hand with
OpenMTP (README, section Kindle; `TODO.md` L1)? Test devices: the owner's
Kindle Paperwhite 11 (firmware 5.19.2) and a new Kindle Basic 2026.

State of this study: **2026-11-08**. Source reading and metadata only
(GitHub API, crates.io API, the projects' READMEs, changelogs and
manifests). Nothing was downloaded, built or run, and no device was touched.
Everything marked *unverified* has to be checked on the device.

## Result in short

- **Yes, and no new native dependency is needed.** `vdavid/mtp-rs` is a
  pure-Rust MTP library with a Windows (WPD) backend and a macOS/Linux USB
  backend (`nusb`). Its README lists a **Kindle Paperwhite 12th generation
  (2024)** as fully supported, and two Kindle fixes came from users with
  Kindles (an "11th gen Kindle" not detected, issue #1, fixed in 0.4.1; the
  session-less `OpenSession` Kindles need, PR #2).
- **Licence: fine.** `mtp-rs` is **MIT OR Apache-2.0** (GitHub shows only
  "Apache-2.0"; the repository holds both licence files and the manifests say
  `MIT OR Apache-2.0`). All transitive runtime dependencies are MIT,
  Apache-2.0, BSD, Zlib, Unlicense or dual combinations of these; none is
  GPL, LGPL or MPL. Details below.
- **Your guess about Windows is right for raw USB, but it does not matter.**
  Windows binds MTP devices to its WPD driver, so a raw-USB library only
  reaches a device that was rebound to WinUSB (Zadig). `mtp-rs` therefore
  uses WPD itself on Windows ("no driver install, no Zadig"). The raw-USB
  path stays only for cameras and rebound devices.
- **macOS is the hard part, not the code.** `ptpcamerad` (and Android File
  Transfer, OpenMTP, `adb`, Image Capture) can hold the device; then opening
  fails or times out. The library reports `Error::is_exclusive_access()`; the
  client must say who holds the device and what to do.
- **Maturity is the real risk.** `mtp-rs` is 9 months old, 0.x with breaking
  minors, one main author (340 of about 370 commits), written with AI
  assistance (the README says so), and the Kindle is tested by third
  parties only (a Paperwhite 12th generation, plus a 11th generation that was
  fixed to be detected). Our use is small (list, download, read only), and the fallback
  (the existing copy path) stays.
- **Recommendation: build on `mtp-rs`, behind a feature flag, in four
  stages (M1 to M4), about 6 to 9 working days, with the first stage as a
  go/no-go on the owner's Paperwhite 11.**

## 1. Candidates

| Candidate | Licence | Language, native deps | Platforms | Maintenance (2026-11-08) | Verdict |
| --- | --- | --- | --- | --- | --- |
| [`mtp-rs`](https://github.com/vdavid/mtp-rs) 0.38.0 (crates.io, 2026-11-06) | **MIT OR Apache-2.0** | Rust; none (USB through `nusb`, Windows through the `windows` crate) | macOS, Linux (USB); Windows (WPD); Linux/macOS/Windows CI | Created 2026-01-30; 49 stars, 12 forks, 3 open issues (one of them the dependency dashboard); 38 minor releases in 9 months; last push 2026-11-06; 340 of about 370 commits by one author | **Use** |
| [`libmtp`](https://github.com/libmtp/libmtp) 1.1.23 (2026-02-15) | **LGPL-2.1** | C; needs `libusb`, `libudev` on Linux | Linux, macOS (via libusb); Windows only through libusb (so a WinUSB driver), as far as its README says | Alive (push 2026-11-06), releases about yearly; the reference implementation for device quirks | LGPL: only dynamic linking and exchangeable; we would have to ship and build a C library on two platforms. **Not worth it** |
| [`libmtp-rs`](https://github.com/quebin31/libmtp-rs) 0.7.7 | MIT (bindings); the linked `libmtp` stays LGPL-2.1 | Rust bindings over `libmtp-sys` 1.1.17-5 (MIT); needs the C library at build time (pkg-config) | as `libmtp` | **Dead**: crate from 2020-10, last commit 2022-12, 8 stars | **No** |
| [`windows`](https://github.com/microsoft/windows-rs) crate 0.62.2, WPD (`Win32_Devices_PortableDevices`) | MIT OR Apache-2.0 | Rust, COM bindings, no native build | Windows only | Very active (Microsoft) | Possible for Windows alone, but `mtp-rs` already contains a tested WPD backend on exactly this crate; writing our own is about 500 to 1,000 lines of COM code for no gain |
| [`winmtp`](https://github.com/daladim/winmtp) 0.3.2 | MIT | Rust over WPD | Windows only | 4 stars, last push 2026-03 | Smaller than `mtp-rs`'s backend, Windows only; no reason to prefer it |
| [`nusb`](https://github.com/kevinmehall/nusb) 0.2.9 (raw USB) | Apache-2.0 OR MIT | Rust | macOS, Linux, Windows (WinUSB devices only) | Active | Only as the base under `mtp-rs`; writing our own MTP protocol on it would be a re-implementation |

Not considered: Android File Transfer and OpenMTP (programs, not libraries;
AFT is closed source), `mtp-rs-cli` as a bundled program (it works, but adds
a second binary per platform and a process boundary for no gain).

### Licence check of `mtp-rs` including transitive dependencies

Read from `Cargo.toml` and `Cargo.lock` of the repository (199 locked
packages, many of them dev-dependencies and benchmarks) and the licence
field of each package that the normal dependencies pull in (75 package
versions; queried from the crates.io API, nothing built):

| Licence expression | Count | Examples |
| --- | --- | --- |
| MIT OR Apache-2.0 / Apache-2.0 OR MIT / MIT/Apache-2.0 | 61 | `nusb`, `futures*`, `async-trait`, `thiserror`, `windows*`, `libc`, `core-foundation`, `io-kit-sys`, `serde`, `syn`, `indexmap`, `bitflags` |
| MIT | 3 | `bytes`, `slab`, `winnow` |
| MIT OR Apache-2.0 plus a third option (BSD-2/-3, LLVM exception) | 5 | `mach2`, `num_enum`, `rustix`, `linux-raw-sys` |
| Unlicense OR MIT | 1 | `memchr` |
| (MIT OR Apache-2.0) AND Unicode-3.0 | 1 | `unicode-ident` |
| Zlib | 1 | `foldhash` (through `hashbrown`) |
| not returned by the query (build-metadata version string) | 3 | `toml_edit`, `toml_datetime`, `toml_parser`: the current releases are MIT OR Apache-2.0 on crates.io; **recheck with `cargo license` in CI** |

No GPL, AGPL, LGPL or MPL anywhere. The project enforces the same list
itself: its `deny.toml` allows only MIT, Apache-2.0 (also with LLVM
exception), BSD-2-Clause, Unicode-3.0, CC0-1.0 and ISC. (`foldhash` is Zlib,
which `deny.toml` does not list; it comes in through `hashbrown` for the
proc-macro/wasm side and is permissive anyway.) What is actually linked into
a macOS or Windows binary is much smaller than the table: `mtp-rs`, `nusb`,
`futures`/`futures-timer`, `bytes`, `thiserror`, `num_enum`, `async-trait`,
`io-kit-sys`/`core-foundation`/`mach2` (macOS), `windows`/`windows-core`
(Windows), `libc`/`rustix`.

Two facts specific to our tree: `Cargo.lock` already contains `windows`
0.62.2 and `thiserror` 2.0.21, so `mtp-rs`'s exact pin `windows =0.62.2`
costs no second copy today; a later `eframe` upgrade that moves `windows`
would have to move in step (a build error, not a silent problem). `nusb` is
new to the tree. Our MSRV (1.97) is above theirs (1.90).

Obligations: MIT and Apache-2.0 both require keeping the copyright and
licence notices; Apache-2.0 adds that a `NOTICE` file, if the upstream has
one, must be passed on. The `mtp-rs` repository has **no `NOTICE` file**
(top level listing: `LICENSE-APACHE`, `LICENSE-MIT`, no `NOTICE`), so we owe
only the notice texts. Because we may choose, we take it under **MIT**, the
same as our own licence, and keep the Apache text in the package too. The
dependencies' notices go where the other third-party licences go today (the
About window and the disk image, README table "Third-party code"); a generated
`THIRD-PARTY-LICENSES` file from `cargo about` or `cargo license` in the
release workflow is the clean way (stage M4).

## 2. Technical questions

### 2.1 Windows

- Windows loads its own MTP class driver (WPD) for an MTP device. A raw-USB
  library like `nusb` only sees devices whose interface is bound to
  **WinUSB**; changing that needs an admin and a tool such as Zadig, and it
  breaks Explorer's access to the device. `mtp-rs`'s README says exactly
  this: the low-level `ptp::` API is WinUSB-only; phones are bound to WPD
  and are reached through the high-level `mtp::` API, which selects the
  **WPD COM backend** automatically on Windows. So WPD it is, and `mtp-rs`
  brings it (pure Rust over the `windows` crate, `cfg(windows)`-gated, a
  Windows CI job, verified on a Pixel 9 Pro XL and two Xiaomi phones and
  Samsung devices on Windows). **No Kindle test on Windows is documented**
  (the Kindle entry is "full support", reported from a Mac): *unverified*.
- Known Windows trap (their README and issue #52): WPD can stall a call
  for 5 to 6 minutes after files changed on the device, as Explorer does.
  We must wrap every call in a timeout and show "Windows is slow to read
  this device".
- COM needs a thread with COM initialised; the library handles it in its
  own worker. We call it from our worker thread and block on the future
  (`futures::executor::block_on`; no `tokio` is required, the library is
  runtime-agnostic).

### 2.2 macOS

- macOS's `ptpcamerad` claims PTP/MTP interfaces when a device is
  connected. A Kindle appears as an imaging device to it. Then `nusb` cannot
  claim the interface: "device is already in use" (exclusive access) or "open
  device - Timeout". Other holders: Android File Transfer, OpenMTP (the
  tool we recommend today!), `adb`, Cmdr, a second `mtp-rs` process, and a
  locked screen on phones (not a Kindle issue).
- Their workarounds: a kill loop for `ptpcamerad` (works as the user, no
  `sudo`; the daemon respawns at once, so a loop is needed), or
  `sudo launchctl disable system/com.apple.ptpcamerad` (system-wide; may
  break Photos/Image Capture). We should **not** disable a system service
  from our app. Proposal: detect `Error::is_exclusive_access()`, read the
  holder from the IORegistry (`UsbExclusiveOwner`, as the library's README
  suggests) and show a plain message, for example "Another program is using
  the Kindle: OpenMTP (quit it) or the macOS camera service. [Try again]
  [How to free it]"; the app does not stop the process itself (owner decision 2026-11-08, section 5).
- The app is not sandboxed or App-Store distributed (a `.dmg`), so USB
  access needs no entitlement. First access may show a macOS prompt for USB
  accessories on recent macOS versions: *unverified*, to see at the first
  test.
- Unplugging at the wrong moment shows as a generic I/O error (issue #53,
  closed): map it to "Kindle disconnected".

### 2.3 What does the Kindle's MTP storage look like?

From the copy of the Paperwhite 11 made in the feasibility step 1
(2026-10-24) and `src/kindle_device.rs`:

```
documents/
  Downloads/Items01/book-<x>_<32hex>.kfx      # Send-to-Kindle, DRM-free KFX
  Downloads/Items01/book-<x>_<32hex>.sdr/     # reading state
      <name><devhash>.yjf                     # KRDS: lpr (last page read), fpr, ...
      (.yjr, ...)
  ... (store-bought books: encrypted, skipped)
```

The Paperwhite 12th generation in the `mtp-rs` issue #8 exposes **2,541
objects, 23 in the root**; the root listing needed a fix (the Kindle answers
`GetObjectHandles(parent=0)` with every object; since PR #9 the library asks
with `0xFFFFFFFF` first). Its vendor extension string is
`microsoft.com/WMDRMPD:10.1;microsoft.com/playready:1.10`, no `android.com`.

*Unverified, to check at the first device test (M1):* that the MTP view of
the Paperwhite 11 shows the **same tree** (`documents/...`) **including the
`.sdr` folders and the hidden-looking dot-files**. Amazon's MTP responder is
a layer over the file system and may hide folders. If `.sdr` is not visible,
the position cannot be read over MTP and the feature is dead for
this device generation; this is the go/no-go of M1. Also unverified: that
`.yjf` files can be read while the Kindle is in use or sleeping.

### 2.4 Is read-only copying into a temporary folder enough?

Yes, for the design the code already has: `kindle_device.rs` accepts "a
folder with `documents/`, or any folder with the KFX files and their `.sdr`
folders". The plan is:

1. Open the device, find the storage, list `documents/` recursively with a
   cancel token and a timeout.
2. Download only what is needed: all `.sdr/*.yjf` (small) and `.kfx`
   **headers/ASIN only if needed**. The `.kfx` files are large
   (megabytes). Reading a KFX is needed for mapping and title (done by
   `src/kfx.rs`); positions need only the `.sdr` files plus the already
   mapped book. Option to download KFX on demand (only for books not yet in
   `mappings.json`).
3. Write into a temporary folder (`$TMP/nickelinksync-kindle-<serial>/
   documents/...`), keep it between runs (cache keyed by object handle,
   size and modification time; the library returns dates), pass it as
   `--device`.
4. Nothing is written to the Kindle; the library API for writes is never
   called.

Size and time: a few hundred books of 1 to 5 MB take several minutes over
USB 2 at the first read; later runs read only changed `.sdr` files. The
library reports speed (Kindle: USB 2.0).

## 3. Stages, effort, risks

| Stage | Content | Effort | Owner test |
| --- | --- | --- | --- |
| **M1 Spike, go/no-go** | New optional module `src/kindle_mtp.rs` behind a Cargo feature (`mtp`), dependency `mtp-rs =0.38.x` (exact pin), read-only: list `documents/`, copy `.sdr` and (on demand) `.kfx` to a temp folder; CLI `nickelinksync kindle-copy --to <folder>` (name open) that prints what it found. macOS first. | 2 to 3 days | **du:** Paperwhite 11 and Kindle Basic 2026 at the Mac, quit OpenMTP first; run the command, send me the output. Decides: is `.sdr` visible? |
| **M2 Windows** | Same code through the WPD backend; timeouts for stalls; CI builds the Windows target with the feature (compile only). | 1 to 2 days | **du:** the same command on a Windows PC with the Paperwhite |
| **M3 Integration** | `--device kindle:` (or auto-detect) in `status`/`push` with the temp-folder cache; client: detect the Kindle in the device list, messages for "in use by another program" and "disconnected", progress and cancel; the OpenMTP instructions in the README become the fallback. | 2 to 3 days | **du:** end to end on both systems: map a book, push a position, KOReader sees it |
| **M4 Licences and release** | `THIRD-PARTY-LICENSES` from `cargo about` in the release workflow (also for the existing crates), notices in the app's About window and the packages, README (Kindle section, macOS and Windows hints), release notes ("experimental"), `cargo deny` job with an allow-list (MIT, Apache-2.0, BSD-2/3, ISC, Unicode-3.0, Zlib, CC0, Unlicense). | 1 day | none |

Total about **6 to 9 working days**; M1 is cheap enough to stop after it.
Stages M1 and M2 can be one owner appointment at the Mac and one at the
Windows PC; both can be bundled with the 0.2.0 Windows visual check (block D).

### Risks

| Risk | Likelihood | Mitigation |
| --- | --- | --- |
| `.sdr` not visible over MTP on this Kindle | unknown (critical) | M1 decides before anything else is built; fallback: the OpenMTP copy stays documented |
| `ptpcamerad`/other holders on macOS make it flaky | high | clear message, retry, documented manual step; the app never stops processes or touches system settings |
| Library churn: 0.x, breaking minors (0.36 to 0.38 within weeks), AI-assisted code, one maintainer | medium | exact version pin, only about 6 API calls used, update by hand after reading the changelog; read-only use limits the damage; if the library is abandoned we fork (permissive licence) |
| Windows WPD stalls (5 to 6 min) | medium | timeouts, a progress message, cancel |
| No documented Kindle test on Windows | medium | M2 test; Explorer should show the Kindle as a portable device, so WPD should see it |
| New dependency closure in a security-sensitive tool (`nusb`, `windows`, about 60 crates) | low | mostly shared with `eframe`; `cargo deny`/`cargo audit` in CI; optional Cargo feature so the CLI without MTP stays small |
| Build time and binary size | low | feature flag; measure in M1 |
| Firmware change by Amazon (5.19.x to later) | medium over time | the Kindle stays "experimental" |

### Obligations checklist

- Keep `mtp-rs`'s copyright notice and the MIT text (we choose MIT), plus
  the notices of the dependencies that are linked (via `cargo about`).
- No `NOTICE` file upstream, so nothing to forward; the package carries a
  `THIRD-PARTY-LICENSES` file anyway.
- Do not copy libmtp's device quirk database (LGPL) into our code.
- Record the library in the README table of third-party code and in
  `SCOPE.md` (licence section).

## 4. Recommendation

Do **M1 first**, as a spike with an exact-pinned `mtp-rs` behind a Cargo
feature, and decide after the Paperwhite 11 test whether `.sdr` is
readable over MTP. If yes, continue with M2 to M4 for 0.2.0 and ship the
Kindle MTP reading as **experimental**. Do not use `libmtp` or
`libmtp-rs` (LGPL, C dependency, dead bindings), and do not write our own
WPD or MTP code.

## 5. Decisions of the owner (2026-11-08)

1. Scope as proposed: read only, into a temporary folder, through the
   existing `--device` path.
2. If M1 shows that `.sdr` is not visible over MTP, **the feature is dropped**
   for 0.2.0 (the OpenMTP copy stays documented).
3. On macOS the app **only explains** how to free the device (holder named,
   manual steps); it never stops `ptpcamerad` or any other process.
4. Test devices: the Paperwhite 11 (firmware 5.19.2) and a new **Kindle Basic
   2026** (not yet checked: model, firmware and whether it shows `.sdr` over
   MTP are *unverified*), plus a **Windows PC**.
5. The copy is **cached** between runs, with a **limit** on what is kept
   (total size and/or number of books, oldest removed first; the value is
   set in M3). The cache holds DRM-free KFX from Send-to-Kindle only, in the
   app's own cache folder, and the user can clear it.
6. The macOS paragraph in 2.2 therefore reads: no opt-in `pkill`.

## 6. M1 result (2026-11-08, `kindle-copy --list`, a Kindle with USB ID 1949:9981)

**Go.** Over MTP the Kindle shows its `documents/` tree including everything
that matters:

- 14 `.sdr` folders, 9 `.kfx` books, each `.sdr` with
  `<title>_<id><devicehash>.yjf` and `.yjr` (50 to 580 bytes), next to
  `.phl`, `.mf`, `AssetDownloadMetadata.meta`, `EndActions…asc`, `XRAY…`,
  `LanguageLayer….kll`, `data/.pagination.cache/` and `assets/` (with
  `voucher`, which marks store-bought, DRM-protected books). One device hash
  only (`689fa069…`).
- Send-to-Kindle books carry a 32-character id (`…_NKEAXN7….kfx`), store
  books an ASIN (`…_B005MN22QK.kfx`); there are also `.azw`/`.azw3` books
  (Mobipocket/KF8, reading state `.azw3f`; not supported) and the
  dictionaries under `documents/dictionaries/`.
- The root of the storage lists `fonts`, `documents`, `audible`,
  `screenshots`, `metadata.calibre`, `driveinfo.calibre`,
  `FILE_SYSTEM_ACCESSIBILITY_FLAG`.
- The serial number and firmware fields of the MTP device info are **empty**
  (so `--serial` cannot select by serial; the USB location is used).
- The listing finished without an error or a holder problem on the Mac; the
  time was not measured.

Consequences: the first version of `kindle-copy` would have fetched about 30
MB of attachments and caches per run; it now fetches only `.yjf`/`.yjr`.
Which Kindle model this was, and the result for the second Kindle, are still
to be noted. Writing (release 0.3.0) is a separate question: the `.yjf` is an
ordinary visible object, but nothing is known yet about writes.

## Sources

- [vdavid/mtp-rs](https://github.com/vdavid/mtp-rs): README, `crates/mtp-rs/README.md`, `Cargo.toml`, `Cargo.lock`, `deny.toml`, `CHANGELOG.md`, `docs/debugging.md`; issues
  [#1](https://github.com/vdavid/mtp-rs/issues/1) (11th gen Kindle not detected),
  [#8](https://github.com/vdavid/mtp-rs/issues/8) (root listing on a Kindle),
  [#52](https://github.com/vdavid/mtp-rs/issues/52) (WPD stalls),
  [#53](https://github.com/vdavid/mtp-rs/issues/53) (macOS disconnect error),
  PR [#2](https://github.com/vdavid/mtp-rs/pull/2) (session-less `OpenSession`).
- [crates.io `mtp-rs`](https://crates.io/crates/mtp-rs), [`nusb`](https://crates.io/crates/nusb), [`winmtp`](https://crates.io/crates/winmtp), [`libmtp-rs`](https://crates.io/crates/libmtp-rs), [`libmtp-sys`](https://crates.io/crates/libmtp-sys): licence and release data.
- [libmtp/libmtp](https://github.com/libmtp/libmtp) (LGPL-2.1), [quebin31/libmtp-rs](https://github.com/quebin31/libmtp-rs), [microsoft/windows-rs](https://github.com/microsoft/windows-rs), [kevinmehall/nusb](https://github.com/kevinmehall/nusb).
- Own notes: Kindle feasibility step 1 (2026-10-24), `src/kindle_device.rs`, README (Kindle).

# Nickel VM

Two virtual e-readers for trying the plugin without the device, each a
firmware rootfs running Nickel in a Docker container under qemu-user:

| VM | Device | Firmware |
| --- | --- | --- |
| `tolino` (default) | Tolino Shine Color | 5.18.279364, the Tolino's current one |
| `kobo` | Kobo Clara Colour | 6.0.279225 |

 A screenshot and a tap replace looking and touching, a fake
Wi-Fi the radio, and a small KOSync mock the server. Everything is driven from the shell, so a
test can be repeated.

It is not the device. Its Wi-Fi is a stand-in that always finds and joins one network; and everything is several
times slower than on the device. See [Limits](#limits) before trusting a result.

## Requirements

- Docker (Docker Desktop on macOS, Apple silicon or Intel), with ARMv7
  emulation (Docker Desktop has it).
- About 2 GB of disk per VM: the firmware download (330 MB) and the
  imported rootfs (1 GB); the helper images are shared.
- For the plugin: `just plugin` and `just build-device` (see the
  [justfile](../justfile)).

## Quick start

```sh
just vm image                 # download the firmware, import the rootfs, build the helpers
just vm up                    # start the mock and Nickel; the first start picks German
just vm shot                  # screen -> vm/state/tolino/screen.png
just vm install               # plugin, CLI and a config.ini for the mock, restart Nickel
python3 vm/mkepub.py vm/state/tolino/test.epub "VM Test Book" 5
just vm books vm/state/tolino/test.epub   # copy to the user partition and import
just vm tap 176 366           # open the first book on the home screen
just vm log                   # NickelInkSync.log and the plugin's syslog lines
just vm down
```

To run a second VM of the same kind beside one from another checkout (the
container names are fixed), set `NICKEL_VM_INSTANCE` (a suffix for the
containers) and `NICKEL_VM_KOSYNC_PORT` for every command, for example
`NICKEL_VM_INSTANCE=c4 NICKEL_VM_KOSYNC_PORT=8776 vm/vm.sh kobo up`.

These commands use the `tolino` VM. For the other one, name it first:
`just vm kobo image`, `just vm kobo up`, `just vm kobo shot` and so on.
Both can run at once (mock servers on `127.0.0.1:8765` for `tolino` and
`8766` for `kobo`). `just vm` without arguments lists all commands. Coordinates are portrait
screen pixels (1072 × 1448), the same as in a screenshot. Wait a few seconds
after a tap before taking a screenshot: Nickel draws in parts, and a
screenshot taken too early shows a half-drawn screen.

Each VM keeps its state in `vm/state/tolino/` or `vm/state/kobo/` (ignored
by git): `onboard/` is the user partition (`/mnt/onboard`, with
`.kobo/KoboReader.sqlite` and `.adds/NickelInkSync/`), `kosync.json` the
mock's documents. Delete the folder to start from a new device. The plugin
runs on both, writing positions through Nickel on the `kobo` VM as on a
real Kobo (stage C4). State from before
the split (files directly in `vm/state/`, a Tolino on 6.0) is not used any
more and can be deleted.

## A pull test

The mock keeps one JSON object per document in `vm/state/tolino/kosync.json`. Open
the book, turn a few pages and leave it (tap the middle, then "Zurück zur
Startseite"): the plugin pushes, `just vm log` shows "Fortschritt zum Server
gesendet", and `just vm kosync` the position. Then move it as another device
would: change `progress` to `/body[1]/DocFragment[3]/body[1]/p[20]/text()[1].0`,
`device` to `KOReader` and `timestamp` to a later time, then open the book:
the plugin pulls, and Nickel opens chapter 3 at paragraph 20.

## How it works

| Device part | In the VM |
| --- | --- |
| Firmware | `tolino`: the Shine Color's `qt6-update-5.18.279364` `update.tar`, linked from [mytolino.de](https://mytolino.de/software-updates-tolino-ereader/); `kobo`: `qt6-update-6.0.279225.zip` for the Clara Colour (the same board). Both come from Kobo's server and are checked against a SHA-256. The `rootfs.img` (ext4, zstd) becomes the Docker image `localhost/nickel-vm-rootfs:VERSION`. Nothing of the firmware is in this repository. |
| CPU | ARMv7 user-mode emulation (qemu-user through Docker's `linux/arm/v7` platform); the kernel is Docker's. |
| Hardware configuration | The `hwcfg` partition, written by [vm/hwcfg.py](../vm/hwcfg.py): board E60T00 (spa), MT8113T, 1448×1072 colour screen, brand Tolino (customer 0x21) or Kobo (0x09). `hwdetect` and Nickel read it and report `spaTolinoColour` on the `tolino` VM. |
| Screen | `/dev/fb0` is a plain file (`vm/state/VM/fb0.raw`, 32-bit). [vm/src/vmshim.c](../vm/src/vmshim.c), preloaded into Nickel, answers the framebuffer ioctls and accepts the e-ink update ioctls. Nickel's own `kobo` platform plugin draws into it; [vm/topng.py](../vm/topng.py) turns it into a PNG. |
| Touch | qemu-user translates neither uinput nor evdev ioctls, so `/dev/input/touchscreen` is a FIFO: [vm/src/vmtouch.c](../vm/src/vmtouch.c) writes input events into it, and the shim answers Qt's evdev queries on it. `just vm tap` sends it a command. |
| Aux partition | `/mnt/aux`, where Nickel keeps the images of its rendered pages (`rendering/`; their rows are in the database), is `vm/state/VM/aux/`, so it survives a restart as on the device. Before 1 November 2026 it was lost at every restart, and Nickel then rendered every page live. |
| Battery | `/sys/class/power_supply/battery` (6.0) and `bd71827_bat` (5.18) on a tmpfs: 80 %, not charging. |
| USB | `export-usb.sh` does nothing; `just vm usb` writes plug events into Nickel's udev FIFO and taps "Verbinden", so Nickel imports new books as after a USB session. |
| Wi-Fi | [vm/wifi/wifi.py](../vm/wifi/wifi.py) in a sidecar container that shares the VM's network: it renames `eth0` to `wlan0`, owns `name.marples.roy.dhcpcd` on the system bus (the dhcpcd-dbus API Nickel scans and connects through) and answers wpa_supplicant's control socket. One network, `VM-WLAN`, is known and joined after each scan, as wpa_supplicant would; on connecting it restores the address and default route, as dhcpcd would. Nickel's connectivity check then goes to the internet through Docker. The real `wpa_supplicant`, `dhcpcd` and `dhcpcd-dbus` are stubbed. |
| Power off, reboot | Replaced by [vm/stub.sh](../vm/stub.sh). |
| KOSync server | [vm/kosync_mock.py](../vm/kosync_mock.py) in its own container, reachable from the VM as `http://nickel-vm-kosync:8765` (and from the host on `127.0.0.1:8765`). Any user, any password. |
| Plugin | Copied into `/usr/local/Kobo/imageformats` at every start (NickelHook renames its library while it starts, which a mounted file does not allow). |

The helpers are built with the Kobo 5.x cross compiler from
[kobuild](https://github.com/nicoverbruggen/kobuild) (MIT), which runs
natively on arm64 and amd64. [vm/init.sh](../vm/init.sh) puts everything in
place inside the container and starts Nickel as `/etc/rc.local` does.

## What was checked

On 27 October 2026, on a Mac with Apple silicon: Nickel boots in Tolino mode
("TOLINO SHINE COLOR" on the home screen) in about 30 s; the first-start
screens, the library, importing an EPUB, opening it and turning pages work by
tap. The plugin loads, finds all its symbols in 6.0, sees a Tolino and runs
the CLI on opening; the CLI reaches the mock. With a newer position on the
mock, Nickel opened the book exactly there (chapter 3, paragraph 20). With
the fake Wi-Fi, Nickel shows "VM-WLAN – verbunden", opening finds the network
at once, and leaving pushes ("Fortschritt zum Server gesendet"); switching
Wi-Fi off and on in the status bar reconnects.

On 29 October 2026 the `tolino` VM on firmware 5.18.279364: Nickel boots to
the Tolino home screen once the battery is also faked as `bd71827_bat` and
the hardware event FIFO `/tmp/nickel-hardware-status` exists (5.18 reads
both, 6.0 neither). Importing a book, the plugin (from `main`), leaving
with a push, and a pull that opened the book exactly at the pulled place
(chapter 3, paragraph 20) all worked. The `kobo` VM on 6.0 boots to the
Kobo language screen; its first-start screens are not tapped through.

On 1 November 2026 the `kobo` VM on 6.0.279225 (stage C0): after the
language screen ("English") Nickel goes straight to the Kobo home screen,
with no further set-up. A NickelHook plugin loads; importing Book R as
kepub and EPUB, opening, turning pages, leaving ("Back to My Books" in the
reading menu), a book menu entry of a plugin and a `ConfirmationDialog`
all work by tap. The results are in
[nickel-6.0-symbols.md](nickel-6.0-symbols.md#the-write-path-in-the-vm).

## Limits

- **Firmware versions.** The `tolino` VM runs what the Tolino runs now
  (5.18.279364); when Tolino ships a new version, `FW_VERSION`, `FW_URL`
  and `FW_SHA256` in [vm/vm.sh](../vm/vm.sh) move to it. The `kobo` VM runs
  6.0. A result in a VM is a hint, not a device test.
- **Wi-Fi is a stand-in.** It joins its one network about a second after
  Nickel scans; there are no passwords, failures, weak signals or captive
  portals, and the timing has nothing to do with a real radio. When Wi-Fi was
  switched off in the status bar, the plugin's silent connect
  (`connectWireless(false, true)`) did not switch it on again: opening then
  pulled without network and failed. Whether Nickel's own idle switch-off
  behaves the same way was not tried. `VM_WIFI_TRACE=1 just vm up` logs
  every call Nickel makes (`just vm wifi`), `VM_DBUS_MONITOR=1` records the
  system bus in `vm/state/VM/dbus-monitor.txt`.
- **Timing.** Everything runs emulated, several times slower than on the
  device. Races between Nickel and the plugin show up differently or not at
  all; the VM cannot confirm or rule out a timing bug. For example, the
  plugin's check 3 s after opening found no position yet under emulation and
  reported "Nickel hat ein anderes Kapitel geöffnet", although Nickel had
  opened the pulled page.
- **Screen and log.** The screenshot sometimes lags one step behind,
  most often with menus and dialogs; take another one a few seconds
  later. The syslog is a 512 KB ring buffer that the renderer's messages
  fill within seconds of opening a book, so the lines of the opening are
  often gone; for such tests run
  `docker exec nickel-vm-kobo sh -c 'killall syslogd; syslogd -O /state/syslog.txt'`
  after the start (the log then is `vm/state/kobo/syslog.txt`).
- **No e-ink.** Update modes, waveforms and flashing are accepted and
  ignored; Nickel's light sleep is refused (it logs "Cannot enter suspend
  while charging" every few seconds, which is harmless).

# Nickel VM

A virtual Tolino Shine Color for trying the plugin without the device: Kobo's
firmware 6.0 rootfs, switched to Tolino, runs Nickel in a Docker container
under qemu-user. A screenshot and a tap replace looking and touching; a small
KOSync mock replaces the server. Everything is driven from the shell, so a
test can be repeated.

It is not the device. It runs firmware **6.0**, not 5.18; Nickel has **no
Wi-Fi** of its own; and everything is several times slower than on the
device. See [Limits](#limits) before trusting a result.

## Requirements

- Docker (Docker Desktop on macOS, Apple silicon or Intel), with ARMv7
  emulation (Docker Desktop has it).
- About 2 GB of disk: the firmware download (330 MB), the imported rootfs
  (1 GB) and two helper images.
- For the plugin: `just plugin` and `just build-device` (see the
  [justfile](../justfile)).

## Quick start

```sh
just vm image                 # download the firmware, import the rootfs, build the helpers
just vm up                    # start the mock and Nickel; the first start picks German
just vm shot                  # screen -> vm/state/screen.png
just vm install               # plugin, CLI and a config.ini for the mock, restart Nickel
python3 vm/mkepub.py vm/state/test.epub "VM Test Book" 5
just vm books vm/state/test.epub   # copy to the user partition and import
just vm tap 176 366           # open the first book on the home screen
just vm log                   # NickelInkSync.log and the plugin's syslog lines
just vm down
```

`just vm` without arguments lists all commands. Coordinates are portrait
screen pixels (1072 × 1448), the same as in a screenshot. Wait a few seconds
after a tap before taking a screenshot: Nickel draws in parts, and a
screenshot taken too early shows a half-drawn screen.

The state lives in `vm/state/` (ignored by git): `onboard/` is the user
partition (`/mnt/onboard`, with `.kobo/KoboReader.sqlite` and
`.adds/NickelInkSync/`), `kosync.json` the mock's documents. Delete
`vm/state/` to start from a new device. `NICKEL_VM_BRAND=kobo just vm up` on
a fresh state starts a Kobo Clara Colour instead; the plugin stays inactive
there, as on a real Kobo.

## A pull test

The mock keeps one JSON object per document in `vm/state/kosync.json`. Open
the book, turn a few pages and leave it (tap the middle, then "Zurück zur
Startseite"); the plugin cannot push without Wi-Fi (see [Limits](#limits)),
so push with the CLI in the VM. Then move the position as another device
would:

```sh
docker exec nickel-vm /mnt/onboard/.adds/NickelInkSync/nickelinksync push --live \
    --device /mnt/onboard --config /mnt/onboard/.adds/NickelInkSync/config.ini \
    --content-id file:///mnt/onboard/test.epub
just vm kosync
```

Change `progress` to `/body[1]/DocFragment[3]/body[1]/p[20]/text()[1].0`,
`device` to `KOReader` and `timestamp` to a later time, then open the book:
the plugin pulls, and Nickel opens chapter 3 at paragraph 20.

## How it works

| Device part | In the VM |
| --- | --- |
| Firmware | `qt6-update-6.0.279225.zip` for the Kobo Clara Colour (the same board as the Tolino Shine Color), downloaded from Kobo's server and checked against a SHA-256. Its `rootfs.img` (ext4, zstd) becomes the Docker image `localhost/nickel-vm-rootfs:6.0.279225`. Nothing of the firmware is in this repository. |
| CPU | ARMv7 user-mode emulation (qemu-user through Docker's `linux/arm/v7` platform); the kernel is Docker's. |
| Hardware configuration | The `hwcfg` partition, written by [vm/hwcfg.py](../vm/hwcfg.py): board E60T00 (spa), MT8113T, 1448×1072 colour screen, brand Tolino (customer 0x21; 0x09 for Kobo). `hwdetect` and Nickel read it and report `spaTolinoColour`. |
| Screen | `/dev/fb0` is a plain file (`vm/state/fb0.raw`, 32-bit). [vm/src/vmshim.c](../vm/src/vmshim.c), preloaded into Nickel, answers the framebuffer ioctls and accepts the e-ink update ioctls. Nickel's own `kobo` platform plugin draws into it; [vm/topng.py](../vm/topng.py) turns it into a PNG. |
| Touch | qemu-user translates neither uinput nor evdev ioctls, so `/dev/input/touchscreen` is a FIFO: [vm/src/vmtouch.c](../vm/src/vmtouch.c) writes input events into it, and the shim answers Qt's evdev queries on it. `just vm tap` sends it a command. |
| Battery | `/sys/class/power_supply/battery` on a tmpfs: 80 %, not charging. |
| USB | `export-usb.sh` does nothing; `just vm usb` writes plug events into Nickel's udev FIFO and taps "Verbinden", so Nickel imports new books as after a USB session. |
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
mock, Nickel opened the book exactly there (chapter 3, paragraph 20).

## Limits

- **Firmware 6.0, not 5.18.** Kobo publishes full images only for the
  current firmware, and Tolino's 5.18.264769 is not available as an image.
  6.0 is one firmware for Kobo and Tolino (the brand is a byte in `hwcfg`).
  Every symbol the plugin uses exists in 6.0, but layouts and behaviour may
  differ from 5.18. A result in the VM is a hint, not a device test.
- **No Wi-Fi for Nickel.** The container has a network (the CLI reaches the
  mock), but Nickel's Wi-Fi never comes up: there is no `wlan0`, and its
  `wpa_supplicant` and the `dhcpcd` D-Bus service (`name.marples.roy.dhcpcd`)
  do not run. So `isInternetAccessible()` stays false: opening waits the full
  10 s and then tries anyway, which works; leaving waits for Wi-Fi and never
  pushes, and Nickel's Wi-Fi dialog does not appear. Push from the shell
  instead (see [A pull test](#a-pull-test)). Making this work needs a fake
  `wpa_supplicant` control socket, a fake dhcpcd D-Bus service and an answer
  to Nickel's connectivity check (`/generate_204`).
- **Timing.** Everything runs emulated, several times slower than on the
  device. Races between Nickel and the plugin show up differently or not at
  all; the VM cannot confirm or rule out a timing bug. For example, the
  plugin's check 3 s after opening found no position yet under emulation and
  reported "Nickel hat ein anderes Kapitel geöffnet", although Nickel had
  opened the pulled page.
- **No e-ink.** Update modes, waveforms and flashing are accepted and
  ignored; Nickel's light sleep is refused (it logs "Cannot enter suspend
  while charging" every few seconds, which is harmless).

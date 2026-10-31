#!/usr/bin/env bash
# The Nickel VM: a firmware rootfs running Nickel under qemu-user in
# Docker. See docs/nickel-vm.md. Two VMs, each with its own state:
#
#   tolino   Tolino Shine Color, firmware 5.18.279364 (the default)
#   kobo     Kobo Clara Colour, firmware 6.0.279225
#
# The VM goes first and may be left out: vm/vm.sh kobo up, vm/vm.sh shot.
#
#   vm/vm.sh image            download the firmware, import the rootfs, build the helpers
#   vm/vm.sh up               start the KOSync mock and Nickel (first start: set-up screens)
#   vm/vm.sh down             stop both
#   vm/vm.sh restart          restart Nickel (keeps the user partition)
#   vm/vm.sh install          put plugin, CLI and a config.ini for the mock in place, restart
#   vm/vm.sh books FILE...    copy EPUBs to the user partition and import them
#   vm/vm.sh usb              plug and unplug USB: Nickel imports new books
#   vm/vm.sh tap X Y          touch the screen (portrait pixels, 1072x1448)
#   vm/vm.sh swipe X1 Y1 X2 Y2 [MS]
#   vm/vm.sh shot [FILE]      save the screen as PNG (default vm/state/VM/screen.png)
#   vm/vm.sh log              plugin log and Nickel's syslog lines of the plugin
#   vm/vm.sh syslog           the whole syslog
#   vm/vm.sh wifi             the fake Wi-Fi's log
#   vm/vm.sh kosync           the mock server's documents (vm/state/VM/kosync.json)
#   vm/vm.sh shell            a shell in the VM
#
# NICKEL_VM_QT_LOGGING sets Nickel's QT_LOGGING_RULES (e.g. "wifi.*=true"),
# VM_WIFI_TRACE=1 logs every call to the fake Wi-Fi, VM_DBUS_MONITOR=1 writes
# the system bus to vm/state/VM/dbus-monitor.txt.
set -euo pipefail

VM_DIR=$(cd "$(dirname "$0")" && pwd)
ROOT=$(dirname "$VM_DIR")
CACHE=$VM_DIR/cache
BUILD=$VM_DIR/build

PROFILE=tolino
case "${1:-}" in
    tolino | kobo)
        PROFILE=$1
        shift
        ;;
esac
# Both come from Kobo's server; the Shine Color and the Clara Colour are
# the same board (spa), and the brand is a byte in hwcfg.
case "$PROFILE" in
    tolino)
        BRAND=tolino
        FW_VERSION=5.18.279364
        FW_URL=https://ereaderfiles.kobo.com/firmwares/kobo12/Oct2026/qt6-update-$FW_VERSION/update.tar
        FW_FILE=qt6-update-$FW_VERSION.tar
        FW_SHA256=e5b0c7264b18c0fad2f7e9e0b6a574621ccb835edfa5fcb5c6e55f44f31be693
        KOSYNC_PORT=8765
        ;;
    kobo)
        BRAND=kobo
        FW_VERSION=6.0.279225
        FW_URL=https://ereaderfiles.kobo.com/firmwares/kobo12/Oct2026/qt6-update-$FW_VERSION.zip
        FW_FILE=qt6-update-$FW_VERSION.zip
        FW_SHA256=48b777179a79f68a5d1216e2a8ae20936f1bfacb6c5595351c9a6104b058f2ac
        KOSYNC_PORT=8766
        ;;
esac
STATE=$VM_DIR/state/$PROFILE
IMAGE=localhost/nickel-vm-rootfs:$FW_VERSION
# Native arm64 and amd64 image with the Kobo 5.x cross compiler.
KOBUILD=ghcr.io/nicoverbruggen/kobuild:qt6@sha256:b939301323e1bed0bf6b0bee43b8791db2efdf01231d4245c1f972d57433644a
NET=nickel-vm
VM=nickel-vm-$PROFILE
KOSYNC=nickel-vm-$PROFILE-kosync
WIFI=nickel-vm-$PROFILE-wifi
WIFI_IMAGE=localhost/nickel-vm-wifi
# Shared with the Wi-Fi sidecar: the system bus, wpa_supplicant's control
# socket and /tmp, where wpa_ctrl clients bind their reply sockets.
SHARED=(-v "$VM-dbus:/run/dbus" -v "$VM-wpa:/run/wpa_supplicant" -v "$VM-tmp:/tmp")

log() { echo "vm: $*" >&2; }

image() {
    mkdir -p "$CACHE" "$BUILD"
    local fw=$CACHE/$FW_FILE
    if [ ! -f "$fw" ]; then
        log "downloading firmware $FW_VERSION (330 MB)"
        curl -fL -o "$fw.part" "$FW_URL"
        mv "$fw.part" "$fw"
    fi
    echo "$FW_SHA256  $fw" | shasum -a 256 -c - >/dev/null || {
        log "checksum mismatch: $fw"
        exit 1
    }
    if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
        log "importing the rootfs as $IMAGE"
        # rootfs.img is a zstd-compressed ext4 image; debugfs copies it out
        # without a loop mount, tar keeps owners and links for the import.
        # 6.0 comes as a zip around update.tar, 5.18 as update.tar itself.
        docker run --rm -v "$CACHE:/cache:ro" debian:bookworm-slim sh -euc "
            apt-get -qq update >/dev/null
            apt-get -qq install -y unzip zstd e2fsprogs >/dev/null
            cd /tmp
            case $FW_FILE in
                *.zip) unzip -q /cache/$FW_FILE update.tar ;;
                *) ln -s /cache/$FW_FILE update.tar ;;
            esac
            tar -xOf update.tar rootfs.img | zstd -dq > rootfs.ext4
            mkdir root
            debugfs -R 'rdump / /tmp/root' rootfs.ext4 >/dev/null 2>&1
            tar -C root --numeric-owner -cf - .
        " | docker import --platform linux/arm/v7 - "$IMAGE" >/dev/null
    fi
    log "building vmshim.so and vmtouch"
    docker run --rm -v "$VM_DIR/src:/src:ro" -v "$BUILD:/out" "$KOBUILD" sh -euc '
        arm-kobo-linux-gnueabihf-gcc -O2 -Wall -shared -fPIC -o /out/vmshim.so /src/vmshim.c -ldl
        arm-kobo-linux-gnueabihf-gcc -O2 -Wall -o /out/vmtouch /src/vmtouch.c
    '
    log "building the Wi-Fi sidecar"
    docker build -q -t "$WIFI_IMAGE" "$VM_DIR/wifi" >/dev/null
    log "ready: vm/vm.sh $PROFILE up"
}

running() { [ "$(docker inspect -f '{{.State.Running}}' "$1" 2>/dev/null)" = true ]; }

touch_cmd() { docker exec "$VM" sh -c "echo '$*' > /tmp/touch"; }

wait_for() {
    local pattern=$1 seconds=$2
    for _ in $(seq "$seconds"); do
        if docker exec "$VM" sh -c 'logread 2>/dev/null' | grep -q "$pattern"; then
            return 0
        fi
        sleep 1
    done
    return 1
}

start_kosync() {
    docker network inspect "$NET" >/dev/null 2>&1 || docker network create "$NET" >/dev/null
    running "$KOSYNC" && return
    docker rm -f "$KOSYNC" >/dev/null 2>&1 || true
    docker run -d --name "$KOSYNC" --network "$NET" -p "127.0.0.1:$KOSYNC_PORT:$KOSYNC_PORT" \
        -v "$VM_DIR:/vm:ro" -v "$STATE:/state" python:3.12-slim \
        python -I /vm/kosync_mock.py "$KOSYNC_PORT" /state/kosync.json >/dev/null
}

start_nickel() {
    docker rm -f "$WIFI" "$VM" >/dev/null 2>&1 || true
    local first=0
    [ -f "$STATE/onboard/.kobo/KoboReader.sqlite" ] || first=1
    docker run -d --name "$VM" --privileged --platform linux/arm/v7 --network "$NET" \
        -e QT_LOGGING_RULES="${NICKEL_VM_QT_LOGGING:-}" -e VM_DBUS_MONITOR="${VM_DBUS_MONITOR:-}" \
        -v "$VM_DIR:/vm:ro" -v "$BUILD:/vmbin:ro" -v "$STATE:/state" "${SHARED[@]}" \
        "$IMAGE" /vm/init.sh >/dev/null
    docker run -d --name "$WIFI" --network "container:$VM" --cap-add NET_ADMIN "${SHARED[@]}" \
        -e VM_WIFI_TRACE="${VM_WIFI_TRACE:-}" "$WIFI_IMAGE" >/dev/null
    log "starting Nickel (about a minute under emulation)"
    if ! wait_for 'MainWindowController::pushView' 180; then
        log "Nickel did not show a view; see: vm/vm.sh syslog"
        exit 1
    fi
    sleep 5
    if [ "$first" = 1 ] && [ "$BRAND" = tolino ]; then
        log "first start: German, set up later"
        touch_cmd tap 150 627
        sleep 8
        touch_cmd tap 500 997
        sleep 10
    fi
    log "up; screen: vm/vm.sh $PROFILE shot"
}

up() {
    [ -f "$BUILD/vmshim.so" ] && docker image inspect "$IMAGE" >/dev/null 2>&1 ||
        { log "run vm/vm.sh $PROFILE image first"; exit 1; }
    mkdir -p "$STATE/onboard" "$STATE/imageformats"
    [ -f "$STATE/hwcfg.img" ] || python3 -I "$VM_DIR/hwcfg.py" "$STATE/hwcfg.img" "$BRAND"
    [ -f "$STATE/fb0.raw" ] || dd if=/dev/zero of="$STATE/fb0.raw" bs=4288 count=1448 2>/dev/null
    start_kosync
    start_nickel
}

down() {
    docker rm -f "$WIFI" "$VM" "$KOSYNC" >/dev/null 2>&1 || true
}

install() {
    local plugin=$ROOT/plugin/libnickelinksync.so
    local cli=$ROOT/target/armv7-unknown-linux-musleabihf/release/nickelinksync
    [ -f "$plugin" ] || { log "no $plugin: just plugin"; exit 1; }
    [ -f "$cli" ] || { log "no $cli: just build-device"; exit 1; }
    local dir=$STATE/onboard/.adds/NickelInkSync
    mkdir -p "$dir" "$STATE/imageformats"
    cp "$plugin" "$STATE/imageformats/"
    cp "$cli" "$dir/nickelinksync"
    chmod 755 "$dir/nickelinksync"
    if [ ! -f "$dir/config.ini" ]; then
        printf 'server = http://%s:%s\nuser = vm\npassword = vm\ndevice = Nickel VM (%s)\n' \
            "$KOSYNC" "$KOSYNC_PORT" "$PROFILE" > "$dir/config.ini"
    fi
    log "installed; restarting Nickel"
    start_nickel
}

usb() {
    # Nickel's hardware event FIFO: /run/kobo/udev/fifo in 6.0,
    # /tmp/nickel-hardware-status in 5.18.
    local fifo=/run/kobo/udev/fifo
    docker exec "$VM" test -p "$fifo" || fifo=/tmp/nickel-hardware-status
    docker exec "$VM" sh -c "echo 'usb plug add' > $fifo"
    sleep 10
    touch_cmd tap 540 1074 # Verbinden
    sleep 10
    docker exec "$VM" sh -c "echo 'usb plug remove' > $fifo"
    sleep 20
}

books() {
    [ $# -gt 0 ] || { log "books FILE..."; exit 2; }
    cp "$@" "$STATE/onboard/"
    usb
}

cmd=${1:-help}
shift || true
case "$cmd" in
    image) image ;;
    up) up ;;
    down) down ;;
    restart) start_nickel ;;
    install) install ;;
    books) books "$@" ;;
    usb) usb ;;
    tap) touch_cmd tap "$1" "$2" ;;
    swipe) touch_cmd swipe "$1" "$2" "$3" "$4" "${5:-300}" ;;
    shot)
        out=${1:-$STATE/screen.png}
        python3 -I "$VM_DIR/topng.py" "$STATE/fb0.raw" "$out"
        echo "$out"
        ;;
    log)
        cat "$STATE/onboard/.adds/NickelInkSync/NickelInkSync.log" 2>/dev/null || true
        docker exec "$VM" sh -c 'logread' | grep -F '(NickelInkSync)' || true
        ;;
    syslog) docker exec "$VM" sh -c 'logread' ;;
    wifi) docker logs "$WIFI" ;;
    kosync) cat "$STATE/kosync.json" 2>/dev/null || echo '{}' ;;
    shell) docker exec -it "$VM" sh ;;
    *) sed -n '2,/^set -euo/p' "$0" | grep '^#' | sed 's/^# \{0,1\}//' ;;
esac

#!/bin/sh
# Start script inside the Nickel VM container (runs under qemu-user as the
# firmware's own /bin/sh). It stands in for the hardware Nickel expects and
# then starts Nickel the way /etc/rc.local does. Mounts (vm/vm.sh):
#   /vm     this directory, read-only
#   /vmbin  vmshim.so and vmtouch, built for the device
#   /state  hwcfg.img, fb0.raw, onboard/ (the user partition), imageformats/

# Hardware configuration: brand, board, screen (vm/hwcfg.py).
mkdir -p /dev/disk/by-partlabel
ln -sf /state/hwcfg.img /dev/disk/by-partlabel/hwcfg

# The user partition.
mkdir -p /mnt/onboard
mount --bind /state/onboard /mnt/onboard

# The aux partition: Nickel's rendered pages (/mnt/aux/rendering), whose
# rows are in the database. Kept across restarts, as on the device.
mkdir -p /state/aux /mnt/aux
mount --bind /state/aux /mnt/aux

# A battery at 80 %, not charging. 6.0 reads `battery`; 5.18 also reads
# the PMIC's own `bd71827_bat`.
mount -t tmpfs none /sys/class/power_supply
for bat in battery bd71827_bat; do
    mkdir -p /sys/class/power_supply/$bat
    cd /sys/class/power_supply/$bat || exit 1
    echo 80 > capacity
    echo 'Not charging' > status
    echo 0 > charging
    echo 0 > charger_type
    echo 0 > time_to_full_now
done
cd /

# Nothing may power off the container or hand the user partition to USB.
# Wi-Fi is vm/wifi/wifi.py in a sidecar: the real wpa_supplicant, dhcpcd
# and dhcpcd-dbus would only fail without the hardware.
for f in /sbin/poweroff /sbin/reboot /sbin/halt /usr/bin/export-usb.sh \
        /usr/sbin/wpa_supplicant /sbin/dhcpcd /usr/libexec/dhcpcd-dbus; do
    mount --bind /vm/stub.sh "$f"
done

# NickelHook plugins. Copied, not mounted: NickelHook renames its library
# into /usr/local/Kobo while it starts, which fails across file systems.
mkdir -p /usr/local/Kobo/imageformats
cp /state/imageformats/*.so /usr/local/Kobo/imageformats/ 2>/dev/null

# 5.18 takes USB and other hardware events from this FIFO, which the
# device's init script creates in /tmp (6.0 makes its own in /run/kobo).
[ -p /tmp/nickel-hardware-status ] || mkfifo /tmp/nickel-hardware-status

# Touchscreen: a FIFO fed by vmtouch, which reads commands from /tmp/touch.
mkdir -p /dev/input
/vmbin/vmtouch /tmp/touch &
while [ ! -p /dev/input/touchscreen ]; do sleep 0.2; done

# Screen: /dev/fb0 is a plain file; vmshim.so answers the framebuffer ioctls.
ln -sf /state/fb0.raw /dev/fb0

export CPU_CORES=2
. /usr/libexec/platform/nickel-env.sh
export QT_USE_SYSTEM_LINE_METRICS=1
echo "PLATFORM=$PLATFORM PRODUCT=$PRODUCT"

mkdir -p /run/dbus
rm -f /run/dbus/pid /run/dbus/system_bus_socket /run/dbus/vm-wifi-ready
dbus-daemon --system --fork
if [ -n "$VM_DBUS_MONITOR" ]; then
    while [ ! -S /run/dbus/system_bus_socket ]; do sleep 0.2; done
    dbus-monitor --system > /state/dbus-monitor.txt 2>&1 &
fi
# Nickel asks name.marples.roy.dhcpcd at once; wait for the sidecar to own it.
i=0
while [ ! -e /run/dbus/vm-wifi-ready ] && [ $i -lt 120 ]; do sleep 0.5; i=$((i + 1)); done
[ -e /run/dbus/vm-wifi-ready ] || echo "vm: no Wi-Fi sidecar, starting without Wi-Fi"
DBUS_SESSION_BUS_ADDRESS=$(dbus-daemon --session --print-address --fork)
export DBUS_SESSION_BUS_ADDRESS

export LD_PRELOAD=/vmbin/vmshim.so VMSHIM_FB_FILE=/state/fb0.raw
exec /usr/local/Kobo/nickel -platform kobo 2>&1

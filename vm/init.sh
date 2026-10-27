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

# A battery at 80 %, not charging.
mount -t tmpfs none /sys/class/power_supply
mkdir -p /sys/class/power_supply/battery
cd /sys/class/power_supply/battery || exit 1
echo 80 > capacity
echo 'Not charging' > status
echo 0 > charging
echo 0 > charger_type
echo 0 > time_to_full_now
cd /

# Nothing may power off the container or hand the user partition to USB.
for f in /sbin/poweroff /sbin/reboot /sbin/halt /usr/bin/export-usb.sh; do
    mount --bind /vm/stub.sh "$f"
done

# NickelHook plugins. Copied, not mounted: NickelHook renames its library
# into /usr/local/Kobo while it starts, which fails across file systems.
mkdir -p /usr/local/Kobo/imageformats
cp /state/imageformats/*.so /usr/local/Kobo/imageformats/ 2>/dev/null

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
dbus-daemon --system --fork
DBUS_SESSION_BUS_ADDRESS=$(dbus-daemon --session --print-address --fork)
export DBUS_SESSION_BUS_ADDRESS

export LD_PRELOAD=/vmbin/vmshim.so VMSHIM_FB_FILE=/state/fb0.raw
exec /usr/local/Kobo/nickel -platform kobo 2>&1

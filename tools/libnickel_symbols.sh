#!/bin/sh
# Lists the `_Z` dynamic symbols of libnickel from a Kobo or Tolino firmware
# update, one "D name" (defined) or "U name" (imported) per line, gzipped:
# the reference tools/check_plugin_symbols.py checks the plugin against.
# The firmware comes from Kobo's server, so neither it nor the list is in
# this repository; CI makes the list and caches it.
#
# Usage: libnickel_symbols.sh <qt6-update-….zip | update.tar> <out.syms.gz>
# Needs unzip, zstd, debugfs (e2fsprogs) and readelf (binutils).
set -eu
fw=$1
out=$2
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
case $fw in
    # 6.0 comes as a zip around update.tar, 5.18 as update.tar itself.
    *.zip)
        unzip -q -o "$fw" update.tar -d "$tmp"
        tar=$tmp/update.tar
        ;;
    *) tar=$fw ;;
esac
# rootfs.img is a zstd-compressed ext4 image; debugfs reads it without a
# loop mount.
tar -xOf "$tar" rootfs.img | zstd -dq >"$tmp/rootfs.ext4"
debugfs -R "dump /usr/local/Kobo/libnickel.so.1.0.0 $tmp/libnickel.so" "$tmp/rootfs.ext4" 2>/dev/null
test -s "$tmp/libnickel.so"
readelf --dyn-syms -W "$tmp/libnickel.so" |
    awk '$8 ~ /^_Z/ { name = $8; sub(/@.*/, "", name); print ($7 == "UND" ? "U " : "D ") name }' |
    sort -u | gzip -9n >"$out"
echo "$(gzip -dc "$out" | wc -l | tr -d ' ') symbols in $out"

#!/bin/sh
# Qt 5 build of the plugin for Kobo firmware 4.x (Qt 5.2.1) in NickelTC
# (strayrose/nickeltc, arm-nickel-linux-gnueabihf, GCC 4.9). The same source
# as the Qt 6 build, built on a copy in target/plugin-qt5 so the two builds
# never mix their objects; the package lands in plugin/KoboRoot.tgz (4.x
# layout: ./usr/local/Kobo/imageformats/libnickelinksync.so) and the
# dynamic symbols in target/plugin-qt5/nm.txt. Needs Docker; amd64 image.
set -eu
cd "$(dirname "$0")/.."
image=strayrose/nickeltc:latest
out=target/plugin-qt5
rm -rf "$out"
mkdir -p "$out"
cp -R plugin/Makefile plugin/src plugin/NickelHook "$out/"
# NickelHook asks git for the version; git in the container does not trust
# the mounted checkout, so it comes from here.
version=$(git describe --tags --always --dirty 2>/dev/null || echo unknown)
docker run --rm --platform linux/amd64 --volume="$PWD:$PWD" --workdir="$PWD" "$image" bash -c "
  set -e
  make -C $out clean >/dev/null
  make -C $out NH_QT_MAJOR=5 VERSION=$version
  make -C $out NH_QT_MAJOR=5 VERSION=$version koboroot
  arm-nickel-linux-gnueabihf-nm -D $out/libnickelinksync.so > $out/nm.txt
  arm-nickel-linux-gnueabihf-objdump -p -T $out/libnickelinksync.so > $out/objdump.txt
"
cp "$out/KoboRoot.tgz" plugin/KoboRoot.tgz

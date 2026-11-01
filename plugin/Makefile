# NickelHook plugin for Kobo firmware 6.0 and Tolino 5.18+ (Qt6). Build in
# the Qt 6.5.2 toolchain image (arm-kobo-linux-gnueabihf), see README.md:
#   make -C plugin && make -C plugin kobo   ->  plugin/Kobo.tgz
NH_QT_MAJOR ?= 6

include ./NickelHook/NickelHook.mk

override LIBRARY  := libnickelinksync.so
override SOURCES  += src/nickelinksync.cc
# QSqlDatabase: Nickel's own connection, for its cached pages (Kobo).
override PKGCONF  += Qt$(NH_QT_MAJOR)Sql
override CFLAGS   += -Wall -Wextra -Werror -Wno-missing-field-initializers -fvisibility=hidden
override CXXFLAGS += -Wall -Wextra -Werror -Wno-missing-field-initializers -fvisibility=hidden -fvisibility-inlines-hidden

include ./NickelHook/NickelHook.mk

#!/usr/bin/env python3
"""Write the NTX hardware configuration of a Tolino Shine Color (or the Kobo
Clara Colour, the same board) as a partition image for the Nickel VM.

Nickel, `hwdetect` and the platform scripts read it from
/dev/disk/by-partlabel/hwcfg: the configuration block starts at sector 1,
the serial number at sector 4. Each field is one byte, an index into the
value list of `ntx_hwconfig -l` (the firmware's own tool, which wrote these
values in the first place).

    hwcfg.py OUTPUT [tolino|kobo]
"""

import sys

FIELDS = {
    0: 113,   # PCB E60T00 (spa)
    1: 12,    # KeyPad NO_Key
    4: 17,    # Wifi MT8113T
    5: 11,    # BT MT8113T
    7: 18,    # TouchCtrl GTXX_NO_PEN
    8: 3,     # TouchType C-Type
    9: 30,    # DisplayCtrl MT8113T+SY7636
    10: 6,    # DisplayPanel 6" Bottom EPD
    14: 1,    # Battery 1500mA
    16: 4,    # RamSize 1024MB
    17: 2,    # IFlash eMMC
    19: 2,    # RootFsType Ext4
    27: 16,   # CPU mt8113t
    29: 12,   # RAMType LPDDR4
    31: 5,    # DisplayResolution 1448x1072
    32: 1,    # FrontLight TABLE0
    38: 10,   # FrontLight_LEDrv AW99703
    45: 11,   # FL_PWM AW99703
    63: 0x08, # PCB_Flags2 USB_TYPEC
    64: 0x08, # EPD_Flags CFA (colour screen)
}
CUSTOMER = {'tolino': 0x21, 'kobo': 0x09}  # field 13, as in switchbrand
SIZE = 77

out = sys.argv[1]
brand = sys.argv[2] if len(sys.argv) > 2 else 'tolino'
values = bytearray(SIZE)
for field, value in FIELDS.items():
    values[field] = value
values[13] = CUSTOMER[brand]

image = bytearray(1 << 20)
block = b'HW CONFIG ' + b'v3.8\0' + bytes([SIZE]) + bytes(values)
image[512:512 + len(block)] = block
serial = b'VM0000000001\n'
image[4 * 512:4 * 512 + len(serial)] = serial
with open(out, 'wb') as f:
    f.write(image)

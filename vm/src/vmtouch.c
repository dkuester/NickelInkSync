// A virtual touchscreen for Nickel in the VM. qemu-user translates neither
// uinput nor evdev ioctls, so the "device" is a FIFO: this program writes
// input events into it and vmshim.so answers Qt's evdev ioctls on it.
//
// Usage: vmtouch COMMAND_FIFO
// Commands, one per line, in screen coordinates (portrait, pixels):
//   tap X Y
//   hold X Y MS
//   swipe X1 Y1 X2 Y2 MS
#define _GNU_SOURCE
#include <fcntl.h>
#include <linux/input.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>

#include "vmtouch.h"

static int out;
static int tracking_id;

static void emit(int type, int code, int value)
{
    struct input_event ev;
    memset(&ev, 0, sizeof(ev));
    gettimeofday(&ev.time, NULL);
    ev.type = type;
    ev.code = code;
    ev.value = value;
    if (write(out, &ev, sizeof(ev)) != sizeof(ev))
        perror("vmtouch: write");
}

static void sleep_ms(int ms)
{
    struct timespec ts = {ms / 1000, (ms % 1000) * 1000000L};
    nanosleep(&ts, NULL);
}

// The panel reports landscape coordinates and Nickel rotates them by 270
// degrees (QT_QPA_EVDEV_TOUCHSCREEN_PARAMETERS ...:rotate=270), which maps
// normalised device (nx, ny) to screen (1 - ny, nx). This is the inverse.
static void to_device(int sx, int sy, int *dx, int *dy)
{
    double nx = (double)sy / (VMTOUCH_SCREEN_H - 1);
    double ny = 1.0 - (double)sx / (VMTOUCH_SCREEN_W - 1);
    *dx = (int)(nx * VMTOUCH_MAX_X + 0.5);
    *dy = (int)(ny * VMTOUCH_MAX_Y + 0.5);
}

static void contact(int sx, int sy, int first)
{
    int dx, dy;
    to_device(sx, sy, &dx, &dy);
    emit(EV_ABS, ABS_MT_SLOT, 0);
    if (first)
        emit(EV_ABS, ABS_MT_TRACKING_ID, ++tracking_id);
    emit(EV_ABS, ABS_MT_POSITION_X, dx);
    emit(EV_ABS, ABS_MT_POSITION_Y, dy);
    emit(EV_ABS, ABS_MT_PRESSURE, 50);
    if (first)
        emit(EV_KEY, BTN_TOUCH, 1);
    emit(EV_ABS, ABS_X, dx);
    emit(EV_ABS, ABS_Y, dy);
    emit(EV_SYN, SYN_REPORT, 0);
}

static void release(void)
{
    emit(EV_ABS, ABS_MT_SLOT, 0);
    emit(EV_ABS, ABS_MT_TRACKING_ID, -1);
    emit(EV_KEY, BTN_TOUCH, 0);
    emit(EV_SYN, SYN_REPORT, 0);
}

static void stroke(int x1, int y1, int x2, int y2, int ms)
{
    int steps = ms / 20;
    if (steps < 1)
        steps = 1;
    contact(x1, y1, 1);
    for (int i = 1; i <= steps; i++) {
        sleep_ms(ms / steps);
        contact(x1 + (x2 - x1) * i / steps, y1 + (y2 - y1) * i / steps, 0);
    }
    release();
}

int main(int argc, char **argv)
{
    if (argc < 2) {
        fprintf(stderr, "usage: vmtouch COMMAND_FIFO\n");
        return 2;
    }
    mkfifo(VMTOUCH_DEVICE, 0644);
    mkfifo(argv[1], 0666);
    // O_RDWR keeps the device FIFO open for Nickel without waiting for it.
    out = open(VMTOUCH_DEVICE, O_RDWR);
    if (out < 0) {
        perror("vmtouch: " VMTOUCH_DEVICE);
        return 1;
    }
    for (;;) {
        FILE *f = fopen(argv[1], "r");
        if (!f) {
            perror("vmtouch: command fifo");
            return 1;
        }
        char line[256];
        while (fgets(line, sizeof(line), f)) {
            int a, b, c, d, e;
            if (sscanf(line, "tap %d %d", &a, &b) == 2)
                stroke(a, b, a, b, 80);
            else if (sscanf(line, "hold %d %d %d", &a, &b, &c) == 3)
                stroke(a, b, a, b, c);
            else if (sscanf(line, "swipe %d %d %d %d %d", &a, &b, &c, &d, &e) == 5)
                stroke(a, b, c, d, e);
            else
                fprintf(stderr, "vmtouch: unknown command: %s", line);
        }
        fclose(f);
    }
}

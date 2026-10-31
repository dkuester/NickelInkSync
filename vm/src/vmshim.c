// LD_PRELOAD shim for Nickel under qemu-user. It answers the ioctls that
// have no hardware behind them in the VM:
//  - /dev/fb0 is a plain file (the start script links it); the shim reports
//    its geometry and accepts the e-ink update ioctls.
//  - /dev/input/touchscreen is a FIFO fed by vmtouch; the shim answers the
//    evdev queries Qt's evdevtouch makes, which qemu-user does not translate.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <linux/fb.h>
#include <linux/input.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

#include "vmtouch.h"

#define FB_BPP 32

enum { OTHER, FB, TOUCH };

static int classify(int fd)
{
    char link[64], target[256];
    snprintf(link, sizeof(link), "/proc/self/fd/%d", fd);
    ssize_t n = readlink(link, target, sizeof(target) - 1);
    if (n < 0)
        return OTHER;
    target[n] = 0;
    const char *fb = getenv("VMSHIM_FB_FILE");
    if (strcmp(target, "/dev/fb0") == 0 || (fb && strcmp(target, fb) == 0))
        return FB;
    if (strcmp(target, VMTOUCH_DEVICE) == 0)
        return TOUCH;
    return OTHER;
}

static int fb_ioctl(unsigned long req, void *arg)
{
    if (req == FBIOGET_VSCREENINFO) {
        struct fb_var_screeninfo *v = arg;
        memset(v, 0, sizeof(*v));
        v->xres = v->xres_virtual = VMTOUCH_SCREEN_W;
        v->yres = v->yres_virtual = VMTOUCH_SCREEN_H;
        v->bits_per_pixel = FB_BPP;
        v->red = (struct fb_bitfield){16, 8, 0};
        v->green = (struct fb_bitfield){8, 8, 0};
        v->blue = (struct fb_bitfield){0, 8, 0};
        v->transp = (struct fb_bitfield){24, 8, 0};
        v->width = 91; // mm
        v->height = 122;
    } else if (req == FBIOGET_FSCREENINFO) {
        struct fb_fix_screeninfo *f = arg;
        memset(f, 0, sizeof(*f));
        strcpy(f->id, "hwtcon");
        f->line_length = VMTOUCH_SCREEN_W * FB_BPP / 8;
        f->smem_len = f->line_length * VMTOUCH_SCREEN_H;
        f->type = FB_TYPE_PACKED_PIXELS;
        f->visual = FB_VISUAL_TRUECOLOR;
    }
    // Everything else is an e-ink update or waveform request. The file
    // already holds the pixels, so there is nothing to refresh.
    return 0;
}

static void set_bit(void *buf, int len, int bit)
{
    if (bit / 8 < len)
        ((unsigned char *)buf)[bit / 8] |= 1 << (bit % 8);
}

static int touch_ioctl(unsigned long req, void *arg)
{
    int nr = _IOC_NR(req), len = _IOC_SIZE(req);
    if (_IOC_TYPE(req) != 'E')
        return 0;
    if (nr == _IOC_NR(EVIOCGVERSION)) {
        *(int *)arg = EV_VERSION;
        return 0;
    }
    if (nr == _IOC_NR(EVIOCGRAB) || nr == _IOC_NR(EVIOCGID)) {
        if (nr == _IOC_NR(EVIOCGID))
            memset(arg, 0, sizeof(struct input_id));
        return 0;
    }
    if (nr == _IOC_NR(EVIOCGNAME(0))) {
        snprintf(arg, len, "vmtouch");
        return strlen(arg) + 1;
    }
    if (nr >= _IOC_NR(EVIOCGABS(0)) && nr < _IOC_NR(EVIOCGABS(ABS_CNT))) {
        int code = nr - _IOC_NR(EVIOCGABS(0));
        struct input_absinfo *a = arg;
        memset(a, 0, sizeof(*a));
        switch (code) {
        case ABS_X:
        case ABS_MT_POSITION_X: a->maximum = VMTOUCH_MAX_X; break;
        case ABS_Y:
        case ABS_MT_POSITION_Y: a->maximum = VMTOUCH_MAX_Y; break;
        case ABS_MT_PRESSURE: a->maximum = 255; break;
        case ABS_MT_SLOT: a->maximum = 4; break;
        case ABS_MT_TRACKING_ID: a->maximum = 65535; break;
        }
        return 0;
    }
    memset(arg, 0, len);
    if (nr == _IOC_NR(EVIOCGPROP(0))) {
        set_bit(arg, len, INPUT_PROP_DIRECT);
    } else if (nr >= _IOC_NR(EVIOCGBIT(0, 0)) && nr < _IOC_NR(EVIOCGBIT(EV_CNT, 0))) {
        switch (nr - _IOC_NR(EVIOCGBIT(0, 0))) {
        case 0:
            set_bit(arg, len, EV_SYN);
            set_bit(arg, len, EV_KEY);
            set_bit(arg, len, EV_ABS);
            break;
        case EV_KEY:
            set_bit(arg, len, BTN_TOUCH);
            break;
        case EV_ABS: {
            int codes[] = {ABS_X, ABS_Y, ABS_MT_SLOT, ABS_MT_TRACKING_ID,
                           ABS_MT_POSITION_X, ABS_MT_POSITION_Y, ABS_MT_PRESSURE};
            for (size_t i = 0; i < sizeof(codes) / sizeof(codes[0]); i++)
                set_bit(arg, len, codes[i]);
            break;
        }
        }
    }
    // Bit and state queries return the number of bytes copied.
    return len;
}

int ioctl(int fd, unsigned long req, ...)
{
    static int (*real)(int, unsigned long, ...);
    if (!real)
        real = dlsym(RTLD_NEXT, "ioctl");
    va_list ap;
    va_start(ap, req);
    void *arg = va_arg(ap, void *);
    va_end(ap);
    switch (classify(fd)) {
    case FB:
        return fb_ioctl(req, arg);
    case TOUCH:
        return touch_ioctl(req, arg);
    default:
        return real(fd, req, arg);
    }
}

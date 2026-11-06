/* Passive libei receiver for private capture-worker acceptance. */
#define _GNU_SOURCE
#include <fcntl.h>
#include <libei.h>
#include <poll.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static bool held[65536];
static bool held_keys[768];
static uint64_t keymap_hash(struct ei_keymap *map) {
    if (!map || ei_keymap_get_type(map) != EI_KEYMAP_TYPE_XKB) exit(16);
    size_t size = ei_keymap_get_size(map);
    if (!size || size > 256 * 1024 + 1) exit(17);
    char *text = malloc(size);
    if (!text || pread(ei_keymap_get_fd(map), text, size, 0) != (ssize_t)size
        || text[size - 1] || !memmem(text, size, "xkb_keymap", 10)) exit(18);
    int seals = fcntl(ei_keymap_get_fd(map), F_GET_SEALS);
    if (seals < 0 || (seals & (F_SEAL_WRITE | F_SEAL_GROW | F_SEAL_SHRINK))
        != (F_SEAL_WRITE | F_SEAL_GROW | F_SEAL_SHRINK)) exit(19);
    uint64_t hash = UINT64_C(14695981039346656037);
    for (size_t i = 0; i < size; i++) hash = (hash ^ (unsigned char)text[i]) * UINT64_C(1099511628211);
    free(text);
    return hash;
}
static void bind(struct ei_seat *seat, unsigned grants, bool request) {
    if (grants == 1) {
        if (request) ei_seat_request_device_with_capabilities(seat, EI_DEVICE_CAP_KEYBOARD, NULL);
        else ei_seat_bind_capabilities(seat, EI_DEVICE_CAP_KEYBOARD, NULL);
    } else if (grants == 3) {
        if (request) ei_seat_request_device_with_capabilities(seat, EI_DEVICE_CAP_POINTER, EI_DEVICE_CAP_BUTTON, EI_DEVICE_CAP_SCROLL, EI_DEVICE_CAP_KEYBOARD, NULL);
        else ei_seat_bind_capabilities(seat, EI_DEVICE_CAP_POINTER, EI_DEVICE_CAP_BUTTON, EI_DEVICE_CAP_SCROLL, EI_DEVICE_CAP_KEYBOARD, NULL);
    } else {
        if (request) ei_seat_request_device_with_capabilities(seat, EI_DEVICE_CAP_POINTER, EI_DEVICE_CAP_BUTTON, EI_DEVICE_CAP_SCROLL, NULL);
        else ei_seat_bind_capabilities(seat, EI_DEVICE_CAP_POINTER, EI_DEVICE_CAP_BUTTON, EI_DEVICE_CAP_SCROLL, NULL);
    }
}
int main(int argc, char **argv) {
    if (argc != 3 && argc != 4) return 2;
    unsigned grants = argc == 4 ? (unsigned)atoi(argv[3]) : 2;
    if (grants < 1 || grants > 3) return 2;
    int control = atoi(argv[1]);
    bool sender = !strcmp(argv[2], "sender");
    bool abrupt = !strcmp(argv[2], "abrupt");
    bool paused = false;
    setvbuf(stdout, NULL, _IOLBF, 0);
    struct ei *context = sender ? ei_new_sender(NULL) : ei_new_receiver(NULL);
    ei_configure_name(context, "Canopa private capture receiver");
    if (ei_setup_backend_fd(context, STDIN_FILENO)) return 3;
    struct ei_device *device = NULL;
    struct ei_seat *seat = NULL;
    struct ei_keymap *map = NULL;
    uint64_t map_hash = 0;
    uint32_t depressed = 0, latched = 0, locked = 0, group = 0;
    bool active = false;
    struct pollfd descriptors[2] = {{.fd=ei_get_fd(context),.events=POLLIN}, {.fd=control,.events=POLLIN}};
    char command[64]; size_t filled=0;
    for (;;) {
        if (!paused) ei_dispatch(context);
        struct ei_event *event;
        while (!paused && (event=ei_get_event(context))) {
            enum ei_event_type type=ei_event_get_type(event);
            struct ei_device *current=ei_event_get_device(event);
            if (type==EI_EVENT_SEAT_ADDED) {
                if (sender) return 4;
                seat=ei_seat_ref(ei_event_get_seat(event));
                if (ei_seat_has_capability(seat,EI_DEVICE_CAP_KEYBOARD) != !!(grants & 1)
                    || ei_seat_has_capability(seat,EI_DEVICE_CAP_POINTER) != !!(grants & 2)
                    || ei_seat_has_capability(seat,EI_DEVICE_CAP_BUTTON) != !!(grants & 2)
                    || ei_seat_has_capability(seat,EI_DEVICE_CAP_SCROLL) != !!(grants & 2)
                    || ei_seat_has_capability(seat,EI_DEVICE_CAP_TOUCH)
                    || ei_seat_has_capability(seat,EI_DEVICE_CAP_TEXT) || ei_seat_has_capability(seat,EI_DEVICE_CAP_POINTER_ABSOLUTE)) return 5;
                bind(seat, grants, false);
                puts("{\"seat\":true}");
            }
            if (type==EI_EVENT_DEVICE_ADDED) {
                if (device || ei_device_has_capability(current,EI_DEVICE_CAP_POINTER) != !!(grants & 2)
                    || ei_device_has_capability(current,EI_DEVICE_CAP_BUTTON) != !!(grants & 2)
                    || ei_device_has_capability(current,EI_DEVICE_CAP_SCROLL) != !!(grants & 2)
                    || ei_device_has_capability(current,EI_DEVICE_CAP_KEYBOARD) != !!(grants & 1) || ei_device_has_capability(current,EI_DEVICE_CAP_TOUCH)
                    || ei_device_has_capability(current,EI_DEVICE_CAP_POINTER_ABSOLUTE)) return 6;
                if (grants & 1) {
                    map = ei_keymap_ref(ei_device_keyboard_get_keymap(current));
                    map_hash = keymap_hash(map);
                    printf("{\"keymap\":{\"size\":%zu,\"fnv\":\"%016llx\"}}\n", ei_keymap_get_size(map), (unsigned long long)map_hash);
                }
                device=ei_device_ref(current); puts("{\"added\":true}");
            }
            if (type==EI_EVENT_DEVICE_RESUMED) puts("{\"resumed\":true}");
            if (type==EI_EVENT_DEVICE_START_EMULATING) {
                if (active) return 7;
                active=true;printf("{\"start\":%u}\n",ei_event_emulating_get_sequence(event));
            }
            if (type==EI_EVENT_KEYBOARD_KEY) {
                unsigned key = ei_event_keyboard_get_key(event);
                bool press = ei_event_keyboard_get_key_is_press(event);
                if (!active || !(grants & 1) || !key || key >= 768 || held_keys[key] == press) return 20;
                held_keys[key] = press;
                printf("{\"key\":[%u,%u]}\n", key, press);
            }
            if (type==EI_EVENT_KEYBOARD_MODIFIERS) {
                if (!(grants & 1)) return 21;
                depressed = ei_event_keyboard_get_xkb_mods_depressed(event);
                latched = ei_event_keyboard_get_xkb_mods_latched(event);
                locked = ei_event_keyboard_get_xkb_mods_locked(event);
                group = ei_event_keyboard_get_xkb_group(event);
                printf("{\"modifiers\":[%u,%u,%u,%u]}\n", depressed, latched, locked, group);
            }
            if (type==EI_EVENT_POINTER_MOTION) {
                if (!active) return 8;
                printf("{\"motion\":[%.8g,%.8g]}\n",ei_event_pointer_get_dx(event),ei_event_pointer_get_dy(event));
            }
            if (type==EI_EVENT_BUTTON_BUTTON) {
                unsigned button=ei_event_button_get_button(event);bool press=ei_event_button_get_is_press(event);
                if (!active || button>=65536 || held[button]==press) return 9;
                held[button]=press;printf("{\"button\":[%u,%u]}\n",button,press);
            }
            if (type==EI_EVENT_SCROLL_DELTA) {
                if (!active) return 10;
                printf("{\"scroll\":[%.8g,%.8g]}\n",ei_event_scroll_get_dx(event),ei_event_scroll_get_dy(event));
            }
            if (type==EI_EVENT_SCROLL_DISCRETE) {
                if (!active) return 11;
                printf("{\"wheel\":[%d,%d]}\n",ei_event_scroll_get_discrete_dx(event),ei_event_scroll_get_discrete_dy(event));
            }
            if (type==EI_EVENT_SCROLL_STOP || type==EI_EVENT_SCROLL_CANCEL)
                printf("{\"scroll_end\":[%u,%u],\"cancel\":%u}\n",ei_event_scroll_get_stop_x(event),ei_event_scroll_get_stop_y(event),type==EI_EVENT_SCROLL_CANCEL);
            if (type==EI_EVENT_FRAME) printf("{\"frame\":%llu}\n",(unsigned long long)ei_event_get_time(event));
            if (type==EI_EVENT_DEVICE_STOP_EMULATING) {
                for (unsigned i=0;i<65536;i++) if (held[i] && !abrupt) return 12;
                for (unsigned i=0;i<768;i++) if (held_keys[i] && !abrupt) return 22;
                if (!abrupt && (depressed || latched || locked || group)) return 23;
                active=false;puts("{\"stop\":true}");
            }
            if (type==EI_EVENT_DEVICE_REMOVED) {
                unsigned retired_held=0;
                for (unsigned i=0;i<65536;i++) if (held[i]) retired_held++;
                for (unsigned i=0;i<768;i++) if (held_keys[i]) retired_held++;
                printf("{\"removed\":true,\"active\":%u,\"held\":%u}\n",active,retired_held);
                /* libei drops further input after the client closes a device.
                 * Removal retires that logical device, including its state. */
                memset(held,0,sizeof(held));active=false;
                memset(held_keys,0,sizeof(held_keys)); depressed=latched=locked=group=0;
                if (map) {
                    if (keymap_hash(map) != map_hash) return 24;
                    ei_keymap_unref(map); map=NULL;
                }
                if (current==device) {ei_device_unref(device);device=NULL;}
            }
            if (type==EI_EVENT_DISCONNECT) {puts("{\"disconnected\":true}");ei_event_unref(event);goto done;}
            ei_event_unref(event);
        }
        descriptors[0].fd = paused ? -1 : ei_get_fd(context);
        if (poll(descriptors,2,10000)<0) return 13;
        if (descriptors[1].revents&(POLLIN|POLLHUP)) {
            ssize_t n=read(control,command+filled,sizeof(command)-1-filled);
            if (n<=0) break;
            filled+=(size_t)n;command[filled]=0;
            char *newline;
            while ((newline=memchr(command,'\n',filled))) {
                *newline=0;
                if (!strcmp(command,"close")) {if(device)ei_device_close(device);}
                else if (!strcmp(command,"pause")) {paused=true;puts("{\"paused\":true}");}
                else if (!strcmp(command,"resume")) {paused=false;puts("{\"resumed_reading\":true}");}
                else if (!strcmp(command,"request")) {
                    if(seat)bind(seat,grants,true);
                } else if (!strcmp(command,"quit")) goto done;
                else return 14;
                size_t used=(size_t)(newline-command)+1;memmove(command,command+used,filled-used);filled-=used;
            }
            if (filled==sizeof(command)-1) return 15;
        }
    }
done:
    if(map)ei_keymap_unref(map);
    if(device)ei_device_unref(device);
    if(seat)ei_seat_unref(seat);
    ei_unref(context);return 0;
}

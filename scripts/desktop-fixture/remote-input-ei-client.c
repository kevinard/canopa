/* Real libei sender on an anonymous fd, used only by the private desktop fixture. */
#define _GNU_SOURCE
#include <fcntl.h>
#include <libei.h>
#include <poll.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>
#include <xkbcommon/xkbcommon.h>

static uint64_t now(void) {
    struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t);
    return (uint64_t)t.tv_sec * 1000000 + t.tv_nsec / 1000;
}
int main(int argc, char **argv) {
    const char *mode = argc > 1 ? argv[1] : "held";
    int receiver = !strcmp(mode, "receiver"), flood = !strcmp(mode, "flood");
    struct ei *context = receiver ? ei_new_receiver(NULL) : ei_new_sender(NULL);
    ei_configure_name(context, "Canopa private keyboard client");
    if (ei_setup_backend_fd(context, STDIN_FILENO)) return 2;
    struct pollfd fd = { .fd = ei_get_fd(context), .events = POLLIN };
    int sent = 0, removed = 0, keymap_seen = 0, modifier_seen = 0;
    int repeat = !strcmp(mode, "repeat"), request = !strcmp(mode, "request"), stage = 0, devices = 0;
    for (int rounds = 0; rounds < 150; rounds++) {
        ei_dispatch(context);
        struct ei_event *event;
        while ((event = ei_get_event(context))) {
            enum ei_event_type type = ei_event_get_type(event);
            if (type == EI_EVENT_SEAT_ADDED) {
                if (receiver) return 3;
                ei_seat_bind_capabilities(ei_event_get_seat(event), EI_DEVICE_CAP_KEYBOARD, NULL);
            }
            if (type == EI_EVENT_DEVICE_ADDED) {
                struct ei_device *device = ei_event_get_device(event);
                if (!ei_device_has_capability(device, EI_DEVICE_CAP_KEYBOARD) ||
                    ei_device_has_capability(device, EI_DEVICE_CAP_POINTER) ||
                    ei_device_has_capability(device, EI_DEVICE_CAP_POINTER_ABSOLUTE) ||
                    ei_device_has_capability(device, EI_DEVICE_CAP_BUTTON) ||
                    ei_device_has_capability(device, EI_DEVICE_CAP_SCROLL) ||
                    ei_device_has_capability(device, EI_DEVICE_CAP_TOUCH) ||
                    ei_device_has_capability(device, EI_DEVICE_CAP_TEXT)) return 4;
                struct ei_keymap *map = ei_device_keyboard_get_keymap(device);
                if (!map || ei_keymap_get_type(map) != EI_KEYMAP_TYPE_XKB) return 5;
                size_t size = ei_keymap_get_size(map);
                int map_fd = ei_keymap_get_fd(map);
                if (!size || size > 1024 * 1024 || !(fcntl(map_fd, F_GET_SEALS) & F_SEAL_WRITE)) return 6;
                char *data = mmap(NULL, size, PROT_READ, MAP_PRIVATE, map_fd, 0);
                if (data == MAP_FAILED) return 7;
                struct xkb_context *xkb = xkb_context_new(XKB_CONTEXT_NO_FLAGS);
                struct xkb_keymap *parsed = xkb_keymap_new_from_buffer(xkb, data, size, XKB_KEYMAP_FORMAT_TEXT_V1, XKB_KEYMAP_COMPILE_NO_FLAGS);
                if (!parsed) return 8;
                xkb_keymap_unref(parsed); xkb_context_unref(xkb); munmap(data, size);
                keymap_seen = 1;
                devices++;
                if (request && devices == 1) ei_device_close(device);
                puts("keyboard-only sealed keymap"); fflush(stdout);
            }
            if (type == EI_EVENT_DEVICE_RESUMED && !sent && (!request || devices == 2)) {
                struct ei_device *device = ei_event_get_device(event);
                ei_device_start_emulating(device, 1);
                if (flood) {
                    for (int i = 0; i < 514; i++) ei_device_keyboard_key(device, 16, i % 2 == 0);
                } else {
                    ei_device_keyboard_key(device, 42, true);
                    ei_device_keyboard_key(device, 16, true);
                    ei_device_frame(device, now());
                    ei_device_keyboard_key(device, 16, false);
                }
                ei_device_frame(device, now());
                sent = 1;
                puts(flood ? "flood sent" : "held sent"); fflush(stdout);
            }
            if (type == EI_EVENT_KEYBOARD_MODIFIERS && ei_event_keyboard_get_xkb_mods_depressed(event)) {
                modifier_seen = 1;
                puts("modifier update"); fflush(stdout);
            }
            if (type == EI_EVENT_KEYBOARD_MODIFIERS && repeat) {
                struct ei_device *device = ei_event_get_device(event);
                if (ei_event_keyboard_get_xkb_mods_depressed(event) && stage == 0) {
                    ei_device_stop_emulating(device); stage = 1;
                } else if (!ei_event_keyboard_get_xkb_mods_depressed(event) && stage == 1) {
                    ei_device_start_emulating(device, 2);
                    ei_device_keyboard_key(device, 16, true);
                    ei_device_frame(device, now());
                    ei_device_keyboard_key(device, 16, false);
                    ei_device_frame(device, now()); stage = 2;
                    puts("period restarted"); fflush(stdout);
                }
            }
            if (type == EI_EVENT_DEVICE_REMOVED) {
                removed = 1;
                if (request && devices == 1)
                    ei_seat_request_device_with_capabilities(ei_device_get_seat(ei_event_get_device(event)), EI_DEVICE_CAP_KEYBOARD, NULL);
            }
            if (type == EI_EVENT_DISCONNECT) {
                ei_event_unref(event); ei_unref(context);
                if (receiver) return sent || keymap_seen ? 9 : 0;
                if (flood) return sent && keymap_seen ? 0 : 10;
                if (repeat && stage != 2) return 14;
                if (request && devices != 2) return 15;
                return sent && keymap_seen && modifier_seen && (removed || !strcmp(mode, "abrupt")) ? 0 : 11;
            }
            ei_event_unref(event);
        }
        if (poll(&fd, 1, 100) < 0) return 12;
    }
    ei_unref(context);
    return 13;
}

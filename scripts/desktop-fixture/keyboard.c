#define _GNU_SOURCE
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>
#include <wayland-client.h>
#include <xkbcommon/xkbcommon.h>
#include "virtual-keyboard.h"

/* One device and one full keymap for the lifetime of a disposable desktop. */
static struct zwp_virtual_keyboard_manager_v1 *manager;
static struct wl_seat *seat;
static void global(void *data, struct wl_registry *registry, uint32_t id,
                   const char *interface, uint32_t version) {
    (void)data; (void)version;
    if (!strcmp(interface, zwp_virtual_keyboard_manager_v1_interface.name))
        manager = wl_registry_bind(registry, id, &zwp_virtual_keyboard_manager_v1_interface, 1);
    else if (!strcmp(interface, wl_seat_interface.name) && !seat)
        seat = wl_registry_bind(registry, id, &wl_seat_interface, 1);
}
static void removed(void *data, struct wl_registry *registry, uint32_t id) {
    (void)data; (void)registry; (void)id;
}
static uint32_t timestamp(void) {
    struct timespec value;
    clock_gettime(CLOCK_MONOTONIC, &value);
    return value.tv_sec * 1000 + value.tv_nsec / 1000000;
}
int main(void) {
    struct wl_display *display = wl_display_connect(NULL);
    if (!display) return 2;
    struct wl_registry *registry = wl_display_get_registry(display);
    const struct wl_registry_listener listener = { global, removed };
    wl_registry_add_listener(registry, &listener, NULL);
    if (wl_display_roundtrip(display) < 0 || !manager || !seat) return 3;
    struct xkb_context *context = xkb_context_new(XKB_CONTEXT_NO_FLAGS);
    if (!context) return 4;
    const struct xkb_rule_names names = { .layout = "us" };
    struct xkb_keymap *keymap = xkb_keymap_new_from_names(context, &names, XKB_KEYMAP_COMPILE_NO_FLAGS);
    if (!keymap) return 4;
    char *map = xkb_keymap_get_as_string(keymap, XKB_KEYMAP_FORMAT_TEXT_V1);
    if (!map) return 4;
    xkb_mod_index_t control = xkb_keymap_mod_get_index(keymap, XKB_MOD_NAME_CTRL);
    if (control >= 32) return 4;
    uint32_t control_mask = 1u << control;
    size_t size = strlen(map) + 1;
    int fd = memfd_create("fixture-keymap", MFD_CLOEXEC);
    if (fd < 0 || write(fd, map, size) != (ssize_t)size) return 5;
    struct zwp_virtual_keyboard_v1 *keyboard = zwp_virtual_keyboard_manager_v1_create_virtual_keyboard(manager, seat);
    zwp_virtual_keyboard_v1_keymap(keyboard, WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1, fd, size);
    if (wl_display_roundtrip(display) < 0) return 6;
    close(fd); free(map); xkb_keymap_unref(keymap); xkb_context_unref(context);
    puts("ready"); fflush(stdout);
    char line[32];
    while (fgets(line, sizeof(line), stdin)) {
        char *end;
        int ctrl = !strncmp(line, "ctrl ", 5);
        char *digits = ctrl ? line + 5 : line;
        errno = 0;
        unsigned long code = strtoul(digits, &end, 10);
        if (errno || code > 767 || end == digits || strcmp(end, "\n")) return 7;
        zwp_virtual_keyboard_v1_modifiers(keyboard, ctrl ? control_mask : 0, 0, 0, 0);
        zwp_virtual_keyboard_v1_key(keyboard, timestamp(), code, WL_KEYBOARD_KEY_STATE_PRESSED);
        if (wl_display_roundtrip(display) < 0) return 6;
        usleep(50000);
        zwp_virtual_keyboard_v1_key(keyboard, timestamp(), code, WL_KEYBOARD_KEY_STATE_RELEASED);
        zwp_virtual_keyboard_v1_modifiers(keyboard, 0, 0, 0, 0);
        if (wl_display_roundtrip(display) < 0) return 6;
        puts("done"); fflush(stdout);
    }
    zwp_virtual_keyboard_v1_destroy(keyboard);
    wl_display_roundtrip(display);
    wl_display_disconnect(display);
    return 0;
}

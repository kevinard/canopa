#define _DEFAULT_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <wayland-client.h>
#include "virtual-pointer.h"

static struct zwlr_virtual_pointer_manager_v1 *manager;
static void global(void *data, struct wl_registry *registry, uint32_t id,
                   const char *interface, uint32_t version) {
    (void)data;
    (void)version;
    if (!strcmp(interface, zwlr_virtual_pointer_manager_v1_interface.name))
        manager = wl_registry_bind(registry, id, &zwlr_virtual_pointer_manager_v1_interface, 1);
}
static void removed(void *data, struct wl_registry *registry, uint32_t id) {
    (void)data; (void)registry; (void)id;
}
static uint32_t timestamp(void) {
    struct timespec value;
    clock_gettime(CLOCK_MONOTONIC, &value);
    return value.tv_sec * 1000 + value.tv_nsec / 1000000;
}
int main(int argc, char **argv) {
    if (argc != 3 && argc != 4) return 2;
    uint32_t button = argc == 4 && !strcmp(argv[3], "right") ? 273 : 272;
    struct wl_display *display = wl_display_connect(NULL);
    if (!display) return 3;
    struct wl_registry *registry = wl_display_get_registry(display);
    const struct wl_registry_listener listener = { global, removed };
    wl_registry_add_listener(registry, &listener, NULL);
    if (wl_display_roundtrip(display) < 0 || !manager) return 4;
    struct zwlr_virtual_pointer_v1 *pointer = zwlr_virtual_pointer_manager_v1_create_virtual_pointer(manager, NULL);
    wl_display_roundtrip(display);
    usleep(250000);
    zwlr_virtual_pointer_v1_motion_absolute(pointer, timestamp(), atoi(argv[1]), atoi(argv[2]), 1600, 1000);
    zwlr_virtual_pointer_v1_frame(pointer);
    wl_display_roundtrip(display);
    usleep(250000);
    if (argc == 4 && !strcmp(argv[3], "move")) {
        wl_display_disconnect(display);
        return 0;
    }
    zwlr_virtual_pointer_v1_button(pointer, timestamp(), button, WL_POINTER_BUTTON_STATE_PRESSED);
    zwlr_virtual_pointer_v1_frame(pointer);
    wl_display_roundtrip(display);
    usleep(100000);
    zwlr_virtual_pointer_v1_button(pointer, timestamp(), button, WL_POINTER_BUTTON_STATE_RELEASED);
    zwlr_virtual_pointer_v1_frame(pointer);
    wl_display_roundtrip(display);
    usleep(250000);
    wl_display_disconnect(display);
    return 0;
}

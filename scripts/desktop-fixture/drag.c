/* Drive an actual Wayland pointer drag, keeping it held for pre-drop checks. */
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
    (void)data; (void)version;
    if (!strcmp(interface, zwlr_virtual_pointer_manager_v1_interface.name))
        manager = wl_registry_bind(registry, id, &zwlr_virtual_pointer_manager_v1_interface, 1);
}
static void removed(void *data, struct wl_registry *registry, uint32_t id) {
    (void)data; (void)registry; (void)id;
}
static uint32_t timestamp(void) {
    struct timespec value; clock_gettime(CLOCK_MONOTONIC, &value);
    return value.tv_sec * 1000 + value.tv_nsec / 1000000;
}
static void motion(struct zwlr_virtual_pointer_v1 *pointer, struct wl_display *display, int x, int y) {
    zwlr_virtual_pointer_v1_motion_absolute(pointer, timestamp(), x, y, 1600, 1000);
    zwlr_virtual_pointer_v1_frame(pointer); wl_display_roundtrip(display);
}
int main(int argc, char **argv) {
    if (argc != 7) return 2;
    struct wl_display *display = wl_display_connect(NULL);
    if (!display) return 3;
    struct wl_registry *registry = wl_display_get_registry(display);
    const struct wl_registry_listener listener = { global, removed };
    wl_registry_add_listener(registry, &listener, NULL);
    if (wl_display_roundtrip(display) < 0 || !manager) return 4;
    struct zwlr_virtual_pointer_v1 *pointer = zwlr_virtual_pointer_manager_v1_create_virtual_pointer(manager, NULL);
    wl_display_roundtrip(display); usleep(250000);
    int x1 = atoi(argv[1]), y1 = atoi(argv[2]), x2 = atoi(argv[3]), y2 = atoi(argv[4]);
    motion(pointer, display, x1, y1); usleep(250000);
    zwlr_virtual_pointer_v1_button(pointer, timestamp(), 272, WL_POINTER_BUTTON_STATE_PRESSED);
    zwlr_virtual_pointer_v1_frame(pointer); wl_display_roundtrip(display); usleep(150000);
    for (int step = 1; step <= 20; step++) {
        motion(pointer, display, x1 + (x2 - x1) * step / 20, y1 + (y2 - y1) * step / 20);
        usleep(25000);
    }
    usleep(500000);
    FILE *ready = fopen(argv[5], "w"); if (!ready) return 5; fputs("held\n", ready); fclose(ready);
    for (int tick = 0; access(argv[6], F_OK) && tick < 200; tick++) {
        if (wl_display_roundtrip(display) < 0) return 6;
        usleep(100000);
    }
    zwlr_virtual_pointer_v1_button(pointer, timestamp(), 272, WL_POINTER_BUTTON_STATE_RELEASED);
    zwlr_virtual_pointer_v1_frame(pointer); wl_display_roundtrip(display); usleep(300000);
    wl_display_disconnect(display); return 0;
}

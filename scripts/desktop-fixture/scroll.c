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
    if (argc != 5 && argc != 6) return 2;
    unsigned int interval = argc == 6 ? atoi(argv[5]) : 8333;
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
    for (int i=0; i<atoi(argv[4]); i++) {
        zwlr_virtual_pointer_v1_axis_source(pointer, WL_POINTER_AXIS_SOURCE_FINGER);
        zwlr_virtual_pointer_v1_axis(pointer, timestamp(), WL_POINTER_AXIS_VERTICAL_SCROLL, wl_fixed_from_double(atof(argv[3])));
        zwlr_virtual_pointer_v1_frame(pointer);
        wl_display_flush(display);
        usleep(interval);
    }
    zwlr_virtual_pointer_v1_axis_stop(pointer, timestamp(), WL_POINTER_AXIS_VERTICAL_SCROLL);
    zwlr_virtual_pointer_v1_frame(pointer);
    wl_display_roundtrip(display);
    usleep(500000);
    wl_display_disconnect(display);
    return 0;
}

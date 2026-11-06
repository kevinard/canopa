/* Persistent private-desktop pointer source for capture/revocation acceptance. */
#define _DEFAULT_SOURCE
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
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
    struct timespec value;
    clock_gettime(CLOCK_MONOTONIC, &value);
    return value.tv_sec * 1000 + value.tv_nsec / 1000000;
}
int main(void) {
    struct wl_display *display = wl_display_connect(NULL);
    if (!display) return 2;
    const struct wl_registry_listener listener = {global, removed};
    wl_registry_add_listener(wl_display_get_registry(display), &listener, NULL);
    if (wl_display_roundtrip(display) < 0 || !manager) return 3;
    struct zwlr_virtual_pointer_v1 *pointer =
        zwlr_virtual_pointer_manager_v1_create_virtual_pointer(manager, NULL);
    if (wl_display_roundtrip(display) < 0) return 4;
    puts("ready"); fflush(stdout);
    char line[256];
    unsigned sequence = 0;
    while (fgets(line, sizeof(line), stdin)) {
        if (!strchr(line, '\n')) return 5;
        double dx, dy;
        int discrete;
        unsigned x, y, width, height, code, state, count, axis;
        if (sscanf(line, "absolute %u %u %u %u", &x, &y, &width, &height) == 4
            && width && height && x <= width && y <= height) {
            zwlr_virtual_pointer_v1_motion_absolute(pointer, timestamp(), x, y, width, height);
            zwlr_virtual_pointer_v1_frame(pointer);
        } else if (sscanf(line, "relative %lf %lf", &dx, &dy) == 2
                   && isfinite(dx) && isfinite(dy) && fabs(dx) <= 1000000 && fabs(dy) <= 1000000) {
            zwlr_virtual_pointer_v1_motion(pointer, timestamp(), wl_fixed_from_double(dx), wl_fixed_from_double(dy));
            zwlr_virtual_pointer_v1_frame(pointer);
        } else if (sscanf(line, "button %u %u", &code, &state) == 2 && code <= 65535 && state <= 1) {
            zwlr_virtual_pointer_v1_button(pointer, timestamp(), code, state);
            zwlr_virtual_pointer_v1_frame(pointer);
        } else if (sscanf(line, "wheel %u %lf %d", &axis, &dy, &discrete) == 3 && axis <= 1
                   && isfinite(dy) && fabs(dy) <= 1000000 && discrete >= -1000 && discrete <= 1000) {
            zwlr_virtual_pointer_v1_axis_source(pointer, WL_POINTER_AXIS_SOURCE_WHEEL);
            zwlr_virtual_pointer_v1_axis_discrete(pointer, timestamp(), axis, wl_fixed_from_double(dy), discrete);
            zwlr_virtual_pointer_v1_frame(pointer);
        } else if (sscanf(line, "axis %u %lf", &axis, &dy) == 2 && axis <= 1
                   && isfinite(dy) && fabs(dy) <= 1000000) {
            zwlr_virtual_pointer_v1_axis_source(pointer, WL_POINTER_AXIS_SOURCE_FINGER);
            if (dy == 0) zwlr_virtual_pointer_v1_axis_stop(pointer, timestamp(), axis);
            else zwlr_virtual_pointer_v1_axis(pointer, timestamp(), axis, wl_fixed_from_double(dy));
            zwlr_virtual_pointer_v1_frame(pointer);
        } else if (sscanf(line, "burst %u", &count) == 1 && count <= 20000) {
            for (unsigned i = 0; i < count; i++) {
                zwlr_virtual_pointer_v1_motion(pointer, timestamp(), 0, 0);
                zwlr_virtual_pointer_v1_frame(pointer);
            }
        } else if (!strcmp(line, "barrier\n")) {
            /* Round-trip only, preserving the source's held state. */
        } else if (!strcmp(line, "quit\n")) {
            break;
        } else return 6;
        if (wl_display_roundtrip(display) < 0) return 7;
        printf("ack %u\n", ++sequence); fflush(stdout);
    }
    zwlr_virtual_pointer_v1_destroy(pointer);
    if (wl_display_roundtrip(display) < 0) return 8;
    wl_display_disconnect(display);
    return 0;
}

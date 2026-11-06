/* Private xdg-foreign-v2 parent for portal acceptance. No toolkit dependency. */
#define _GNU_SOURCE
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>
#include <wayland-client.h>
#include "xdg-shell.h"
#include "xdg-foreign.h"
#ifdef PORTAL_CHOSEN
#include "xdg-activation.h"
static struct xdg_activation_v1 *activation;
#endif

static struct wl_compositor *compositor;
static struct wl_shm *shm;
static struct xdg_wm_base *shell;
static struct zxdg_exporter_v2 *exporter;
static struct wl_surface *surface;
static struct wl_buffer *buffer;
static int mapped;
#ifdef PORTAL_CAPTURE_CURSOR
static struct wl_surface *cursor_surface;
static void pointer_enter(void *data, struct wl_pointer *pointer, uint32_t serial,
                          struct wl_surface *entered, wl_fixed_t x, wl_fixed_t y) {
    (void)data; (void)entered;
    wl_pointer_set_cursor(pointer, serial, cursor_surface, 3, 5);
    printf("cursor-enter %.2f %.2f\n", wl_fixed_to_double(x), wl_fixed_to_double(y));
    fflush(stdout);
}
static void pointer_leave(void *d, struct wl_pointer *p, uint32_t s, struct wl_surface *w) {
    (void)d; (void)p; (void)s; (void)w;
}
static void pointer_motion(void *d, struct wl_pointer *p, uint32_t t, wl_fixed_t x, wl_fixed_t y) {
    (void)d; (void)p; (void)t; (void)x; (void)y;
}
static void pointer_button(void *d, struct wl_pointer *p, uint32_t s, uint32_t t, uint32_t b, uint32_t state) {
    (void)d; (void)p; (void)s; (void)t; (void)b; (void)state;
}
static void pointer_axis(void *d, struct wl_pointer *p, uint32_t t, uint32_t a, wl_fixed_t v) {
    (void)d; (void)p; (void)t; (void)a; (void)v;
}
static const struct wl_pointer_listener pointer_listener = {
    .enter = pointer_enter, .leave = pointer_leave, .motion = pointer_motion,
    .button = pointer_button, .axis = pointer_axis
};
static void capabilities(void *data, struct wl_seat *seat, uint32_t caps) {
    (void)data;
    if (caps & WL_SEAT_CAPABILITY_POINTER)
        wl_pointer_add_listener(wl_seat_get_pointer(seat), &pointer_listener, NULL);
}
static const struct wl_seat_listener seat_listener = { .capabilities = capabilities };
#endif

static void ping(void *data, struct xdg_wm_base *base, uint32_t serial) {
    (void)data;
    xdg_wm_base_pong(base, serial);
}
static const struct xdg_wm_base_listener shell_listener = { .ping = ping };
static void global(void *data, struct wl_registry *registry, uint32_t id,
                   const char *interface, uint32_t version) {
    (void)data; (void)version;
#ifdef PORTAL_CHOSEN
    if (!strcmp(interface, xdg_activation_v1_interface.name))
        activation = wl_registry_bind(registry, id, &xdg_activation_v1_interface, 1);
#endif
#ifdef PORTAL_CAPTURE_CURSOR
    if (!strcmp(interface, wl_seat_interface.name)) {
        struct wl_seat *seat = wl_registry_bind(registry, id, &wl_seat_interface, 1);
        wl_seat_add_listener(seat, &seat_listener, NULL);
    }
#endif
    if (!strcmp(interface, wl_compositor_interface.name))
        compositor = wl_registry_bind(registry, id, &wl_compositor_interface, 1);
    else if (!strcmp(interface, wl_shm_interface.name))
        shm = wl_registry_bind(registry, id, &wl_shm_interface, 1);
    else if (!strcmp(interface, xdg_wm_base_interface.name)) {
        shell = wl_registry_bind(registry, id, &xdg_wm_base_interface, 1);
        xdg_wm_base_add_listener(shell, &shell_listener, NULL);
    } else if (!strcmp(interface, zxdg_exporter_v2_interface.name))
        exporter = wl_registry_bind(registry, id, &zxdg_exporter_v2_interface, 1);
}
static void removed(void *data, struct wl_registry *registry, uint32_t id) {
    (void)data; (void)registry; (void)id;
}
static const struct wl_registry_listener registry_listener = { global, removed };
static void configure(void *data, struct xdg_surface *xdg, uint32_t serial) {
    (void)data;
    xdg_surface_ack_configure(xdg, serial);
    if (!mapped) {
        wl_surface_attach(surface, buffer, 0, 0);
        wl_surface_damage(surface, 0, 0, 400, 300);
        wl_surface_commit(surface);
        mapped = 1;
#ifdef PORTAL_CHOSEN
        const char *token = getenv("XDG_ACTIVATION_TOKEN");
        if (activation && token && *token) xdg_activation_v1_activate(activation, token, surface);
        unsetenv("XDG_ACTIVATION_TOKEN");
#endif
    }
}
static const struct xdg_surface_listener surface_listener = { .configure = configure };
static void toplevel_configure(void *data, struct xdg_toplevel *object, int32_t width,
                              int32_t height, struct wl_array *states) {
    (void)data; (void)object; (void)width; (void)height; (void)states;
}
static void toplevel_close(void *data, struct xdg_toplevel *object) {
    (void)data; (void)object;
    exit(0);
}
static const struct xdg_toplevel_listener toplevel_listener = {
    .configure = toplevel_configure, .close = toplevel_close
};
static void exported(void *data, struct zxdg_exported_v2 *object, const char *handle) {
    (void)data; (void)object;
    printf("wayland:%s\n", handle);
    fflush(stdout);
}
static const struct zxdg_exported_v2_listener exported_listener = { .handle = exported };

int main(void) {
    struct wl_display *display = wl_display_connect(NULL);
    if (!display) return 1;
    struct wl_registry *registry = wl_display_get_registry(display);
    wl_registry_add_listener(registry, &registry_listener, NULL);
    if (wl_display_roundtrip(display) < 0 || !compositor || !shm || !shell || !exporter) return 2;
    const int length = 400 * 300 * 4;
    int fd = memfd_create("canopa-portal-parent", MFD_CLOEXEC);
    if (fd < 0 || ftruncate(fd, length) < 0) return 3;
    uint32_t *pixels = mmap(NULL, length, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (pixels == MAP_FAILED) return 4;
    for (int i = 0; i < 400 * 300; ++i) {
#ifdef PORTAL_CAPTURE_PATTERN
        int x = i % 400, y = i / 400;
        pixels[i] = x < 137 ? (y < 93 ? 0xffb41428 : 0xff0a288c)
                            : (y < 181 ? 0xff149628 : 0xffb49a18);
#else
        pixels[i] = 0xff203040;
#endif
    }
    struct wl_shm_pool *pool = wl_shm_create_pool(shm, fd, length);
    buffer = wl_shm_pool_create_buffer(pool, 0, 400, 300, 400 * 4, WL_SHM_FORMAT_XRGB8888);
    wl_shm_pool_destroy(pool);
    close(fd);
#ifdef PORTAL_CAPTURE_CURSOR
    int cursor_fd = memfd_create("canopa-portal-cursor", MFD_CLOEXEC);
    const int cursor_bytes = 24 * 24 * 4;
    if (cursor_fd < 0 || ftruncate(cursor_fd, cursor_bytes) < 0) return 5;
    uint32_t *cursor_pixels = mmap(NULL, cursor_bytes, PROT_READ | PROT_WRITE, MAP_SHARED, cursor_fd, 0);
    if (cursor_pixels == MAP_FAILED) return 6;
    for (int y = 0; y < 24; ++y)
        for (int x = 0; x < 24; ++x)
            cursor_pixels[y * 24 + x] = ((x / 4 + y / 4) % 2) ? 0xffff00ff : 0xff00ffff;
    struct wl_shm_pool *cursor_pool = wl_shm_create_pool(shm, cursor_fd, cursor_bytes);
    struct wl_buffer *cursor_buffer = wl_shm_pool_create_buffer(cursor_pool, 0, 24, 24, 24 * 4, WL_SHM_FORMAT_ARGB8888);
    wl_shm_pool_destroy(cursor_pool);
    close(cursor_fd);
    cursor_surface = wl_compositor_create_surface(compositor);
    wl_surface_attach(cursor_surface, cursor_buffer, 0, 0);
    wl_surface_damage(cursor_surface, 0, 0, 24, 24);
    wl_surface_commit(cursor_surface);
#endif
    surface = wl_compositor_create_surface(compositor);
    struct xdg_surface *xdg = xdg_wm_base_get_xdg_surface(shell, surface);
    xdg_surface_add_listener(xdg, &surface_listener, NULL);
    struct xdg_toplevel *toplevel = xdg_surface_get_toplevel(xdg);
    xdg_toplevel_add_listener(toplevel, &toplevel_listener, NULL);
#ifdef PORTAL_CHOSEN
    xdg_toplevel_set_title(toplevel, "Portal chosen application");
    xdg_toplevel_set_app_id(toplevel, "org.canopa.PortalChosen");
#else
    xdg_toplevel_set_title(toplevel, "Portal test parent");
    xdg_toplevel_set_app_id(toplevel, "org.canopa.PortalTest");
#endif
    wl_surface_commit(surface);
    struct zxdg_exported_v2 *object = zxdg_exporter_v2_export_toplevel(exporter, surface);
    zxdg_exported_v2_add_listener(object, &exported_listener, NULL);
    while (wl_display_dispatch(display) >= 0) {}
    return 0;
}

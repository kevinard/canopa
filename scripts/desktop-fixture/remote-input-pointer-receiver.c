/* Harmless private-desktop input receiver. No toolkit or live session input. */
#define _GNU_SOURCE
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>
#include <wayland-client.h>
#include <xkbcommon/xkbcommon.h>
#include "xdg-shell.h"

static struct wl_compositor *compositor;
static struct wl_shm *shm;
static struct xdg_wm_base *shell;
static struct wl_surface *surface;
static struct wl_buffer *buffer;
static struct xkb_context *context;
static struct xkb_keymap *keymap;
static struct xkb_state *state;
static int mapped;
static int wanted_width=400,wanted_height=300,current_width,current_height;

static void keyboard_keymap(void *d, struct wl_keyboard *k, uint32_t format, int fd, uint32_t size) {
    (void)d; (void)k;
    if (format != WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1 || !size || size > 1024 * 1024) exit(8);
    char *map = mmap(NULL, size, PROT_READ, MAP_PRIVATE, fd, 0);
    if (map == MAP_FAILED) exit(9);
    if (state) xkb_state_unref(state);
    if (keymap) xkb_keymap_unref(keymap);
    keymap = xkb_keymap_new_from_buffer(context, map, size, XKB_KEYMAP_FORMAT_TEXT_V1, XKB_KEYMAP_COMPILE_NO_FLAGS);
    if (!keymap || !(state = xkb_state_new(keymap))) exit(10);
    munmap(map, size); close(fd);
    puts("keymap"); fflush(stdout);
}
static void keyboard_enter(void *d, struct wl_keyboard *k, uint32_t serial, struct wl_surface *s, struct wl_array *keys) {
    (void)d; (void)k; (void)serial; (void)s; (void)keys;
    puts("focused"); fflush(stdout);
}
static void keyboard_leave(void *d, struct wl_keyboard *k, uint32_t serial, struct wl_surface *s) {
    (void)d; (void)k; (void)serial; (void)s;
}
static void keyboard_key(void *d, struct wl_keyboard *k, uint32_t serial, uint32_t time, uint32_t key, uint32_t pressed) {
    (void)d; (void)k; (void)serial; (void)time;
    char name[128];
    xkb_keysym_get_name(xkb_state_key_get_one_sym(state, key + 8), name, sizeof(name));
    printf("key %u %u %s\n", key, pressed, name); fflush(stdout);
}
static void keyboard_modifiers(void *d, struct wl_keyboard *k, uint32_t serial, uint32_t depressed, uint32_t latched, uint32_t locked, uint32_t group) {
    (void)d; (void)k; (void)serial;
    if (!state) exit(11);
    xkb_state_update_mask(state, depressed, latched, locked, 0, 0, group);
    printf("mods %u %u %u %u\n", depressed, latched, locked, group); fflush(stdout);
}
static void keyboard_repeat(void *d, struct wl_keyboard *k, int32_t rate, int32_t delay) {
    (void)d; (void)k; (void)rate; (void)delay;
}
static const struct wl_keyboard_listener keyboard_listener = {
    .keymap = keyboard_keymap, .enter = keyboard_enter, .leave = keyboard_leave,
    .key = keyboard_key, .modifiers = keyboard_modifiers, .repeat_info = keyboard_repeat,
};

static void pointer_enter(void*d,struct wl_pointer*p,uint32_t s,struct wl_surface*w,wl_fixed_t x,wl_fixed_t y){(void)d;(void)p;(void)s;(void)w;printf("enter %.2f %.2f\n",wl_fixed_to_double(x),wl_fixed_to_double(y));fflush(stdout);}
static void pointer_leave(void*d,struct wl_pointer*p,uint32_t s,struct wl_surface*w){(void)d;(void)p;(void)s;(void)w;puts("pointer-leave");fflush(stdout);}
static void pointer_motion(void*d,struct wl_pointer*p,uint32_t t,wl_fixed_t x,wl_fixed_t y){(void)d;(void)p;(void)t;printf("motion %.2f %.2f\n",wl_fixed_to_double(x),wl_fixed_to_double(y));fflush(stdout);}
static void pointer_button(void*d,struct wl_pointer*p,uint32_t s,uint32_t t,uint32_t b,uint32_t v){(void)d;(void)p;(void)s;(void)t;printf("button %u %u\n",b,v);fflush(stdout);}
static void pointer_axis(void*d,struct wl_pointer*p,uint32_t t,uint32_t a,wl_fixed_t v){(void)d;(void)p;(void)t;printf("axis %u %.2f\n",a,wl_fixed_to_double(v));fflush(stdout);}
static void pointer_frame(void*d,struct wl_pointer*p){(void)d;(void)p;}
static void pointer_source(void*d,struct wl_pointer*p,uint32_t s){(void)d;(void)p;printf("axis-source %u\n",s);fflush(stdout);}
static void pointer_stop(void*d,struct wl_pointer*p,uint32_t t,uint32_t a){(void)d;(void)p;(void)t;printf("axis-stop %u\n",a);fflush(stdout);}
static void pointer_discrete(void*d,struct wl_pointer*p,uint32_t a,int32_t v){(void)d;(void)p;printf("axis-discrete %u %d\n",a,v);fflush(stdout);}
static const struct wl_pointer_listener pointer_listener={.enter=pointer_enter,.leave=pointer_leave,.motion=pointer_motion,.button=pointer_button,.axis=pointer_axis,.frame=pointer_frame,.axis_source=pointer_source,.axis_stop=pointer_stop,.axis_discrete=pointer_discrete};

static void capabilities(void *d, struct wl_seat *seat, uint32_t caps) {
    (void)d;
    if (caps & WL_SEAT_CAPABILITY_POINTER) wl_pointer_add_listener(wl_seat_get_pointer(seat), &pointer_listener, NULL);
    if (caps & WL_SEAT_CAPABILITY_KEYBOARD)
        wl_keyboard_add_listener(wl_seat_get_keyboard(seat), &keyboard_listener, NULL);
}
static void seat_name(void *d, struct wl_seat *seat, const char *name) { (void)d; (void)seat; (void)name; }
static const struct wl_seat_listener seat_listener = { .capabilities = capabilities, .name = seat_name };
static void ping(void *d, struct xdg_wm_base *base, uint32_t serial) { (void)d; xdg_wm_base_pong(base, serial); }
static const struct xdg_wm_base_listener shell_listener = { .ping = ping };
static void global(void *d, struct wl_registry *registry, uint32_t id, const char *interface, uint32_t version) {
    (void)d; (void)version;
    if (!strcmp(interface, wl_compositor_interface.name))
        compositor = wl_registry_bind(registry, id, &wl_compositor_interface, 1);
    else if (!strcmp(interface, wl_shm_interface.name))
        shm = wl_registry_bind(registry, id, &wl_shm_interface, 1);
    else if (!strcmp(interface, xdg_wm_base_interface.name)) {
        shell = wl_registry_bind(registry, id, &xdg_wm_base_interface, 1);
        xdg_wm_base_add_listener(shell, &shell_listener, NULL);
    } else if (!strcmp(interface, wl_seat_interface.name)) {
        struct wl_seat *seat = wl_registry_bind(registry, id, &wl_seat_interface, version < 7 ? version : 7);
        wl_seat_add_listener(seat, &seat_listener, NULL);
    }
}
static void removed(void *d, struct wl_registry *registry, uint32_t id) { (void)d; (void)registry; (void)id; }
static const struct wl_registry_listener registry_listener = { .global = global, .global_remove = removed };
static struct wl_buffer *new_buffer(int width,int height){
    if(width<1||height<1||width>8192||height>8192)exit(20);
    int length=width*height*4;int fd=memfd_create("private-capture-metadata",MFD_CLOEXEC);if(fd<0||ftruncate(fd,length))exit(21);
    uint32_t *pixels=mmap(NULL,length,PROT_READ|PROT_WRITE,MAP_SHARED,fd,0);if(pixels==MAP_FAILED)exit(22);
    for(int y=0;y<height;y++)for(int x=0;x<width;x++)pixels[y*width+x]=y<height/2?(x<width/2?0xffd02020:0xff20b050):(x<width/2?0xff304fd0:0xffe0b020);
    struct wl_shm_pool *pool=wl_shm_create_pool(shm,fd,length);struct wl_buffer *next=wl_shm_pool_create_buffer(pool,0,width,height,width*4,WL_SHM_FORMAT_XRGB8888);wl_shm_pool_destroy(pool);munmap(pixels,length);close(fd);return next;
}
static void configure(void*d,struct xdg_surface*xdg,uint32_t serial){
    (void)d;xdg_surface_ack_configure(xdg,serial);
    if(!mapped||wanted_width!=current_width||wanted_height!=current_height){
        struct wl_buffer *old=buffer;buffer=new_buffer(wanted_width,wanted_height);wl_surface_attach(surface,buffer,0,0);wl_surface_damage(surface,0,0,wanted_width,wanted_height);wl_surface_commit(surface);if(old)wl_buffer_destroy(old);current_width=wanted_width;current_height=wanted_height;mapped=1;printf("configured %d %d\n",current_width,current_height);fflush(stdout);
    }
}
static const struct xdg_surface_listener surface_listener = { .configure = configure };
static void toplevel_configure(void *d, struct xdg_toplevel *t, int32_t w, int32_t h, struct wl_array *a) {
    (void)d; (void)t; (void)a;if(w>0)wanted_width=w;if(h>0)wanted_height=h;
}
static void toplevel_close(void *d, struct xdg_toplevel *t) { (void)d; (void)t; exit(0); }
static const struct xdg_toplevel_listener toplevel_listener = { .configure = toplevel_configure, .close = toplevel_close };
int main(void) {
    context = xkb_context_new(XKB_CONTEXT_NO_FLAGS);
    struct wl_display *display = wl_display_connect(NULL);
    if (!display || !context) return 1;
    wl_registry_add_listener(wl_display_get_registry(display), &registry_listener, NULL);
    if (wl_display_roundtrip(display) < 0 || !compositor || !shm || !shell) return 2;
    surface = wl_compositor_create_surface(compositor);
    struct xdg_surface *xdg = xdg_wm_base_get_xdg_surface(shell, surface);
    xdg_surface_add_listener(xdg, &surface_listener, NULL);
    struct xdg_toplevel *toplevel = xdg_surface_get_toplevel(xdg);
    xdg_toplevel_add_listener(toplevel, &toplevel_listener, NULL);
    xdg_toplevel_set_title(toplevel, "Remote input receiver");
    xdg_toplevel_set_app_id(toplevel, "org.canopa.RemoteInputReceiver");
    wl_surface_commit(surface);
    while (wl_display_dispatch(display) >= 0) {}
    wl_display_disconnect(display);
    return 0;
}

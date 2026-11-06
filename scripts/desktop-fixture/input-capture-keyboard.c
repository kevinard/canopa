/* Persistent raw keyboard on an owned private Wayland display. */
#define _GNU_SOURCE
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>
#include <wayland-client.h>
#include <xkbcommon/xkbcommon.h>
#include "virtual-keyboard.h"
static struct zwp_virtual_keyboard_manager_v1 *manager;
static struct wl_seat *seat;
static struct xkb_context *context;
static struct xkb_keymap *map;
static struct xkb_state *state;
static unsigned held[768];
static void global(void *data, struct wl_registry *registry, uint32_t id, const char *name, uint32_t version) {
    (void)data; (void)version;
    if (!strcmp(name, zwp_virtual_keyboard_manager_v1_interface.name)) manager=wl_registry_bind(registry,id,&zwp_virtual_keyboard_manager_v1_interface,1);
    else if (!strcmp(name,wl_seat_interface.name) && !seat) seat=wl_registry_bind(registry,id,&wl_seat_interface,1);
}
static void removed(void*d,struct wl_registry*r,uint32_t id){(void)d;(void)r;(void)id;}
static void keymap(struct zwp_virtual_keyboard_v1 *keyboard,const char *layout) {
    for(unsigned i=0;i<768;i++) if(held[i]) exit(10);
    struct xkb_rule_names names={.layout=layout};
    struct xkb_keymap *next=xkb_keymap_new_from_names(context,&names,XKB_KEYMAP_COMPILE_NO_FLAGS);
    if(!next)exit(11);
    char *text=xkb_keymap_get_as_string(next,XKB_KEYMAP_FORMAT_TEXT_V1);
    size_t size=strlen(text)+1; int fd=memfd_create("capture-keyboard-fixture",MFD_CLOEXEC);
    if(fd<0 || write(fd,text,size)!=(ssize_t)size)exit(12);
    zwp_virtual_keyboard_v1_keymap(keyboard,WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1,fd,size);
    if(state)xkb_state_unref(state);
    if(map)xkb_keymap_unref(map);
    map=next;state=xkb_state_new(map);if(!state)exit(13);
    free(text);close(fd);
}
int main(void) {
    struct wl_display *display=wl_display_connect(NULL); if(!display)return 2;
    const struct wl_registry_listener listener={global,removed};
    wl_registry_add_listener(wl_display_get_registry(display),&listener,NULL);
    if(wl_display_roundtrip(display)<0 || !manager || !seat)return 3;
    context=xkb_context_new(XKB_CONTEXT_NO_FLAGS);if(!context)return 4;
    struct zwp_virtual_keyboard_v1 *keyboard=zwp_virtual_keyboard_manager_v1_create_virtual_keyboard(manager,seat);
    keymap(keyboard,"us");
    zwp_virtual_keyboard_v1_modifiers(keyboard,0,0,0,0);
    if(wl_display_roundtrip(display)<0)return 5;
    puts("ready");fflush(stdout); char line[128];unsigned sequence=0;
    while(fgets(line,sizeof(line),stdin)) {
        unsigned code,pressed,d,l,k,g;char layout[8];
        if(!strchr(line,'\n'))return 6;
        if(sscanf(line,"key %u %u",&code,&pressed)==2 && code>0 && code<768 && pressed<2 && held[code]!=pressed) {
            struct timespec now;clock_gettime(CLOCK_MONOTONIC,&now);
            held[code]=pressed; xkb_state_update_key(state,code+8,pressed?XKB_KEY_DOWN:XKB_KEY_UP);
            zwp_virtual_keyboard_v1_key(keyboard,(uint32_t)(now.tv_sec*1000+now.tv_nsec/1000000),code,pressed);
        } else if(!strcmp(line,"sync\n")) {
            zwp_virtual_keyboard_v1_modifiers(keyboard,xkb_state_serialize_mods(state,XKB_STATE_MODS_DEPRESSED),xkb_state_serialize_mods(state,XKB_STATE_MODS_LATCHED),xkb_state_serialize_mods(state,XKB_STATE_MODS_LOCKED),xkb_state_serialize_layout(state,XKB_STATE_LAYOUT_EFFECTIVE));
        } else if(sscanf(line,"mods %u %u %u %u",&d,&l,&k,&g)==4) {
            zwp_virtual_keyboard_v1_modifiers(keyboard,d,l,k,g);
        } else if(sscanf(line,"map %7s",layout)==1 && (!strcmp(layout,"us") || !strcmp(layout,"fr"))) {
            keymap(keyboard,layout);zwp_virtual_keyboard_v1_modifiers(keyboard,0,0,0,0);
        } else if(!strcmp(line,"quit\n"))break;
        else if(strcmp(line,"barrier\n"))return 7;
        if(wl_display_roundtrip(display)<0)return 8;
        printf("ack %u\n",++sequence);fflush(stdout);
    }
    zwp_virtual_keyboard_v1_destroy(keyboard);wl_display_roundtrip(display);
    xkb_state_unref(state);xkb_keymap_unref(map);xkb_context_unref(context);wl_display_disconnect(display);return 0;
}

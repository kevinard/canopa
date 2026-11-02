/* Private-compositor acceptance input. Fixed US keymap also works through nested
 * kiosk compositors, unlike per-message synthetic keymaps. */
#define _GNU_SOURCE
#include <wayland-client.h>
#include <xkbcommon/xkbcommon.h>
#include <sys/mman.h>
#include <unistd.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <ctype.h>
#include "virtual-keyboard.h"
static struct wl_seat *seat;
static struct zwp_virtual_keyboard_manager_v1 *manager;
static void global(void *data,struct wl_registry *registry,uint32_t id,const char *name,uint32_t version){
    (void)data;(void)version;
    if(strcmp(name,"wl_seat")==0)seat=wl_registry_bind(registry,id,&wl_seat_interface,1);
    if(strcmp(name,"zwp_virtual_keyboard_manager_v1")==0)manager=wl_registry_bind(registry,id,&zwp_virtual_keyboard_manager_v1_interface,1);
}
static void removed(void *data,struct wl_registry *registry,uint32_t id){(void)data;(void)registry;(void)id;}
static const struct wl_registry_listener listener={global,removed};
static unsigned code(char c){
    const char *rows[]={"1234567890-=","qwertyuiop[]","asdfghjkl;'","zxcvbnm,./"};const unsigned starts[]={2,16,30,44};
    if(c==' ')return 57;
    for(unsigned i=0;i<4;i++){const char *p=strchr(rows[i],c);if(p)return starts[i]+(unsigned)(p-rows[i]);}return 0;
}
static unsigned named(const char *s){
    if(!strcmp(s,"Return"))return 28;if(!strcmp(s,"Escape"))return 1;if(!strcmp(s,"Tab"))return 15;
    if(!strcmp(s,"Down"))return 108;if(!strcmp(s,"Up"))return 103;if(!strcmp(s,"Left"))return 105;if(!strcmp(s,"Right"))return 106;if(!strcmp(s,"BackSpace"))return 14;
    if(!strcmp(s,"Home"))return 102;if(!strcmp(s,"End"))return 107;if(!strcmp(s,"Delete"))return 111;
    if(!strcmp(s,"Next"))return 109;if(!strcmp(s,"Prior"))return 104;if(!strcmp(s,"F6"))return 64;return 0;
}
static void tap(struct wl_display *display,struct zwp_virtual_keyboard_v1 *keyboard,unsigned key){
    zwp_virtual_keyboard_v1_key(keyboard,0,key,1);wl_display_roundtrip(display);usleep(12000);
    zwp_virtual_keyboard_v1_key(keyboard,0,key,0);wl_display_roundtrip(display);
}
int main(int argc,char **argv){
    if(argc<2)return 2;struct wl_display *display=wl_display_connect(NULL);if(!display)return 3;
    struct wl_registry *registry=wl_display_get_registry(display);wl_registry_add_listener(registry,&listener,NULL);wl_display_roundtrip(display);
    if(!seat||!manager)return 4;
    struct zwp_virtual_keyboard_v1 *keyboard=zwp_virtual_keyboard_manager_v1_create_virtual_keyboard(manager,seat);
    struct xkb_context *context=xkb_context_new(XKB_CONTEXT_NO_FLAGS);const char *layouts=getenv("CANOPA_FIXTURE_LAYOUT");struct xkb_rule_names names={.layout=layouts?layouts:"us"};
    struct xkb_keymap *map=xkb_keymap_new_from_names(context,&names,XKB_KEYMAP_COMPILE_NO_FLAGS);char *text=xkb_keymap_get_as_string(map,XKB_KEYMAP_FORMAT_TEXT_V1);size_t length=strlen(text)+1;
    int fd=memfd_create("canopa-keymap",MFD_CLOEXEC);if(fd<0||ftruncate(fd,(off_t)length))return 5;
    if(write(fd,text,length)!=(ssize_t)length)return 6;
    zwp_virtual_keyboard_v1_keymap(keyboard,WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1,fd,(uint32_t)length);wl_display_roundtrip(display);close(fd);free(text);xkb_keymap_unref(map);xkb_context_unref(context);
    if(!strcmp(argv[1],"hold")){sleep(600);return 0;}
    if(argc!=3)return 2;
    if(!strcmp(argv[1],"held")){unsigned key=named(argv[2]);if(!key)return 7;
        zwp_virtual_keyboard_v1_key(keyboard,0,key,1);wl_display_roundtrip(display);usleep(2400000);
        zwp_virtual_keyboard_v1_key(keyboard,0,key,0);wl_display_roundtrip(display);
    }
    else if(!strcmp(argv[1],"key")){unsigned key=named(argv[2]);if(!key)return 7;tap(display,keyboard,key);}
    else if(!strcmp(argv[1],"sequence")){
        char *sequence=strdup(argv[2]);if(!sequence)return 9;
        for(char *token=strtok(sequence," ");token;token=strtok(NULL," ")){
            unsigned mods=0;if(!strncmp(token,"Ctrl+",5)){mods=4;token+=5;}
            if(!strncmp(token,"Shift+",6)){mods|=1;token+=6;}
            unsigned key=named(token);if(!key&&strlen(token)==1)key=code((char)tolower(token[0]));if(!key){free(sequence);return 7;}
            zwp_virtual_keyboard_v1_modifiers(keyboard,mods,0,0,0);wl_display_roundtrip(display);tap(display,keyboard,key);
            zwp_virtual_keyboard_v1_modifiers(keyboard,0,0,0,0);wl_display_roundtrip(display);
        }free(sequence);
    }
    else if(!strcmp(argv[1],"text")||!strcmp(argv[1],"type")){
        for(const unsigned char *p=(const unsigned char*)argv[2];*p;p++){
            unsigned key=code(*p=='%'?'5':*p=='+'?'=':(char)tolower(*p));if(!key)return 8;
            int shifted=isupper(*p)||*p=='%'||*p=='+';
            if(shifted)zwp_virtual_keyboard_v1_modifiers(keyboard,1,0,0,0);
            tap(display,keyboard,key);
            if(shifted)zwp_virtual_keyboard_v1_modifiers(keyboard,0,0,0,0);
        }
        if(!strcmp(argv[1],"text"))tap(display,keyboard,28);
    }else return 2;
    zwp_virtual_keyboard_v1_destroy(keyboard);wl_display_roundtrip(display);wl_display_disconnect(display);return 0;
}

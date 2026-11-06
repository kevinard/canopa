/* Private Wayland touch receiver, independent of GTK and Iced input adapters. */
#define _GNU_SOURCE
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>
#include <wayland-client.h>
#include "xdg-shell.h"
#include "idle-notify.h"

static struct wl_compositor *compositor;
static struct wl_shm *shm;
static struct xdg_wm_base *shell;
static struct wl_surface *surface;
static struct wl_touch *touch;
static struct wl_buffer *buffer;
static struct wl_seat *tracked_seat;
static struct ext_idle_notifier_v1 *idle_notifier;
static int width=400,height=300;
static void down(void*d,struct wl_touch*t,uint32_t s,uint32_t time,struct wl_surface*w,int32_t id,wl_fixed_t x,wl_fixed_t y){
    (void)d;(void)t;(void)s;(void)time;if(w!=surface)exit(5);
    printf("down %d %.2f %.2f\n",id,wl_fixed_to_double(x),wl_fixed_to_double(y));fflush(stdout);
}
static void up(void*d,struct wl_touch*t,uint32_t s,uint32_t time,int32_t id){(void)d;(void)t;(void)s;(void)time;printf("up %d\n",id);fflush(stdout);}
static void motion(void*d,struct wl_touch*t,uint32_t time,int32_t id,wl_fixed_t x,wl_fixed_t y){(void)d;(void)t;(void)time;printf("motion %d %.2f %.2f\n",id,wl_fixed_to_double(x),wl_fixed_to_double(y));fflush(stdout);}
static void frame(void*d,struct wl_touch*t){(void)d;(void)t;puts("frame");fflush(stdout);}
static void cancel(void*d,struct wl_touch*t){(void)d;(void)t;puts("cancel");fflush(stdout);}
static void shape(void*d,struct wl_touch*t,int32_t id,wl_fixed_t a,wl_fixed_t b){(void)d;(void)t;(void)id;(void)a;(void)b;}
static void orientation(void*d,struct wl_touch*t,int32_t id,wl_fixed_t angle){(void)d;(void)t;(void)id;(void)angle;}
static const struct wl_touch_listener touch_listener={.down=down,.up=up,.motion=motion,.frame=frame,.cancel=cancel,.shape=shape,.orientation=orientation};
static void capabilities(void*d,struct wl_seat*seat,uint32_t caps){
    (void)d;printf("caps %u\n",caps);fflush(stdout);
    if(caps&WL_SEAT_CAPABILITY_TOUCH){if(!touch){touch=wl_seat_get_touch(seat);wl_touch_add_listener(touch,&touch_listener,NULL);}}
    else if(touch){wl_touch_release(touch);touch=NULL;}
}
static void seat_name(void*d,struct wl_seat*s,const char*n){(void)d;(void)s;(void)n;}
static const struct wl_seat_listener seat_listener={.capabilities=capabilities,.name=seat_name};
static void idled(void*d,struct ext_idle_notification_v1*n){(void)d;(void)n;puts("idled");fflush(stdout);}
static void resumed(void*d,struct ext_idle_notification_v1*n){(void)d;(void)n;puts("resumed");fflush(stdout);}
static const struct ext_idle_notification_v1_listener idle_listener={.idled=idled,.resumed=resumed};
static void ping(void*d,struct xdg_wm_base*b,uint32_t s){(void)d;xdg_wm_base_pong(b,s);}
static const struct xdg_wm_base_listener shell_listener={.ping=ping};
static void global(void*d,struct wl_registry*r,uint32_t id,const char*name,uint32_t version){
    (void)d;
    if(!strcmp(name,"wl_compositor"))compositor=wl_registry_bind(r,id,&wl_compositor_interface,1);
    else if(!strcmp(name,"wl_shm"))shm=wl_registry_bind(r,id,&wl_shm_interface,1);
    else if(!strcmp(name,"xdg_wm_base")){shell=wl_registry_bind(r,id,&xdg_wm_base_interface,1);xdg_wm_base_add_listener(shell,&shell_listener,NULL);}
    else if(!strcmp(name,"wl_seat")){tracked_seat=wl_registry_bind(r,id,&wl_seat_interface,version<7?version:7);wl_seat_add_listener(tracked_seat,&seat_listener,NULL);}
    else if(!strcmp(name,"ext_idle_notifier_v1"))idle_notifier=wl_registry_bind(r,id,&ext_idle_notifier_v1_interface,1);
}
static void removed(void*d,struct wl_registry*r,uint32_t id){(void)d;(void)r;(void)id;}
static const struct wl_registry_listener registry_listener={.global=global,.global_remove=removed};
static void configure(void*d,struct xdg_surface*s,uint32_t serial){
    (void)d;xdg_surface_ack_configure(s,serial);
    int length=width*height*4;int fd=memfd_create("canopa-touch-fixture",MFD_CLOEXEC);
    if(fd<0||ftruncate(fd,length))exit(6);
    uint32_t*p=mmap(NULL,length,PROT_READ|PROT_WRITE,MAP_SHARED,fd,0);if(p==MAP_FAILED)exit(7);
    for(int i=0;i<width*height;i++)p[i]=0xff2090c0;
    struct wl_shm_pool*pool=wl_shm_create_pool(shm,fd,length);
    struct wl_buffer*next=wl_shm_pool_create_buffer(pool,0,width,height,width*4,WL_SHM_FORMAT_XRGB8888);
    wl_shm_pool_destroy(pool);munmap(p,length);close(fd);
    wl_surface_attach(surface,next,0,0);wl_surface_damage(surface,0,0,width,height);wl_surface_commit(surface);
    if(buffer)wl_buffer_destroy(buffer);
    buffer=next;printf("configured %d %d\n",width,height);fflush(stdout);
}
static const struct xdg_surface_listener surface_listener={.configure=configure};
static void size(void*d,struct xdg_toplevel*t,int32_t w,int32_t h,struct wl_array*a){(void)d;(void)t;(void)a;if(w>0)width=w;if(h>0)height=h;if(width>8192||height>8192)exit(8);}
static void close_window(void*d,struct xdg_toplevel*t){(void)d;(void)t;exit(0);}
static const struct xdg_toplevel_listener toplevel_listener={.configure=size,.close=close_window};
int main(void){
    struct wl_display*display=wl_display_connect(NULL);if(!display)return 1;
    wl_registry_add_listener(wl_display_get_registry(display),&registry_listener,NULL);
    if(wl_display_roundtrip(display)<0||!compositor||!shm||!shell)return 2;
    if(!idle_notifier||!tracked_seat)return 3;
    struct ext_idle_notification_v1*notification=ext_idle_notifier_v1_get_idle_notification(idle_notifier,200,tracked_seat);
    ext_idle_notification_v1_add_listener(notification,&idle_listener,NULL);
    surface=wl_compositor_create_surface(compositor);
    struct xdg_surface*s=xdg_wm_base_get_xdg_surface(shell,surface);xdg_surface_add_listener(s,&surface_listener,NULL);
    struct xdg_toplevel*t=xdg_surface_get_toplevel(s);xdg_toplevel_add_listener(t,&toplevel_listener,NULL);
    xdg_toplevel_set_app_id(t,"org.canopa.TouchReceiver");xdg_toplevel_set_title(t,"Private touch receiver");wl_surface_commit(surface);
    while(wl_display_dispatch(display)>=0){}
    wl_display_disconnect(display);return 0;
}

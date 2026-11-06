/* Real libei touch sender for private portal acceptance. */
#define _GNU_SOURCE
#include <libei.h>
#include <poll.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
static uint64_t now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return(uint64_t)t.tv_sec*1000000+t.tv_nsec/1000;}
int main(int argc,char**argv){
    if(argc!=2)return 2;
    int control=atoi(argv[1]);setvbuf(stdout,NULL,_IOLBF,0);
    struct ei*context=ei_new_sender(NULL);ei_configure_name(context,"Canopa touch fixture");
    if(ei_setup_backend_fd(context,STDIN_FILENO))return 3;
    struct pollfd fds[2]={{.fd=ei_get_fd(context),.events=POLLIN},{.fd=control,.events=POLLIN}};
    struct ei_seat*seat=NULL;struct ei_device*device=NULL;struct ei_touch*touches[32]={0};
    unsigned sequence=0,generation=0;char buffer[256];size_t filled=0;
    for(;;){
        ei_dispatch(context);struct ei_event*event;
        while((event=ei_get_event(context))){
            enum ei_event_type type=ei_event_get_type(event);struct ei_device*d=ei_event_get_device(event);
            if(type==EI_EVENT_SEAT_ADDED){
                seat=ei_seat_ref(ei_event_get_seat(event));
                ei_seat_bind_capabilities(seat,EI_DEVICE_CAP_TOUCH,NULL);
            }
            if(type==EI_EVENT_DEVICE_ADDED){
                if(device||!ei_device_has_capability(d,EI_DEVICE_CAP_TOUCH)
                    ||ei_device_has_capability(d,EI_DEVICE_CAP_POINTER)||ei_device_has_capability(d,EI_DEVICE_CAP_KEYBOARD))return 4;
                device=ei_device_ref(d);generation++;
                struct ei_region*r=ei_device_get_region(device,0);if(!r||!ei_region_get_mapping_id(r))return 5;
                printf("{\"added\":true,\"generation\":%u,\"width\":%u,\"height\":%u,\"scale\":%.8g,\"mapping\":\"%s\"}\n",
                    generation,ei_region_get_width(r),ei_region_get_height(r),ei_region_get_physical_scale(r),ei_region_get_mapping_id(r));
            }
            if(type==EI_EVENT_DEVICE_RESUMED){ei_device_start_emulating(d,++sequence);printf("{\"resumed\":%u}\n",generation);}
            if(type==EI_EVENT_DEVICE_REMOVED){
                for(unsigned i=0;i<32;i++){if(touches[i])ei_touch_unref(touches[i]);touches[i]=NULL;}
                if(d==device){ei_device_unref(device);device=NULL;}printf("{\"removed\":%u}\n",generation);
            }
            if(type==EI_EVENT_DISCONNECT){puts("{\"disconnected\":true}");ei_event_unref(event);goto done;}
            ei_event_unref(event);
        }
        if(poll(fds,2,10000)<0)return 6;
        if(fds[1].revents&(POLLIN|POLLHUP)){
            ssize_t n=read(control,buffer+filled,sizeof(buffer)-1-filled);if(n<=0)break;
            filled+=(size_t)n;buffer[filled]=0;char*newline;
            while((newline=memchr(buffer,'\n',filled))){
                *newline=0;unsigned id;double x,y;bool send_frame=true;struct ei_region*r=device?ei_device_get_region(device,0):NULL;
                if(sscanf(buffer,"down %u %lf %lf",&id,&x,&y)==3&&r&&id<32&&!touches[id]){
                    touches[id]=ei_device_touch_new(device);if(!touches[id])return 7;
                    ei_touch_down(touches[id],ei_region_get_x(r)+x,ei_region_get_y(r)+y);
                }else if(sscanf(buffer,"motion %u %lf %lf",&id,&x,&y)==3&&r&&id<32&&touches[id]){
                    ei_touch_motion(touches[id],ei_region_get_x(r)+x,ei_region_get_y(r)+y);
                }else if(sscanf(buffer,"up %u",&id)==1&&device&&id<32&&touches[id]){
                    ei_touch_up(touches[id]);ei_touch_unref(touches[id]);touches[id]=NULL;
                }else if(!strcmp(buffer,"stop")&&device){ei_device_stop_emulating(device);send_frame=false;}
                else return 8;
                if(device&&send_frame)ei_device_frame(device,now());
                puts("{\"sent\":true}");size_t consumed=(size_t)(newline-buffer)+1;
                memmove(buffer,buffer+consumed,filled-consumed);filled-=consumed;
            }
            if(filled==sizeof(buffer)-1)return 9;
        }
    }
done:
    for(unsigned i=0;i<32;i++)if(touches[i])ei_touch_unref(touches[i]);
    if(device)ei_device_unref(device);
    if(seat)ei_seat_unref(seat);
    ei_unref(context);return 0;
}

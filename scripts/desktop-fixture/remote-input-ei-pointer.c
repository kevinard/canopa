/* Real libei sender for private keyboard/pointer lifecycle acceptance. */
#define _GNU_SOURCE
#include <libei.h>
#include <poll.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

static uint64_t now(void) {
    struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t);
    return (uint64_t)t.tv_sec * 1000000 + t.tv_nsec / 1000;
}
static void string(const char *s) {
    putchar('"');
    for (; *s; s++) {
        unsigned char c = (unsigned char)*s;
        if (c == '"' || c == '\\') printf("\\%c", c);
        else if (c < 32) printf("\\u%04x", c);
        else putchar(c);
    }
    putchar('"');
}
int main(int argc, char **argv) {
    if (argc != 4) return 2;
    int control = atoi(argv[1]), grants = atoi(argv[2]), mapped = atoi(argv[3]);
    setvbuf(stdout, NULL, _IOLBF, 0);
    struct ei *context = ei_new_sender(NULL);
    ei_configure_name(context, "Canopa private pointer client");
    if (ei_setup_backend_fd(context, STDIN_FILENO)) return 3;
    struct pollfd fds[2] = {{.fd=ei_get_fd(context),.events=POLLIN}, {.fd=control,.events=POLLIN}};
    struct ei_device *pointer = NULL, *keyboard = NULL;
    struct ei_seat *seat = NULL;
    unsigned generation = 0, sequence = 0;
    char buffer[256]; size_t filled = 0;
    for (;;) {
        ei_dispatch(context);
        struct ei_event *event;
        while ((event = ei_get_event(context))) {
            enum ei_event_type type = ei_event_get_type(event);
            struct ei_device *device = ei_event_get_device(event);
            if (type == EI_EVENT_SEAT_ADDED) {
                seat = ei_seat_ref(ei_event_get_seat(event));
                if (grants == 1) ei_seat_bind_capabilities(seat, EI_DEVICE_CAP_KEYBOARD, NULL);
                else if (grants == 3 && mapped) ei_seat_bind_capabilities(seat, EI_DEVICE_CAP_KEYBOARD, EI_DEVICE_CAP_POINTER, EI_DEVICE_CAP_POINTER_ABSOLUTE, EI_DEVICE_CAP_BUTTON, EI_DEVICE_CAP_SCROLL, NULL);
                else if (grants == 3) ei_seat_bind_capabilities(seat, EI_DEVICE_CAP_KEYBOARD, EI_DEVICE_CAP_POINTER, EI_DEVICE_CAP_BUTTON, EI_DEVICE_CAP_SCROLL, NULL);
                else if (mapped) ei_seat_bind_capabilities(seat, EI_DEVICE_CAP_POINTER, EI_DEVICE_CAP_POINTER_ABSOLUTE, EI_DEVICE_CAP_BUTTON, EI_DEVICE_CAP_SCROLL, NULL);
                else ei_seat_bind_capabilities(seat, EI_DEVICE_CAP_POINTER, EI_DEVICE_CAP_BUTTON, EI_DEVICE_CAP_SCROLL, NULL);
            }
            if (type == EI_EVENT_DEVICE_ADDED) {
                bool key = ei_device_has_capability(device, EI_DEVICE_CAP_KEYBOARD);
                if (ei_device_has_capability(device, EI_DEVICE_CAP_TOUCH) || ei_device_has_capability(device, EI_DEVICE_CAP_TEXT)) return 4;
                if (key) {
                    if (keyboard || ei_device_has_capability(device, EI_DEVICE_CAP_POINTER) || !(grants & 1) || !ei_device_keyboard_get_keymap(device)) return 5;
                    keyboard = ei_device_ref(device);
                    puts("{\"added\":\"keyboard\"}");
                } else {
                    if (pointer || !(grants & 2) || !ei_device_has_capability(device, EI_DEVICE_CAP_POINTER) || !ei_device_has_capability(device, EI_DEVICE_CAP_BUTTON) || !ei_device_has_capability(device, EI_DEVICE_CAP_SCROLL) || ei_device_has_capability(device, EI_DEVICE_CAP_POINTER_ABSOLUTE) != !!mapped) return 6;
                    pointer = ei_device_ref(device); generation++;
                    printf("{\"added\":\"pointer\",\"generation\":%u,\"regions\":[",generation);
                    for (size_t i = 0; ; i++) {
                        struct ei_region *r = ei_device_get_region(device,i); if (!r) break;
                        const char *id = ei_region_get_mapping_id(r); if (!id) return 7;
                        if (i) putchar(',');
                        printf("{\"id\":"); string(id);
                        printf(",\"offset\":[%u,%u],\"size\":[%u,%u],\"scale\":%.8g}",ei_region_get_x(r),ei_region_get_y(r),ei_region_get_width(r),ei_region_get_height(r),ei_region_get_physical_scale(r));
                    }
                    puts("]}");
                }
            }
            if (type == EI_EVENT_DEVICE_RESUMED) {
                ei_device_start_emulating(device,++sequence);
                printf("{\"resumed\":\"%s\",\"generation\":%u}\n",device == keyboard ? "keyboard":"pointer",generation);
            }
            if (type == EI_EVENT_DEVICE_REMOVED) {
                bool key = device == keyboard;
                printf("{\"removed\":\"%s\",\"generation\":%u}\n",key ? "keyboard":"pointer",generation);
                if (key) { ei_device_unref(keyboard); keyboard=NULL; }
                else if (device == pointer) { ei_device_unref(pointer); pointer=NULL; }
            }
            if (type == EI_EVENT_DISCONNECT) { puts("{\"disconnected\":true}"); ei_event_unref(event); goto done; }
            ei_event_unref(event);
        }
        if (poll(fds,2,10000) < 0) return 8;
        if (fds[1].revents & (POLLIN|POLLHUP)) {
            ssize_t n = read(control,buffer+filled,sizeof(buffer)-1-filled);
            if (n <= 0) break;
            filled += (size_t)n; buffer[filled] = 0;
            char *newline;
            while ((newline = memchr(buffer,'\n',filled))) {
                *newline = 0;
                double x,y; unsigned code,state,index;
                struct ei_device *frame = pointer;
                if (sscanf(buffer,"relative %lf %lf",&x,&y)==2 && pointer) ei_device_pointer_motion(pointer,x,y);
                else if (sscanf(buffer,"absolute %u %lf %lf",&index,&x,&y)==3 && pointer) {
                    struct ei_region *r = ei_device_get_region(pointer,index); if (!r) return 9;
                    ei_device_pointer_motion_absolute(pointer,ei_region_get_x(r)+x,ei_region_get_y(r)+y);
                } else if (sscanf(buffer,"button %u %u",&code,&state)==2 && pointer) ei_device_button_button(pointer,code,!!state);
                else if (sscanf(buffer,"key %u %u",&code,&state)==2 && keyboard) { ei_device_keyboard_key(keyboard,code,!!state); frame=keyboard; }
                else if (sscanf(buffer,"scroll %lf %lf",&x,&y)==2 && pointer) ei_device_scroll_delta(pointer,x,y);
                else if (sscanf(buffer,"wheel %lf %lf",&x,&y)==2 && pointer) ei_device_scroll_discrete(pointer,(int32_t)x,(int32_t)y);
                else if (sscanf(buffer,"stop %u %u",&code,&state)==2 && pointer) ei_device_scroll_stop(pointer,!!code,!!state);
                else if (sscanf(buffer,"cancel %u %u",&code,&state)==2 && pointer) ei_device_scroll_cancel(pointer,!!code,!!state);
                else if (!strcmp(buffer,"restart") && pointer) { ei_device_stop_emulating(pointer); ei_device_start_emulating(pointer,++sequence); frame=NULL; }
                else if (!strcmp(buffer,"close") && pointer) { ei_device_close(pointer); frame=NULL; }
                else if (!strcmp(buffer,"request") && seat) {
                    if (mapped) ei_seat_request_device_with_capabilities(seat,EI_DEVICE_CAP_POINTER,EI_DEVICE_CAP_POINTER_ABSOLUTE,EI_DEVICE_CAP_BUTTON,EI_DEVICE_CAP_SCROLL,NULL);
                    else ei_seat_request_device_with_capabilities(seat,EI_DEVICE_CAP_POINTER,EI_DEVICE_CAP_BUTTON,EI_DEVICE_CAP_SCROLL,NULL);
                    frame=NULL;
                } else if (!strcmp(buffer,"flood") && pointer) {
                    for (int i=0;i<514;i++) ei_device_pointer_motion(pointer,1.,1.);
                } else return 10;
                if (frame) ei_device_frame(frame,now());
                printf("{\"sent\":"); string(buffer); puts("}");
                size_t consumed = (size_t)(newline-buffer)+1;
                memmove(buffer,buffer+consumed,filled-consumed); filled-=consumed;
            }
            if (filled == sizeof(buffer)-1) return 11;
        }
    }
done:
    if (pointer) ei_device_unref(pointer);
    if (keyboard) ei_device_unref(keyboard);
    if (seat) ei_seat_unref(seat);
    ei_unref(context);
    return 0;
}

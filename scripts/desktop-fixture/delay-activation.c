#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

/* Private native test shim: pause token delivery in Wayland's diagnostic print,
 * after the real reply arrives and before it reaches the client dispatcher.
 * All output and protocol messages are forwarded unchanged. */
static int (*original_vfprintf)(FILE *, const char *, va_list);
static pthread_once_t initialized = PTHREAD_ONCE_INIT;
static pthread_mutex_t records_lock = PTHREAD_MUTEX_INITIALIZER;
static struct { unsigned token, source; } records[256];
static atomic_int delayed;
static _Thread_local unsigned setting_surface;
static _Thread_local int inspecting;

static void initialize(void) {
    original_vfprintf = (int (*)(FILE *, const char *, va_list))dlsym(RTLD_NEXT, "vfprintf");
    if (!original_vfprintf) _exit(127);
}

static void observe(const char *chunk) {
    const char *method = strstr(chunk, "xdg_activation_token_v1#");
    unsigned token = 0;
    int done = 0;
    if (method) {
        char *end;
        token = (unsigned)strtoul(method + strlen("xdg_activation_token_v1#"), &end, 10);
        setting_surface = strstr(end, ".set_surface(") ? token : 0;
        done = strncmp(end, ".done(", 6) == 0;
    }
    const char *surface = strstr(chunk, "wl_surface#");
    if (setting_surface && surface) {
        unsigned source = (unsigned)strtoul(surface + strlen("wl_surface#"), NULL, 10);
        pthread_mutex_lock(&records_lock);
        unsigned slot = setting_surface % 256;
        records[slot].token = setting_surface;
        records[slot].source = source;
        pthread_mutex_unlock(&records_lock);
        setting_surface = 0;
    }
    if (!done || atomic_load(&delayed)) return;
    const char *path = getenv("CANOPA_FOCUS_DELAY_CONTROL");
    if (!path) return;
    char value[32] = {0};
    int fd = open(path, O_RDONLY | O_CLOEXEC);
    if (fd < 0) return;
    ssize_t size = read(fd, value, sizeof(value) - 1);
    close(fd);
    if (size <= 0) return;
    unsigned expected = (unsigned)strtoul(value, NULL, 10);
    pthread_mutex_lock(&records_lock);
    unsigned slot = token % 256;
    int matches = expected && records[slot].token == token && records[slot].source == expected;
    pthread_mutex_unlock(&records_lock);
    if (!matches || atomic_exchange(&delayed, 1)) return;
    static const char marker[] = "\n[files-focus-expiry-fixture] delaying requesting-surface token reply 1200ms\n";
    (void)write(STDERR_FILENO, marker, sizeof(marker) - 1);
    struct timespec delay = {1, 200000000};
    while (nanosleep(&delay, &delay) < 0 && errno == EINTR) {}
}

static int forward(FILE *stream, const char *format, va_list args) {
    pthread_once(&initialized, initialize);
    va_list copy;
    va_copy(copy, args);
    int result = original_vfprintf(stream, format, args);
    if (!inspecting && stream == stderr) {
        inspecting = 1;
        char chunk[4096];
        (void)vsnprintf(chunk, sizeof(chunk), format, copy);
        observe(chunk);
        inspecting = 0;
    }
    va_end(copy);
    return result;
}

int vfprintf(FILE *stream, const char *format, va_list args) {
    return forward(stream, format, args);
}
int fprintf(FILE *stream, const char *format, ...) {
    va_list args;
    va_start(args, format);
    int result = forward(stream, format, args);
    va_end(args);
    return result;
}
int __fprintf_chk(FILE *stream, int flag, const char *format, ...) {
    (void)flag;
    va_list args;
    va_start(args, format);
    int result = forward(stream, format, args);
    va_end(args);
    return result;
}

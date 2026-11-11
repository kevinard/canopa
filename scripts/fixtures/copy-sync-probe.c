// LD_PRELOAD probe for scripts/measure-files-copy.py: counts and times the
// durability and namespace calls a copy makes, then writes one JSON line to
// $COPY_SYNC_PROBE_OUT at exit. Build: cc -O2 -shared -fPIC -o probe.so probe.c -ldl
#define _GNU_SOURCE
#include <dlfcn.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

enum { FSYNC, FDATASYNC, SYNCFS, SYNC_FILE_RANGE, RENAMEAT2, OPENAT, PWRITE, COUNT };
static const char *names[COUNT] = {"fsync", "fdatasync", "syncfs", "sync_file_range",
                                   "renameat2", "openat", "pwrite64"};
static _Atomic unsigned long calls[COUNT];
static _Atomic unsigned long nanos[COUNT];

static unsigned long now(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (unsigned long)t.tv_sec * 1000000000ul + (unsigned long)t.tv_nsec;
}

#define TIMED(kind, type, call)                                                        \
    unsigned long start = now();                                                       \
    type result = call;                                                                \
    atomic_fetch_add(&nanos[kind], now() - start);                                     \
    atomic_fetch_add(&calls[kind], 1);                                                 \
    return result;

#define REAL(name, type, ...)                                                          \
    static type (*real)(__VA_ARGS__);                                                  \
    if (!real) real = (type(*)(__VA_ARGS__))dlsym(RTLD_NEXT, name);

int fsync(int fd) {
    REAL("fsync", int, int);
    TIMED(FSYNC, int, real(fd));
}
int fdatasync(int fd) {
    REAL("fdatasync", int, int);
    TIMED(FDATASYNC, int, real(fd));
}
int syncfs(int fd) {
    REAL("syncfs", int, int);
    TIMED(SYNCFS, int, real(fd));
}
int sync_file_range(int fd, off_t offset, off_t count, unsigned int flags) {
    REAL("sync_file_range", int, int, off_t, off_t, unsigned int);
    TIMED(SYNC_FILE_RANGE, int, real(fd, offset, count, flags));
}
int renameat2(int a, const char *b, int c, const char *d, unsigned int e) {
    REAL("renameat2", int, int, const char *, int, const char *, unsigned int);
    TIMED(RENAMEAT2, int, real(a, b, c, d, e));
}
ssize_t pwrite64(int fd, const void *buffer, size_t count, off_t offset) {
    REAL("pwrite64", ssize_t, int, const void *, size_t, off_t);
    TIMED(PWRITE, ssize_t, real(fd, buffer, count, offset));
}
int openat64(int dir, const char *path, int flags, ...) {
    REAL("openat64", int, int, const char *, int, ...);
    mode_t mode = 0;
    if (flags & (O_CREAT | __O_TMPFILE)) {
        va_list list;
        va_start(list, flags);
        mode = va_arg(list, mode_t);
        va_end(list);
    }
    TIMED(OPENAT, int, real(dir, path, flags, mode));
}

__attribute__((destructor)) static void report(void) {
    const char *path = getenv("COPY_SYNC_PROBE_OUT");
    if (!path) return;
    FILE *out = fopen(path, "a");
    if (!out) return;
    fputc('{', out);
    for (int i = 0; i < COUNT; i++)
        fprintf(out, "%s\"%s\":[%lu,%lu]", i ? "," : "", names[i], atomic_load(&calls[i]),
                atomic_load(&nanos[i]) / 1000);
    fputs("}\n", out);
    fclose(out);
}

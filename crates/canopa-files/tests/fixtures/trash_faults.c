/* Private process fixture: fail one exact fsync or pause the next armed worker.
 * Never loaded by the application or the parent test process. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <unistd.h>

static const char *private_root(const char *expected_mode) {
    const char *root = getenv("CANOPA_FILES_TRASH_FAULT_ROOT");
    const char *mode = getenv("CANOPA_FILES_TRASH_FAULT_MODE");
    return root && mode && !strcmp(mode, expected_mode) &&
        !strncmp(root, "/tmp/canopa-files-trash-fault-", sizeof("/tmp/canopa-files-trash-fault-") - 1) ? root : NULL;
}

int fsync(int descriptor) {
    int (*real_fsync)(int) = dlsym(RTLD_NEXT, "fsync");
    const char *root = private_root("recover");
    char armed[PATH_MAX];
    if (root) {
        int size = snprintf(armed, sizeof(armed), "%s/fsync-arm", root);
        if (size < 0 || size >= (int)sizeof(armed) || access(armed, F_OK) != 0) root = NULL;
    }
    if (root) {
        char link[64], target[PATH_MAX], expected[PATH_MAX], marker[PATH_MAX];
        snprintf(link, sizeof(link), "/proc/self/fd/%d", descriptor);
        ssize_t length = readlink(link, target, sizeof(target) - 1);
        if (length >= 0 && length < (ssize_t)sizeof(target)) {
            target[length] = 0;
            int size = snprintf(expected, sizeof(expected), "%s/source", root);
            if (size > 0 && size < (int)sizeof(expected) && !strcmp(target, expected)) {
                size = snprintf(marker, sizeof(marker), "%s/fsync-fault-fired", root);
                if (size > 0 && size < (int)sizeof(marker)) {
                    int file = open(marker, O_CREAT | O_EXCL | O_WRONLY | O_CLOEXEC | O_NOFOLLOW, 0600);
                    if (file >= 0) {
                        close(file);
                        errno = EIO;
                        return -1;
                    }
                }
            }
        }
    }
    return real_fsync ? real_fsync(descriptor) : syscall(SYS_fsync, descriptor);
}

struct paused_start {
    void *(*start)(void *);
    void *argument;
    char root[PATH_MAX];
};

static void *paused_worker(void *value) {
    struct paused_start *paused = value;
    char path[PATH_MAX];
    snprintf(path, sizeof(path), "%s/dispatch-started", paused->root);
    int marker = open(path, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW, 0600);
    if (marker < 0) _exit(90);
    close(marker);
    snprintf(path, sizeof(path), "%s/dispatch-release", paused->root);
    int attempts = 0;
    while (access(path, F_OK) != 0) {
        if (++attempts >= 10000) _exit(91);
        usleep(1000);
    }
    void *(*start)(void *) = paused->start;
    void *argument = paused->argument;
    free(paused);
    return start(argument);
}

int pthread_create(pthread_t *thread, const pthread_attr_t *attributes,
                   void *(*start)(void *), void *argument) {
    int (*real_create)(pthread_t *, const pthread_attr_t *, void *(*)(void *), void *) =
        dlsym(RTLD_NEXT, "pthread_create");
    if (!real_create) return EAGAIN;
    const char *root = private_root("dispatch");
    if (root && strlen(root) < PATH_MAX - 32) {
        char path[PATH_MAX];
        snprintf(path, sizeof(path), "%s/dispatch-arm", root);
        if (access(path, F_OK) == 0) {
            snprintf(path, sizeof(path), "%s/dispatch-claimed", root);
            int marker = open(path, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW, 0600);
            if (marker >= 0) {
                close(marker);
                struct paused_start *paused = malloc(sizeof(*paused));
                if (!paused) return ENOMEM;
                paused->start = start;
                paused->argument = argument;
                strcpy(paused->root, root);
                int result = real_create(thread, attributes, paused_worker, paused);
                if (result) free(paused);
                return result;
            }
        }
    }
    return real_create(thread, attributes, start, argument);
}

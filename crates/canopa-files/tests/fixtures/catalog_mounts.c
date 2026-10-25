#define _GNU_SOURCE
#include <dlfcn.h>
#include <fcntl.h>
#include <limits.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

/* Only this test's private child sees a synthetic mount inventory. All payloads,
 * file monitors and engine operations remain real, under its private /tmp root. */
static int opened(const char *symbol, const char *path, int flags, mode_t mode)
{
    int (*real_open)(const char *, int, ...) = dlsym(RTLD_NEXT, symbol);
    if (!real_open) _exit(125);
    const char *root = getenv("CANOPA_FILES_CATALOG_FIXTURE");
    const char prefix[] = "/tmp/canopa-files-trash-catalog-";
    char inventory[PATH_MAX];
    char marker[PATH_MAX];
    if (root && strncmp(root, prefix, sizeof(prefix) - 1) == 0 &&
        !strchr(root + sizeof(prefix) - 1, '/') &&
        strcmp(path, "/proc/self/mountinfo") == 0 &&
        snprintf(inventory, sizeof(inventory), "%s/mountinfo", root) < (int)sizeof(inventory) &&
        snprintf(marker, sizeof(marker), "%s/mountinfo-read", root) < (int)sizeof(marker)) {
        int descriptor = real_open(marker, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW, 0600);
        if (descriptor >= 0) close(descriptor);
        return real_open(inventory, flags, mode);
    }
    return real_open(path, flags, mode);
}

#define OPEN_WRAPPER(name) \
int name(const char *path, int flags, ...) \
{ \
    mode_t mode = 0; \
    if ((flags & O_CREAT) || (flags & O_TMPFILE) == O_TMPFILE) { \
        va_list arguments; va_start(arguments, flags); \
        mode = (mode_t)va_arg(arguments, int); va_end(arguments); \
    } \
    return opened(#name, path, flags, mode); \
}

OPEN_WRAPPER(open)
OPEN_WRAPPER(open64)

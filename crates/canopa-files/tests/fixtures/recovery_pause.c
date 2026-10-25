/* Private child-only fixture: pause after actual Recovery output installation. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <fcntl.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

int fsync(int descriptor) {
    int (*real_fsync)(int) = dlsym(RTLD_NEXT, "fsync");
    if (!real_fsync) _exit(90);
    int result = real_fsync(descriptor);
    const char *root = getenv("CANOPA_FILES_RECOVERY_PAUSE_ROOT");
    const char prefix[] = "/tmp/canopa-files-recovery-runtime-";
    if (result || !root || strncmp(root, prefix, sizeof(prefix)-1) ||
        strchr(root+sizeof(prefix)-1, '/') || strlen(root) > PATH_MAX-64) return result;
    char link[64], target[PATH_MAX], expected[PATH_MAX], path[PATH_MAX];
    snprintf(link, sizeof(link), "/proc/self/fd/%d", descriptor);
    ssize_t length = readlink(link, target, sizeof(target)-1);
    if (length < 0) return result;
    target[length] = 0;
    snprintf(expected, sizeof(expected), "%s/output/recovered", root);
    if (strcmp(target, expected)) return result;
    snprintf(path, sizeof(path), "%s/output/recovered/a.txt", root);
    if (access(path, F_OK)) return result;
    snprintf(path, sizeof(path), "%s/copy-paused", root);
    int marker = open(path, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW, 0600);
    if (marker < 0) return result;
    close(marker);
    snprintf(path, sizeof(path), "%s/copy-release", root);
    for (int attempt=0; access(path,F_OK); attempt++) {
        if (attempt >= 10000) _exit(91);
        usleep(1000);
    }
    return result;
}

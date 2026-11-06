/* Synthetic NSS enumeration for tests. Only the isolated Canopa helper is affected. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <pwd.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static int enabled(void) {
    char cmd[4096];
    int fd = open("/proc/self/cmdline", O_RDONLY);
    if (fd < 0) return 0;
    ssize_t n = read(fd, cmd, sizeof(cmd));
    close(fd);
    for (ssize_t i = 0; i < n;) {
        size_t len = strnlen(cmd + i, (size_t)(n - i));
        if (len == strlen("internal-nss-users") &&
            !memcmp(cmd + i, "internal-nss-users", len)) return 1;
        i += (ssize_t)len + 1;
    }
    return 0;
}
static unsigned index_;
static const char *mode(void) {
    const char *s = getenv("CANOPA_NSS_FIXTURE");
    return s ? s : "ok";
}
static void event(const char *name) {
    const char *path = getenv("CANOPA_NSS_EVENTS");
    if (!path) return;
    FILE *f = fopen(path, "a");
    if (!f) _exit(91);
    fprintf(f, "%ld %s\n", (long)getpid(), name);
    fclose(f);
}
void setpwent(void) {
    if (!enabled()) {
        void (*next)(void) = dlsym(RTLD_NEXT, "setpwent"); next(); return;
    }
    if (getenv("GREETD_SOCK") || getenv("NOTIFY_SOCKET")) _exit(92);
    index_ = 0;
    event("begin");
    if (!strcmp(mode(), "overflow")) {
        char bytes[4096]; memset(bytes, 'x', sizeof(bytes));
        for (int i = 0; i < 32; i++) {
            if (write(STDOUT_FILENO, bytes, sizeof(bytes)) < 0) _exit(93);
        }
    }
    if (!strcmp(mode(), "fork")) {
        pid_t child = fork();
        if (child == 0) { event("descendant"); sleep(60); _exit(0); }
        if (child < 0) _exit(94);
    }
    if (!strcmp(mode(), "block") || !strcmp(mode(), "fork") ||
        !strcmp(mode(), "overflow")) sleep(60);
}
void endpwent(void) {
    if (!enabled()) {
        void (*next)(void) = dlsym(RTLD_NEXT, "endpwent"); next(); return;
    }
    event("end");
}
int getpwent_r(struct passwd *p, char *buf, size_t len, struct passwd **result) {
    if (!enabled()) {
        int (*next)(struct passwd *, char *, size_t, struct passwd **) =
            dlsym(RTLD_NEXT, "getpwent_r");
        return next(p, buf, len, result);
    }
    *result = NULL;
    if (!strcmp(mode(), "error")) return EIO;
    if (!strcmp(mode(), "range")) return ERANGE;
    struct row { const char *user, *gecos, *shell; uid_t uid; };
    static const struct row rows[] = {
        {"root", "Root", "/bin/sh", 0},
        {"daemon", "Daemon", "/bin/sh", 42},
        {"alice", "Alice NSS,Room 12,555-123,555-456", "/bin/sh", 1000},
        {"bob", "Bob NSS", "", 1200},
        {"enterprise", "Enterprise NSS", "/bin/sh", 200000},
        {"disabled", "Disabled", "/usr/sbin/nologin", 1001},
        {"false-shell", "False", "/bin/false", 1002},
        {"bad user", "Invalid", "/bin/sh", 1003},
        {"bad\377", "Invalid", "/bin/sh", 1004},
        {"fallback", "Bad\nName,555-secret", "/bin/sh", 1005},
        {"alice", "Duplicate name", "/bin/sh", 1006},
        {"alias", "Duplicate UID", "/bin/sh", 1200},
    };
    char user[32];
    struct row row;
    if (!strcmp(mode(), "many")) {
        if (index_ >= 300) return ENOENT;
        snprintf(user, sizeof(user), "user%u", index_);
        row = (struct row){user, "Synthetic", "/bin/sh", 2000 + index_};
    } else if (!strcmp(mode(), "budget")) {
        if (index_ >= 5000) return ENOENT;
        row = rows[0];
    } else {
        if (index_ >= sizeof(rows) / sizeof(rows[0])) return ENOENT;
        row = rows[index_];
    }
    index_++;
    memset(p, 0, sizeof(*p));
    char *cursor = buf;
    const char *values[] = {row.user, "PASSWORD-MUST-NOT-LEAK", row.gecos,
                           "/HOME-MUST-NOT-LEAK", row.shell};
    char **fields[] = {&p->pw_name, &p->pw_passwd, &p->pw_gecos, &p->pw_dir, &p->pw_shell};
    for (size_t i = 0; i < 5; i++) {
        size_t size = strlen(values[i]) + 1;
        if (size > len) return ERANGE;
        memcpy(cursor, values[i], size); *fields[i] = cursor;
        cursor += size; len -= size;
    }
    p->pw_uid = row.uid; p->pw_gid = row.uid;
    *result = p;
    return 0;
}

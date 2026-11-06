/* Test-only read-only FUSE source with a controlled close/flush reply. */
#define FUSE_USE_VERSION 35
#include <fuse3/fuse.h>
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <time.h>
#include <unistd.h>

static char *document;
static size_t document_bytes;
static atomic_int released;
static int workers_only;
static pid_t backend_parent;

static int attributes(const char *path, struct stat *result, struct fuse_file_info *file) {
    (void)file;
    memset(result, 0, sizeof(*result));
    result->st_uid = getuid(); result->st_gid = getgid();
    if (strcmp(path, "/") == 0) {
        result->st_mode = S_IFDIR | 0700; result->st_nlink = 2;
    } else if (strcmp(path, "/source.pdf") == 0) {
        result->st_mode = S_IFREG | 0400; result->st_nlink = 1;
        result->st_size = (off_t)document_bytes;
    } else return -ENOENT;
    return 0;
}

static int open_source(const char *path, struct fuse_file_info *file) {
    if (strcmp(path, "/source.pdf") != 0) return -ENOENT;
    if ((file->flags & O_ACCMODE) != O_RDONLY) return -EACCES;
    file->direct_io = 1;
    return 0;
}

static int read_source(const char *path, char *buffer, size_t bytes, off_t offset,
                       struct fuse_file_info *file) {
    (void)path; (void)file;
    if (offset < 0) return -EINVAL;
    if ((size_t)offset >= document_bytes) return 0;
    size_t remaining = document_bytes - (size_t)offset;
    if (bytes > remaining) bytes = remaining;
    memcpy(buffer, document + offset, bytes);
    return (int)bytes;
}

static int flush_source(const char *path, struct fuse_file_info *file) {
    (void)path; (void)file;
    const struct fuse_context *context = fuse_get_context();
    if (workers_only) {
        char name[64], command[512];
        snprintf(name, sizeof(name), "/proc/%d/cmdline", context->pid);
        int descriptor = open(name, O_RDONLY | O_CLOEXEC);
        ssize_t bytes = descriptor < 0 ? -1 : read(descriptor, command, sizeof(command));
        if (descriptor >= 0) close(descriptor);
        int worker = 0;
        for (ssize_t offset = 0; bytes > 0 && offset < bytes;) {
            size_t length = strnlen(command + offset, (size_t)(bytes - offset));
            if (length == strlen("--print-worker") && memcmp(command + offset, "--print-worker", length) == 0) worker = 1;
            offset += (ssize_t)length + 1;
        }
        if (backend_parent) {
            /* cmdline can be empty during exit_files. A denied child which
             * already inherited the source must still exercise real close I/O. */
            snprintf(name, sizeof(name), "/proc/%d/stat", context->pid);
            FILE *status = fopen(name, "r");
            if (status) {
                char buffer[1024];
                if (fgets(buffer, sizeof(buffer), status)) {
                    char *fields = strrchr(buffer, ')');
                    char state; int parent;
                    if (fields && sscanf(fields + 1, " %c %d", &state, &parent) == 2 && parent == backend_parent) worker = 1;
                }
                fclose(status);
            }
        }
        if (!worker) { printf("PASS_FLUSH %d\n", context->pid); return 0; }
    }
    printf("FLUSH %d\n", context->pid);
    if (!atomic_load(&released)) {
        /* Parent-owned stdin releases the obstruction. EOF also releases it;
         * parent death kills this server, closing the FUSE connection. */
        struct pollfd control = { .fd = STDIN_FILENO, .events = POLLIN | POLLHUP };
        struct timespec started, now;
        clock_gettime(CLOCK_MONOTONIC, &started);
        int deadline = 0;
        while (!atomic_load(&released)) {
            int ready = poll(&control, 1, 100);
            if (ready < 0 && errno != EINTR) return -errno;
            if (ready > 0 && (control.revents & (POLLIN | POLLHUP))) {
                char command;
                ssize_t bytes = read(STDIN_FILENO, &command, 1);
                if (bytes >= 0) atomic_store(&released, 1);
                else if (errno != EAGAIN && errno != EINTR) return -errno;
            }
            clock_gettime(CLOCK_MONOTONIC, &now);
            if (now.tv_sec - started.tv_sec >= 300) { deadline = 1; atomic_store(&released, 1); }
        }
        printf("RELEASED %d %s\n", context->pid, deadline ? "deadline" : "controller");
    }
    return 0;
}

static int release_source(const char *path, struct fuse_file_info *file) {
    (void)path; (void)file;
    return 0;
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    if ((argc < 3 || argc > 5) || strncmp(argv[1], "/tmp/canopa-print-fuse-", 23) != 0) return 2;
    workers_only = argc >= 4 && strcmp(argv[3], "workers") == 0;
    if ((argc >= 4 && !workers_only) || fcntl(STDIN_FILENO, F_SETFL, O_NONBLOCK) != 0) return 2;
    if (argc == 5) {
        char *end;
        long parent = strtol(argv[4], &end, 10);
        if (*end || parent <= 1 || parent > 2147483647) return 2;
        backend_parent = (pid_t)parent;
    }
    pid_t parent = getppid();
    if (prctl(PR_SET_PDEATHSIG, SIGKILL) != 0 || getppid() != parent) return 2;
    FILE *source = fopen(argv[2], "rb");
    if (!source || fseek(source, 0, SEEK_END) != 0) return 2;
    long length = ftell(source);
    if (length <= 0 || length > 1024 * 1024 || fseek(source, 0, SEEK_SET) != 0) return 2;
    document_bytes = (size_t)length;
    document = malloc(document_bytes);
    if (!document || fread(document, 1, document_bytes, source) != document_bytes) return 2;
    fclose(source);
    const struct fuse_operations operations = {
        .getattr = attributes, .open = open_source, .read = read_source,
        .flush = flush_source, .release = release_source,
    };
    char *arguments[8] = { argv[0], argv[1], "-f" };
    int count = 3;
    if (!workers_only) arguments[count++] = "-s";
    arguments[count++] = "-o";
    arguments[count++] = "attr_timeout=0,entry_timeout=0,negative_timeout=0";
    arguments[count] = NULL;
    int result = fuse_main(count, arguments, &operations, NULL);
    free(document);
    return result;
}

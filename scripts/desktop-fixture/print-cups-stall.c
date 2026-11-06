/* Private native failure fixture. Forward the public CUPS API unchanged except
 * for a delayed FinishDestDocument; the Rust client must retire the stalled
 * worker and cancel its owned partial job through a second worker.
 * Build as libcups.so.2 in a private LD_LIBRARY_PATH used only by the fixture.
 */
#define _GNU_SOURCE
#include <cups/cups.h>
#include <cups/http.h>
#include <dlfcn.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

static void *real_cups(void) {
    static void *library;
    if (!library) library = dlopen("/usr/lib/libcups.so.2", RTLD_NOW | RTLD_LOCAL);
    if (!library) abort();
    return library;
}
#define FWD(type, name, parameters, arguments) \
    type name parameters { \
        __typeof__(&name) function = (__typeof__(&name))dlsym(real_cups(), #name); \
        if (!function) abort(); \
        return function arguments; \
    }
#define FWD_VOID(name, parameters, arguments) \
    void name parameters { \
        __typeof__(&name) function = (__typeof__(&name))dlsym(real_cups(), #name); \
        if (!function) abort(); \
        function arguments; \
    }
FWD(const char *, cupsServer, (void), ())
FWD(const char *, cupsUser, (void), ())
FWD(int, ippPort, (void), ())
FWD(http_encryption_t, cupsEncryption, (void), ())
FWD(http_t *, httpConnect2, (const char *host, int port, http_addrlist_t *addresses, int family, http_encryption_t encryption, int blocking, int milliseconds, int *cancel), (host, port, addresses, family, encryption, blocking, milliseconds, cancel))
FWD_VOID(httpSetTimeout, (http_t *http, double timeout, http_timeout_cb_t callback, void *data), (http, timeout, callback, data))
FWD(http_status_t, httpGetStatus, (http_t *http), (http))
FWD_VOID(httpClose, (http_t *http), (http))
FWD(cups_dest_t *, cupsGetNamedDest, (http_t *http, const char *name, const char *instance), (http, name, instance))
FWD(int, cupsGetDests2, (http_t *http, cups_dest_t **destinations), (http, destinations))
FWD(const char *, cupsGetOption, (const char *name, int count, cups_option_t *options), (name, count, options))
FWD(ipp_status_t, cupsLastError, (void), ())
FWD_VOID(cupsFreeDests, (int count, cups_dest_t *destinations), (count, destinations))
FWD(cups_dinfo_t *, cupsCopyDestInfo, (http_t *http, cups_dest_t *destination), (http, destination))
FWD_VOID(cupsFreeDestInfo, (cups_dinfo_t *info), (info))
FWD(int, cupsCheckDestSupported, (http_t *http, cups_dest_t *destination, cups_dinfo_t *info, const char *option, const char *value), (http, destination, info, option, value))
FWD(ipp_attribute_t *, cupsFindDestSupported, (http_t *http, cups_dest_t *destination, cups_dinfo_t *info, const char *option), (http, destination, info, option))
FWD(ipp_attribute_t *, cupsFindDestDefault, (http_t *http, cups_dest_t *destination, cups_dinfo_t *info, const char *option), (http, destination, info, option))
FWD(int, cupsGetDestMediaByName, (http_t *http, cups_dest_t *destination, cups_dinfo_t *info, const char *media, unsigned flags, cups_size_t *size), (http, destination, info, media, flags, size))
FWD(ipp_tag_t, ippGetValueTag, (ipp_attribute_t *attribute), (attribute))
FWD(int, ippGetCount, (ipp_attribute_t *attribute), (attribute))
FWD(const char *, ippGetString, (ipp_attribute_t *attribute, int element, const char **language), (attribute, element, language))
FWD(int, ippGetRange, (ipp_attribute_t *attribute, int element, int *upper), (attribute, element, upper))
FWD(ipp_status_t, cupsCreateDestJob, (http_t *http, cups_dest_t *destination, cups_dinfo_t *info, int *job, const char *title, int count, cups_option_t *options), (http, destination, info, job, title, count, options))
FWD(http_status_t, cupsStartDestDocument, (http_t *http, cups_dest_t *destination, cups_dinfo_t *info, int job, const char *name, const char *format, int count, cups_option_t *options, int last), (http, destination, info, job, name, format, count, options, last))
FWD(http_status_t, cupsWriteRequestData, (http_t *http, const char *data, size_t length), (http, data, length))
FWD(ipp_status_t, cupsCancelDestJob, (http_t *http, cups_dest_t *destination, int job), (http, destination, job))
FWD(int, cupsGetJobs2, (http_t *http, cups_job_t **jobs, const char *name, int mine, int which), (http, jobs, name, mine, which))
FWD_VOID(cupsFreeJobs, (int count, cups_job_t *jobs), (count, jobs))

ipp_status_t cupsFinishDestDocument(http_t *http, cups_dest_t *destination, cups_dinfo_t *info) {
    const char *marker = getenv("CANOPA_PRINT_FIXTURE_FINISH_MARKER");
    if (!marker) abort();
    int fd = open(marker, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0600);
    if (fd < 0) abort();
    char text[64];
    int length = snprintf(text, sizeof(text), "%ld\n", (long)getpid());
    if (write(fd, text, (size_t)length) != length) abort();
    close(fd);
    sleep(30);
    __typeof__(&cupsFinishDestDocument) function = (__typeof__(&cupsFinishDestDocument))dlsym(real_cups(), "cupsFinishDestDocument");
    if (!function) abort();
    return function(http, destination, info);
}

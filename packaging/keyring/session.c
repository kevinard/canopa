#define _GNU_SOURCE
#include "session.h"
#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <systemd/sd-login.h>
#include <unistd.h>

static GVariant *manager_property(GDBusConnection *bus, const char *path,
                                  const char *interface, const char *property,
                                  const GVariantType *type)
{
    g_autoptr(GVariant) reply = g_dbus_connection_call_sync(
        bus, "org.freedesktop.systemd1", path, "org.freedesktop.DBus.Properties",
        "Get", g_variant_new("(ss)", interface, property), G_VARIANT_TYPE("(v)"),
        G_DBUS_CALL_FLAGS_NO_AUTO_START, 1000, NULL, NULL);
    if (!reply)
        return NULL;
    GVariant *value;
    g_variant_get(reply, "(v)", &value);
    if (g_variant_is_of_type(value, type))
        return value;
    g_variant_unref(value);
    return NULL;
}

char **user_manager_environment(GDBusConnection *bus)
{
    g_autoptr(GVariant) value = manager_property(
        bus, "/org/freedesktop/systemd1", "org.freedesktop.systemd1.Manager",
        "Environment", G_VARIANT_TYPE_STRING_ARRAY);
    return value ? g_variant_dup_strv(value, NULL) : NULL;
}

static gboolean session_is_canopa(char **environment)
{
    const char *pid_string = g_environ_getenv(environment, "CANOPA_SESSION_PID");
    if (!pid_string)
        return FALSE;
    gint64 pid;
    if (!g_ascii_string_to_signed(pid_string, 10, 1, G_MAXINT, &pid, NULL))
        return FALSE;
    g_autofree char *session = NULL;
    g_autofree char *display_session = NULL;
    g_autofree char *type = NULL;
    return sd_pid_get_session((pid_t)pid, &session) >= 0 &&
           sd_uid_get_display(getuid(), &display_session) >= 0 &&
           strcmp(session, display_session) == 0 &&
           sd_session_is_active(session) > 0 &&
           sd_session_get_type(session, &type) >= 0 &&
           strcmp(type, "wayland") == 0;
}

int connect_canopa_display(GDBusConnection *bus, char **environment)
{
    if (!environment || !session_is_canopa(environment))
        return -1;

    const char *display = g_environ_getenv(environment, "WAYLAND_DISPLAY");
    if (!display || !*display)
        return -1;
    g_autofree char *path = g_path_is_absolute(display)
        ? g_strdup(display) : g_build_filename(g_get_user_runtime_dir(), display, NULL);
    struct sockaddr_un address = { .sun_family = AF_UNIX };
    if (g_strlcpy(address.sun_path, path, sizeof(address.sun_path)) >= sizeof(address.sun_path))
        return -1;

    g_autoptr(GVariant) main_pid = manager_property(
        bus, "/org/freedesktop/systemd1/unit/canopa_2dcompositor_2eservice",
        "org.freedesktop.systemd1.Service", "MainPID", G_VARIANT_TYPE_UINT32);
    if (!main_pid || g_variant_get_uint32(main_pid) == 0)
        return -1;

    int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0);
    if (fd < 0)
        return -1;
    struct ucred peer;
    socklen_t size = sizeof(peer);
    // The live connection must be to the compositor owned by Canopa's unit.
    if (connect(fd, (struct sockaddr *)&address, sizeof(address)) < 0 ||
        getsockopt(fd, SOL_SOCKET, SO_PEERCRED, &peer, &size) < 0 ||
        peer.uid != getuid() || (guint32)peer.pid != g_variant_get_uint32(main_pid)) {
        close(fd);
        return -1;
    }
    return fd;
}

/* Controlled application-side token ownership probe for real Flatpak callers. */
#include <gio/gio.h>
#include <gio/gunixfdlist.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#define FRONT "org.freedesktop.portal.Desktop"
#define PATH "/org/freedesktop/portal/desktop"
#define PRINT "org.freedesktop.portal.Print"

typedef struct { char *label; gboolean prepare; int writer; char *bytes; gsize length; } Pending;
static GMainLoop *loop;
static GDBusConnection *primary, *extra;
static GHashTable *pending;
static guint subscriptions[2], serial;
static gboolean failed;

static void pending_free(gpointer pointer) {
    Pending *item = pointer;
    if (item->writer >= 0) close(item->writer);
    g_free(item->bytes); g_free(item->label); g_free(item);
}

static void error(const char *action, GError *failure) {
    g_printerr("ERROR %s %s\n", action, failure ? failure->message : "invalid request");
    g_clear_error(&failure); failed = TRUE; g_main_loop_quit(loop);
}

static void response(GDBusConnection *connection, const gchar *sender,
                     const gchar *path, const gchar *interface, const gchar *signal,
                     GVariant *parameters, gpointer data) {
    (void)sender; (void)interface; (void)signal; (void)data;
    Pending *item = g_hash_table_lookup(pending, path);
    if (!item) return;
    guint32 code, token = 0; GVariant *values;
    g_variant_get(parameters, "(u@a{sv})", &code, &values);
    if (item->prepare && code == 0 && !g_variant_lookup(values, "token", "u", &token)) {
        g_variant_unref(values); error("missing prepared token", NULL); return;
    }
    g_print("RESPONSE %s %s %u %u %s\n", item->label, item->prepare ? "prepare" : "print",
            code, token, g_dbus_connection_get_unique_name(connection));
    g_variant_unref(values); g_hash_table_remove(pending, path);
}

static GDBusConnection *connection(void) {
    GError *failure = NULL;
    GDBusConnection *result = g_dbus_connection_new_for_address_sync(
        g_getenv("DBUS_SESSION_BUS_ADDRESS"), G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT |
        G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION, NULL, NULL, &failure);
    if (!result) { error("session bus", failure); return NULL; }
    g_dbus_connection_set_exit_on_close(result, FALSE);
    return result;
}

static guint subscribe(GDBusConnection *bus) {
    return g_dbus_connection_signal_subscribe(bus, FRONT, "org.freedesktop.portal.Request", "Response",
        NULL, NULL, G_DBUS_SIGNAL_FLAGS_NONE, response, NULL, NULL);
}

static void request(GDBusConnection *bus, const char *label, gboolean prepare, guint32 token, gboolean held) {
    if (++serial > 64) { error("command limit", NULL); return; }
    GError *failure = NULL;
    GVariantBuilder options;
    g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
    char *handle_token = g_strdup_printf("%s%u", label, serial);
    g_variant_builder_add(&options, "{sv}", "handle_token", g_variant_new_string(handle_token));
    g_free(handle_token);
    GVariant *reply; int held_writer = -1; char *held_bytes = NULL; gsize held_length = 0;
    if (prepare) {
        GVariantBuilder settings, paper;
        g_variant_builder_init(&settings, G_VARIANT_TYPE_VARDICT);
        g_variant_builder_add(&settings, "{sv}", "n-copies", g_variant_new_string("1"));
        g_variant_builder_add(&settings, "{sv}", "output-file-format", g_variant_new_string("pdf"));
        g_variant_builder_init(&paper, G_VARIANT_TYPE_VARDICT);
        g_variant_builder_add(&paper, "{sv}", "Name", g_variant_new_string("iso_a4"));
        g_variant_builder_add(&paper, "{sv}", "DisplayName", g_variant_new_string("A4"));
        g_variant_builder_add(&paper, "{sv}", "Width", g_variant_new_double(210));
        g_variant_builder_add(&paper, "{sv}", "Height", g_variant_new_double(297));
        reply = g_dbus_connection_call_sync(bus, FRONT, PATH, PRINT, "PreparePrint",
            g_variant_new("(ss@a{sv}@a{sv}@a{sv})", "", "Token approval test",
                g_variant_builder_end(&settings), g_variant_builder_end(&paper), g_variant_builder_end(&options)),
            G_VARIANT_TYPE("(o)"), G_DBUS_CALL_FLAGS_NONE, 5000, NULL, &failure);
    } else {
        gchar *bytes = NULL, *name = NULL; gsize length = 0;
        if (!g_file_get_contents("/app/share/document.pdf", &bytes, &length, &failure)) {
            error("application document", failure); return;
        }
        int document;
        struct stat metadata;
        if (held) {
            int descriptors[2];
            if (pipe2(descriptors, O_CLOEXEC) != 0) { g_free(bytes); error("held document pipe", NULL); return; }
            document = descriptors[0]; held_writer = descriptors[1]; held_bytes = bytes; held_length = length;
            if (fstat(document, &metadata) != 0) { close(document); close(held_writer); g_free(bytes); error("pipe metadata", NULL); return; }
            g_print("DOCUMENT %s %zu pipe %llu\n", label, (size_t)length, (unsigned long long)metadata.st_ino);
        } else {
            int writer = g_file_open_tmp("canopa-token-XXXXXX.pdf", &name, &failure);
            if (writer < 0) { g_free(bytes); error("private document", failure); return; }
            close(writer);
            gboolean written = g_file_set_contents(name, bytes, length, &failure); g_free(bytes);
            document = written ? open(name, O_RDONLY | O_CLOEXEC) : -1;
            gboolean unlinked = document >= 0 && unlink(name) == 0 && fstat(document, &metadata) == 0 && metadata.st_nlink == 0;
            g_free(name);
            if (!unlinked) { if (document >= 0) close(document); error("unlinked document", failure); return; }
            g_print("DOCUMENT %s %zu unlinked\n", label, (size_t)length);
        }
        GUnixFDList *fds = g_unix_fd_list_new();
        int handle = g_unix_fd_list_append(fds, document, &failure); close(document);
        if (handle < 0) { g_object_unref(fds); error("document descriptor", failure); return; }
        g_variant_builder_add(&options, "{sv}", "token", g_variant_new_uint32(token));
        reply = g_dbus_connection_call_with_unix_fd_list_sync(bus, FRONT, PATH, PRINT, "Print",
            g_variant_new("(ssh@a{sv})", "", "Token approval test", handle, g_variant_builder_end(&options)),
            G_VARIANT_TYPE("(o)"), G_DBUS_CALL_FLAGS_NONE, 5000, fds, NULL, NULL, &failure);
        g_object_unref(fds);
    }
    if (!reply) { if (held_writer >= 0) close(held_writer); g_free(held_bytes); error(label, failure); return; }
    char *path; g_variant_get(reply, "(o)", &path); g_variant_unref(reply);
    Pending *item = g_new0(Pending, 1); item->label = g_strdup(label); item->prepare = prepare;
    item->writer = held_writer; item->bytes = held_bytes; item->length = held_length;
    g_hash_table_insert(pending, path, item);
    g_print("REQUEST %s %s %s %s\n", label, prepare ? "prepare" : "print", path,
            g_dbus_connection_get_unique_name(bus));
}

static gboolean control(GIOChannel *input, GIOCondition condition, gpointer data) {
    (void)data;
    char *line = NULL;
    if (!(condition & G_IO_IN) || g_io_channel_read_line(input, &line, NULL, NULL, NULL) != G_IO_STATUS_NORMAL) {
        g_free(line); error("controller EOF", NULL); return G_SOURCE_REMOVE;
    }
    g_strchomp(line);
    if (g_str_equal(line, "quit")) { g_free(line); g_main_loop_quit(loop); return G_SOURCE_REMOVE; }
    char **parts = g_strsplit(line, " ", -1); guint count = g_strv_length(parts);
    if (count == 2 && g_str_equal(parts[0], "release")) {
        GHashTableIter iterator; gpointer value; gboolean found = FALSE;
        g_hash_table_iter_init(&iterator, pending);
        while (g_hash_table_iter_next(&iterator, NULL, &value)) {
            Pending *item = value;
            if (g_str_equal(item->label, parts[1]) && item->writer >= 0) {
                found = TRUE;
                if (write(item->writer, item->bytes, item->length) != (ssize_t)item->length) error("release write", NULL);
                close(item->writer); item->writer = -1; g_clear_pointer(&item->bytes, g_free);
                g_print("RELEASED %s\n", item->label); break;
            }
        }
        if (!found) error("unknown held stream", NULL);
        goto finish;
    }
    gboolean prepare = count == 2 && g_str_equal(parts[0], "prepare");
    gboolean send = count == 3 && (g_str_equal(parts[0], "print") || g_str_equal(parts[0], "print_extra") || g_str_equal(parts[0], "print_held"));
    if ((!prepare && !send) || strlen(parts[1]) > 32 || !*parts[1]) { error("controller command", NULL); goto finish; }
    for (const char *p = parts[1]; *p; p++) if (!(g_ascii_isalnum(*p) || *p == '_')) { error("label", NULL); goto finish; }
    guint64 token = 0;
    if (send) {
        char *end; token = g_ascii_strtoull(parts[2], &end, 10);
        if (*end || !token || token > G_MAXUINT32) { error("token", NULL); goto finish; }
    }
    GDBusConnection *bus = primary;
    if (send && g_str_equal(parts[0], "print_extra")) {
        if (!extra) {
            extra = connection(); if (!extra) goto finish;
            subscriptions[1] = subscribe(extra);
            g_print("EXTRA %s\n", g_dbus_connection_get_unique_name(extra));
        }
        bus = extra;
    }
    request(bus, parts[1], prepare, (guint32)token, send && g_str_equal(parts[0], "print_held"));
finish:
    g_strfreev(parts); g_free(line); return !failed;
}

static gboolean deadline(gpointer data) {
    (void)data; error("controller deadline", NULL); return G_SOURCE_REMOVE;
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    if (argc != 3) return 2;
    GKeyFile *info = g_key_file_new();
    if (!g_key_file_load_from_file(info, "/.flatpak-info", G_KEY_FILE_NONE, NULL)) return 2;
    char *identity = g_key_file_get_string(info, "Application", "name", NULL);
    gboolean valid = g_strcmp0(identity, argv[1]) == 0 &&
        (g_str_equal(argv[1], "org.canopa.PortalPrintOwner") || g_str_equal(argv[1], "org.canopa.PortalPrintOther"));
    g_print("IDENTITY %s\n", identity ? identity : "missing");
    g_free(identity); g_key_file_unref(info);
    if (!valid || g_file_test(argv[2], G_FILE_TEST_EXISTS)) return 2;
    g_print("HOST_FILE inaccessible\n");
    loop = g_main_loop_new(NULL, FALSE);
    pending = g_hash_table_new_full(g_str_hash, g_str_equal, g_free, pending_free);
    primary = connection(); if (!primary) { g_hash_table_unref(pending); g_main_loop_unref(loop); return 1; }
    subscriptions[0] = subscribe(primary);
    GIOChannel *input = g_io_channel_unix_new(0);
    guint controller = g_io_add_watch(input, G_IO_IN | G_IO_HUP | G_IO_ERR, control, NULL);
    guint timer = g_timeout_add_seconds(180, deadline, NULL);
    g_print("READY %s %s\n", argv[1], g_dbus_connection_get_unique_name(primary));
    g_main_loop_run(loop);
    if (g_main_context_find_source_by_id(NULL, controller)) g_source_remove(controller);
    if (g_main_context_find_source_by_id(NULL, timer)) g_source_remove(timer);
    if (extra) { g_dbus_connection_signal_unsubscribe(extra, subscriptions[1]); g_dbus_connection_close_sync(extra, NULL, NULL); g_object_unref(extra); }
    g_dbus_connection_signal_unsubscribe(primary, subscriptions[0]); g_dbus_connection_close_sync(primary, NULL, NULL); g_object_unref(primary);
    g_io_channel_unref(input); g_hash_table_unref(pending); g_main_loop_unref(loop);
    g_print("DONE %d\n", failed); return failed ? 1 : 0;
}

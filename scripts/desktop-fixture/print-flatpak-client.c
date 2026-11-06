/* Application-side Print probe. Run only as the owned Flatpak test application. */
#include <gio/gio.h>
#include <gio/gunixfdlist.h>
#include <fcntl.h>
#include <stdio.h>
#include <sys/stat.h>
#include <unistd.h>

#define FRONT "org.freedesktop.portal.Desktop"
#define PATH "/org/freedesktop/portal/desktop"
#define PRINT "org.freedesktop.portal.Print"
#define APP_ID "org.canopa.PortalPrintProbe"

static GMainLoop *loop;
static GDBusConnection *bus;
static char *request;
static const char *mode;
static gboolean preparing;
static int outcome = 1;

static void fail(const char *action, GError *error) {
    g_printerr("%s: %s\n", action, error ? error->message : "invalid result");
    g_clear_error(&error);
    g_main_loop_quit(loop);
}

static gboolean timeout(gpointer data) {
    (void)data;
    fail("Print response timed out", NULL);
    return G_SOURCE_REMOVE;
}

static void print_document(guint32 token) {
    GError *error = NULL;
    gchar *document = NULL;
    gsize length = 0;
    if (!g_file_get_contents("/app/share/document.pdf", &document, &length, &error)) {
        fail("Read application document", error);
        return;
    }
    char *name = NULL;
    int source = g_file_open_tmp("canopa-print-XXXXXX.pdf", &name, &error);
    if (source < 0) {
        g_free(document);
        fail("Create private document", error);
        return;
    }
    gboolean written = g_file_set_contents(name, document, length, &error);
    g_free(document);
    close(source);
    source = written ? open(name, O_RDONLY | O_CLOEXEC) : -1;
    struct stat metadata;
    gboolean unlinked = source >= 0 && unlink(name) == 0 &&
        fstat(source, &metadata) == 0 && metadata.st_nlink == 0;
    g_free(name);
    if (!unlinked) {
        if (source >= 0) close(source);
        fail("Retain unlinked sandbox document", error);
        return;
    }
    GUnixFDList *fds = g_unix_fd_list_new();
    int handle = g_unix_fd_list_append(fds, source, &error);
    close(source);
    if (handle < 0) {
        g_object_unref(fds);
        fail("Transfer document descriptor", error);
        return;
    }
    g_print("DOCUMENT unlinked=%d bytes=%zu\n", unlinked, (size_t)length);
    GVariantBuilder options;
    g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
    g_variant_builder_add(&options, "{sv}", "handle_token", g_variant_new_string("print"));
    if (token) g_variant_builder_add(&options, "{sv}", "token", g_variant_new_uint32(token));
    preparing = FALSE;
    GVariant *reply = g_dbus_connection_call_with_unix_fd_list_sync(
        bus, FRONT, PATH, PRINT, "Print",
        g_variant_new("(ssh@a{sv})", "", "Flatpak print probe", handle,
                      g_variant_builder_end(&options)),
        G_VARIANT_TYPE("(o)"), G_DBUS_CALL_FLAGS_NONE, 5000, fds, NULL, NULL, &error);
    g_object_unref(fds);
    if (!reply) {
        fail("Public Print call", error);
        return;
    }
    g_free(request);
    g_variant_get(reply, "(o)", &request);
    g_print("REQUEST print %s\n", request);
    g_variant_unref(reply);
}

static void response(GDBusConnection *connection, const gchar *sender,
                     const gchar *path, const gchar *interface,
                     const gchar *signal, GVariant *parameters, gpointer data) {
    (void)connection; (void)sender; (void)interface; (void)signal; (void)data;
    if (g_strcmp0(path, request)) return;
    guint32 code;
    GVariant *values;
    g_variant_get(parameters, "(u@a{sv})", &code, &values);
    char *printed = g_variant_print(values, TRUE);
    g_print("RESPONSE %s %u %s\n", preparing ? "prepare" : "print", code, printed);
    g_free(printed);
    if (preparing && code == 0) {
        guint32 token = 0;
        if (g_variant_lookup(values, "token", "u", &token) && token) print_document(token);
        else fail("Prepared token", NULL);
    } else {
        outcome = code == (g_str_equal(mode, "cancel") ? 1u : 0u) ? 0 : 1;
        g_main_loop_quit(loop);
    }
    g_variant_unref(values);
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    if (argc != 3 || !(g_str_equal(argv[1], "prepared") ||
                      g_str_equal(argv[1], "direct") || g_str_equal(argv[1], "cancel"))) return 2;
    mode = argv[1];
    GError *error = NULL;
    GKeyFile *info = g_key_file_new();
    if (!g_key_file_load_from_file(info, "/.flatpak-info", G_KEY_FILE_NONE, &error)) {
        g_printerr("Flatpak identity: %s\n", error->message);
        g_clear_error(&error); g_key_file_unref(info); return 2;
    }
    char *identity = g_key_file_get_string(info, "Application", "name", NULL);
    gboolean identified = g_strcmp0(identity, APP_ID) == 0;
    g_print("IDENTITY %s\n", identity ? identity : "missing");
    g_free(identity); g_key_file_unref(info);
    if (!identified || g_file_test(argv[2], G_FILE_TEST_EXISTS)) return 2;
    g_print("HOST_FILE inaccessible\n");
    loop = g_main_loop_new(NULL, FALSE);
    bus = g_bus_get_sync(G_BUS_TYPE_SESSION, NULL, &error);
    if (!bus) { fail("Session bus", error); g_main_loop_unref(loop); return 1; }
    g_print("BUS %s\n", g_dbus_connection_get_unique_name(bus));
    guint subscription = g_dbus_connection_signal_subscribe(
        bus, FRONT, "org.freedesktop.portal.Request", "Response", NULL, NULL,
        G_DBUS_SIGNAL_FLAGS_NONE, response, NULL, NULL);
    guint deadline = g_timeout_add_seconds(120, timeout, NULL);
    preparing = !g_str_equal(mode, "direct");
    if (preparing) {
        GVariantBuilder settings, paper, options;
        g_variant_builder_init(&settings, G_VARIANT_TYPE_VARDICT);
        g_variant_builder_add(&settings, "{sv}", "n-copies", g_variant_new_string("1"));
        g_variant_builder_add(&settings, "{sv}", "output-file-format", g_variant_new_string("pdf"));
        g_variant_builder_init(&paper, G_VARIANT_TYPE_VARDICT);
        g_variant_builder_add(&paper, "{sv}", "Name", g_variant_new_string("iso_a4"));
        g_variant_builder_add(&paper, "{sv}", "DisplayName", g_variant_new_string("A4"));
        g_variant_builder_add(&paper, "{sv}", "Width", g_variant_new_double(210));
        g_variant_builder_add(&paper, "{sv}", "Height", g_variant_new_double(297));
        g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
        g_variant_builder_add(&options, "{sv}", "handle_token", g_variant_new_string("prepare"));
        GVariant *reply = g_dbus_connection_call_sync(
            bus, FRONT, PATH, PRINT, "PreparePrint",
            g_variant_new("(ss@a{sv}@a{sv}@a{sv})", "", "Flatpak print probe",
                          g_variant_builder_end(&settings), g_variant_builder_end(&paper),
                          g_variant_builder_end(&options)),
            G_VARIANT_TYPE("(o)"), G_DBUS_CALL_FLAGS_NONE, 5000, NULL, &error);
        if (reply) {
            g_variant_get(reply, "(o)", &request);
            g_print("REQUEST prepare %s\n", request);
            g_variant_unref(reply);
        } else fail("Public PreparePrint call", error);
    } else print_document(0);
    if (request) g_main_loop_run(loop);
    if (g_main_context_find_source_by_id(NULL, deadline)) g_source_remove(deadline);
    g_dbus_connection_signal_unsubscribe(bus, subscription);
    g_object_unref(bus); g_main_loop_unref(loop); g_free(request);
    g_print("DONE %s %d\n", mode, outcome);
    return outcome;
}

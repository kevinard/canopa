/* A real sandboxed GIO client. The chooser itself is Canopa's Rust/Iced UI. */
#include <gio/gio.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

#define APP_ID "org.canopa.PortalFileChooserProbe"
#define DESKTOP "org.freedesktop.portal.Desktop"
#define OBJECT "/org/freedesktop/portal/desktop"

static GMainLoop *loop;
static char *request_path;
static const char *mode;
static gboolean passed;

static void response(GDBusConnection *connection, const char *sender,
                     const char *path, const char *interface, const char *signal,
                     GVariant *parameters, gpointer data) {
    (void)connection; (void)sender; (void)interface; (void)signal; (void)data;
    if (g_strcmp0(path, request_path) != 0) return;
    guint32 status;
    GVariant *results;
    g_variant_get(parameters, "(u@a{sv})", &status, &results);
    g_print("RESPONSE %u\n", status);
    if (g_str_equal(mode, "cancel")) {
        char **uris = NULL;
        gboolean has_uris = g_variant_lookup(results, "uris", "^as", &uris);
        passed = status == 1 && (!has_uris || !uris[0]);
        g_strfreev(uris);
    } else if (status == 0) {
        char **uris = NULL;
        GError *error = NULL;
        if (!g_variant_lookup(results, "uris", "^as", &uris) || !uris[0] || uris[1])
            goto done;
        char *file = g_filename_from_uri(uris[0], NULL, &error);
        if (!file || !strstr(file, "/doc/")) {
            g_printerr("Missing Documents URI: %s\n", uris[0]);
            g_clear_error(&error); g_free(file); g_strfreev(uris); goto done;
        }
        g_print("DOCUMENT_URI %s\n", uris[0]);
        if (g_str_equal(mode, "save")) {
            const char *body = "Written through Canopa's sandbox file chooser\n";
            passed = g_file_set_contents(file, body, -1, &error);
            if (passed) g_print("SAVE_OK\n");
        } else {
            char *body = NULL; gsize length = 0;
            passed = g_file_get_contents(file, &body, &length, &error);
            if (passed) {
                char *hash = g_compute_checksum_for_data(G_CHECKSUM_SHA256, (guchar *)body, length);
                g_print("READ_OK %s %zu\n", hash, (size_t)length); g_free(hash);
                int write_fd = open(file, O_WRONLY | O_CLOEXEC);
                if (write_fd >= 0) { close(write_fd); passed = FALSE; }
                else g_print("READ_ONLY %d\n", errno);
            }
            g_free(body);
        }
        if (error) { g_printerr("Document access: %s\n", error->message); g_error_free(error); }
        g_free(file); g_strfreev(uris);
    }
done:
    g_variant_unref(results);
    g_main_loop_quit(loop);
}

static gboolean timeout(gpointer data) {
    (void)data; g_printerr("Timed out waiting for FileChooser\n");
    g_main_loop_quit(loop); return G_SOURCE_REMOVE;
}

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    setvbuf(stdout, NULL, _IONBF, 0);
    mode = argv[1];
    if (!g_str_equal(mode, "open") && !g_str_equal(mode, "save") && !g_str_equal(mode, "cancel")) return 2;
    GKeyFile *info = g_key_file_new();
    if (!g_key_file_load_from_file(info, "/.flatpak-info", G_KEY_FILE_NONE, NULL)) return 3;
    char *identity = g_key_file_get_string(info, "Application", "name", NULL);
    if (g_strcmp0(identity, APP_ID) != 0) return 3;
    g_print("IDENTITY %s\n", identity); g_free(identity); g_key_file_unref(info);
    int outside = open(argv[2], O_RDONLY | O_CLOEXEC);
    if (outside >= 0) { close(outside); g_printerr("Host file was already accessible\n"); return 4; }
    if (errno != ENOENT && errno != EACCES) return 4;
    g_print("HOST_FILE inaccessible %d\n", errno);
    GError *error = NULL;
    GDBusConnection *connection = g_bus_get_sync(G_BUS_TYPE_SESSION, NULL, &error);
    if (!connection) { g_printerr("Bus: %s\n", error->message); return 5; }
    loop = g_main_loop_new(NULL, FALSE);
    guint subscription = g_dbus_connection_signal_subscribe(connection, DESKTOP,
        "org.freedesktop.portal.Request", "Response", NULL, NULL,
        G_DBUS_SIGNAL_FLAGS_NONE, response, NULL, NULL);
    char *folder = g_path_get_dirname(argv[2]);
    GVariantBuilder options;
    g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
    g_variant_builder_add(&options, "{sv}", "handle_token", g_variant_new_string("chooserprobe"));
    g_variant_builder_add(&options, "{sv}", "current_folder", g_variant_new_bytestring(folder));
    if (g_str_equal(mode, "save"))
        g_variant_builder_add(&options, "{sv}", "current_name", g_variant_new_string("sandbox-saved.txt"));
    GVariant *reply = g_dbus_connection_call_sync(connection, DESKTOP, OBJECT,
        "org.freedesktop.portal.FileChooser", g_str_equal(mode, "save") ? "SaveFile" : "OpenFile",
        g_variant_new("(ss@a{sv})", "", "Canopa sandbox file choice", g_variant_builder_end(&options)),
        G_VARIANT_TYPE("(o)"), G_DBUS_CALL_FLAGS_NONE, 10000, NULL, &error);
    g_free(folder);
    if (!reply) { g_printerr("FileChooser: %s\n", error->message); return 6; }
    g_variant_get(reply, "(o)", &request_path); g_variant_unref(reply);
    g_print("REQUEST %s\n", request_path);
    guint timer = g_timeout_add_seconds(50, timeout, NULL);
    g_main_loop_run(loop);
    if (passed) g_source_remove(timer);
    g_dbus_connection_signal_unsubscribe(connection, subscription);
    g_object_unref(connection); g_main_loop_unref(loop); g_free(request_path);
    return passed ? 0 : 7;
}

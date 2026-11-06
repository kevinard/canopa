/* Ordinary GTK4 caller. GTK owns portal routing; this code makes no D-Bus calls. */
#include <gtk/gtk.h>
#include <stdio.h>
#include <string.h>

typedef struct {
    const char *mode;
    GtkWindow *parent;
    GtkFileDialog *dialog;
    GCancellable *cancel;
    GMainLoop *loop;
    int status;
} State;

static void finished(GObject *source, GAsyncResult *result, gpointer user_data) {
    State *state = user_data;
    GError *error = NULL;
    GFile *file = g_str_equal(state->mode, "save")
        ? gtk_file_dialog_save_finish(GTK_FILE_DIALOG(source), result, &error)
        : gtk_file_dialog_open_finish(GTK_FILE_DIALOG(source), result, &error);
    char *path = file ? g_file_get_path(file) : NULL;
    char *contents = NULL;
    if (file && !path) {
        g_set_error_literal(&error, G_IO_ERROR, G_IO_ERROR_FAILED, "Expected a local file");
    } else if (file && g_str_equal(state->mode, "save")) {
        const char *body = "Written by an ordinary GTK file dialog caller\n";
        GFileOutputStream *output = g_file_create(file, G_FILE_CREATE_NONE, NULL, &error);
        gsize written = 0;
        gboolean okay = output && g_output_stream_write_all(G_OUTPUT_STREAM(output), body, strlen(body), &written, NULL, &error);
        if (output) {
            if (!g_output_stream_close(G_OUTPUT_STREAM(output), NULL, NULL)) okay = FALSE;
            g_object_unref(output);
        }
        if (!okay || written != strlen(body)) {
            if (!error) g_set_error_literal(&error, G_IO_ERROR, G_IO_ERROR_FAILED, "Cannot finish Save");
        } else contents = g_strdup(body);
    } else if (file) {
        gsize length = 0;
        if (!g_file_load_contents(file, NULL, &contents, &length, NULL, &error)) {
            g_clear_pointer(&contents, g_free);
        } else if (!g_utf8_validate(contents, length, NULL)) {
            g_set_error_literal(&error, G_IO_ERROR, G_IO_ERROR_FAILED, "Expected UTF-8 fixture content");
        }
    }
    gboolean dismissed = g_error_matches(error, GTK_DIALOG_ERROR, GTK_DIALOG_ERROR_DISMISSED);
    if (error && !dismissed) {
        g_printerr("GtkFileDialog failed: %s\n", error->message);
        state->status = 1;
    } else {
        char *version = g_strdup_printf("%u.%u.%u", gtk_get_major_version(), gtk_get_minor_version(), gtk_get_micro_version());
        const char *paths[] = {path, NULL};
        GVariantBuilder evidence;
        g_variant_builder_init(&evidence, G_VARIANT_TYPE_VARDICT);
        g_variant_builder_add(&evidence, "{sv}", "gtk_version", g_variant_new_string(version));
        g_variant_builder_add(&evidence, "{sv}", "mode", g_variant_new_string(state->mode));
        g_variant_builder_add(&evidence, "{sv}", "api", g_variant_new_string("GtkFileDialog"));
        g_variant_builder_add(&evidence, "{sv}", "dialog_result", g_variant_new_int32(file ? 1 : 0));
        g_variant_builder_add(&evidence, "{sv}", "paths", g_variant_new_strv(file ? paths : NULL, file ? -1 : 0));
        g_variant_builder_add(&evidence, "{sv}", "contents", g_variant_new_string(contents ? contents : ""));
        GVariant *value = g_variant_ref_sink(g_variant_builder_end(&evidence));
        char *printed = g_variant_print(value, TRUE);
        g_print("GTK_RESULT %s\n", printed);
        g_free(printed);g_variant_unref(value);g_free(version);
    }
    g_clear_error(&error);g_clear_object(&file);g_free(path);g_free(contents);
    g_main_loop_quit(state->loop);
}

static gboolean open_dialog(gpointer user_data) {
    State *state = user_data;
    if (g_str_equal(state->mode, "save"))
        gtk_file_dialog_save(state->dialog, state->parent, state->cancel, finished, state);
    else gtk_file_dialog_open(state->dialog, state->parent, state->cancel, finished, state);
    return G_SOURCE_REMOVE;
}

static gboolean deadline(gpointer user_data) {
    State *state = user_data;
    state->status = 2;
    g_cancellable_cancel(state->cancel);
    g_main_loop_quit(state->loop);
    return G_SOURCE_REMOVE;
}

int main(int argc, char **argv) {
    if (argc != 3 || !(g_str_equal(argv[1], "open") || g_str_equal(argv[1], "save") || g_str_equal(argv[1], "cancel"))) return 2;
    setvbuf(stdout, NULL, _IOLBF, 0);
    g_set_prgname("org.canopa.FileChooserGtk");
    gtk_init();
    GtkWidget *parent = g_object_ref_sink(gtk_window_new());
    gtk_window_set_title(GTK_WINDOW(parent), "GTK file chooser parent");
    gtk_window_set_default_size(GTK_WINDOW(parent), 420, 180);
    gtk_window_set_child(GTK_WINDOW(parent), gtk_label_new("This application calls GtkFileDialog."));
    gtk_window_present(GTK_WINDOW(parent));
    GtkFileDialog *dialog = gtk_file_dialog_new();
    gtk_file_dialog_set_title(dialog, "Ordinary GTK file chooser");
    gtk_file_dialog_set_modal(dialog, TRUE);
    gtk_file_dialog_set_accept_label(dialog, g_str_equal(argv[1], "save") ? "Save" : "Open");
    GFile *folder = g_file_new_for_path(argv[2]);
    gtk_file_dialog_set_initial_folder(dialog, folder);g_object_unref(folder);
    if (g_str_equal(argv[1], "save")) gtk_file_dialog_set_initial_name(dialog, "GTK saved été.txt");
    GListStore *filters = g_list_store_new(GTK_TYPE_FILE_FILTER);
    GtkFileFilter *filter = gtk_file_filter_new();
    gtk_file_filter_set_name(filter, "Text documents");gtk_file_filter_add_pattern(filter, "*.txt");
    g_list_store_append(filters, filter);gtk_file_dialog_set_default_filter(dialog, filter);g_object_unref(filter);
    gtk_file_dialog_set_filters(dialog, G_LIST_MODEL(filters));g_object_unref(filters);
    State state = {.mode=argv[1], .parent=GTK_WINDOW(parent), .dialog=dialog,
                   .cancel=g_cancellable_new(), .loop=g_main_loop_new(NULL, FALSE), .status=0};
    guint opening = g_timeout_add(150, open_dialog, &state);
    guint timer = g_timeout_add_seconds(45, deadline, &state);
    g_main_loop_run(state.loop);
    if (g_main_context_find_source_by_id(NULL, opening)) g_source_remove(opening);
    if (g_main_context_find_source_by_id(NULL, timer)) g_source_remove(timer);
    g_object_unref(state.cancel);g_main_loop_unref(state.loop);g_object_unref(dialog);
    gtk_window_destroy(GTK_WINDOW(parent));g_object_unref(parent);
    return state.status;
}

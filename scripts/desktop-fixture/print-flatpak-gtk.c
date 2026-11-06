/* Real application-side GTK portal consumer; Canopa's UI remains Rust/Iced. */
#include <gtk/gtk.h>
#include <stdio.h>
#include <string.h>

#define APP_ID "org.canopa.PortalGtkPrintProbe"

static GMainLoop *loop;
static GtkPrintOperationResult result = GTK_PRINT_OPERATION_RESULT_ERROR;
static gboolean completed, released, failed;
static guint draws;
static const char *mode;
static GtkPrintOperation *printing;

static void draw_page(GtkPrintOperation *operation, GtkPrintContext *context,
                      gint page, gpointer data) {
    (void)operation; (void)data;
    cairo_t *cr = gtk_print_context_get_cairo_context(context);
    char label[32];
    g_snprintf(label, sizeof label, "PAGE-%d", page + 1);
    cairo_select_font_face(cr, "DejaVu Sans", CAIRO_FONT_SLANT_NORMAL, CAIRO_FONT_WEIGHT_NORMAL);
    cairo_set_font_size(cr, 24);
    cairo_move_to(cr, 40, 60);
    cairo_show_text(cr, label);
    if (page == 0) {
        cairo_rectangle(cr, 42, 80, 20, 20);
        cairo_fill(cr);
    }
    draws++;
    g_print("DRAW %d %.6f %.6f\n", page + 1,
            gtk_print_context_get_width(context), gtk_print_context_get_height(context));
}

static void report_spool(GtkPrintOperation *operation) {
    GtkPrintSettings *settings = gtk_print_operation_get_print_settings(operation);
    if (result == GTK_PRINT_OPERATION_RESULT_APPLY) {
        const char *uri = gtk_print_settings_get(settings, GTK_PRINT_SETTINGS_OUTPUT_URI);
        char *path = uri ? g_filename_from_uri(uri, NULL, NULL) : NULL;
        char *bytes = NULL; gsize length = 0;
        if (!path || !g_file_get_contents(path, &bytes, &length, NULL) || length > 2 * 1024 * 1024) {
            failed = TRUE;
            g_printerr("Missing application-rendered PDF\n");
        } else {
            char *digest = g_compute_checksum_for_data(G_CHECKSUM_SHA256, (const guchar *)bytes, length);
            g_print("SPOOL %s %zu %s\n", digest, (size_t)length, path);
            g_free(digest);
        }
        g_free(bytes); g_free(path);
    }
}

static void done(GtkPrintOperation *operation, GtkPrintOperationResult value, gpointer data) {
    (void)data;
    completed = TRUE; result = value;
    g_print("GTK_DONE %d %u\n", value, draws);
    GtkPrintSettings *settings = gtk_print_operation_get_print_settings(operation);
    if (value == GTK_PRINT_OPERATION_RESULT_APPLY) {
        g_print("SETTINGS copies=%d nup=%d reverse=%d scale=%.6f\n",
                gtk_print_settings_get_n_copies(settings), gtk_print_settings_get_number_up(settings),
                gtk_print_settings_get_reverse(settings), gtk_print_settings_get_scale(settings));
    }
    /* A GUI app remains alive after rendering. Only its owned test controller
       releases this loop, after observing publication or cancellation outside. */
}

static gboolean control(GIOChannel *channel, GIOCondition condition, gpointer data) {
    (void)data;
    char *line = NULL;
    if ((condition & G_IO_IN) &&
        g_io_channel_read_line(channel, &line, NULL, NULL, NULL) == G_IO_STATUS_NORMAL)
        released = g_strcmp0(line, "quit\n") == 0;
    g_free(line);
    /* GtkPrintOperation::done can precede the asynchronous file writer. The
       controller releases only after the public Print response and publication. */
    if (released && completed) report_spool(printing);
    g_main_loop_quit(loop);
    return G_SOURCE_REMOVE;
}

static gboolean timeout(gpointer data) {
    (void)data; failed = TRUE;
    g_printerr("Controller deadline expired\n");
    g_main_loop_quit(loop);
    return G_SOURCE_REMOVE;
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    if (argc != 3) return 2;
    mode = argv[1];
    if (!(g_str_equal(mode, "basic") || g_str_equal(mode, "layout") ||
          g_str_equal(mode, "cancel") || g_str_equal(mode, "withdraw"))) return 2;
    GKeyFile *info = g_key_file_new();
    if (!g_key_file_load_from_file(info, "/.flatpak-info", G_KEY_FILE_NONE, NULL)) return 2;
    char *identity = g_key_file_get_string(info, "Application", "name", NULL);
    gboolean identified = g_strcmp0(identity, APP_ID) == 0;
    g_print("IDENTITY %s\n", identity ? identity : "missing");
    g_free(identity); g_key_file_unref(info);
    if (!identified || g_file_test(argv[2], G_FILE_TEST_EXISTS) || g_getenv("GTK_USE_PORTAL")) return 2;
    g_print("HOST_FILE inaccessible\nPORTAL automatic\n");
    gtk_init(&argc, &argv);
    loop = g_main_loop_new(NULL, FALSE);
    GIOChannel *input = g_io_channel_unix_new(0);
    guint controller = g_io_add_watch(input, G_IO_IN | G_IO_HUP | G_IO_ERR, control, NULL);
    guint deadline = g_timeout_add_seconds(120, timeout, NULL);
    GtkWidget *parent = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_title(GTK_WINDOW(parent), "Canopa GTK Flatpak print probe");
    gtk_window_set_default_size(GTK_WINDOW(parent), 320, 160);
    gtk_widget_show(parent);
    GtkPrintOperation *operation = gtk_print_operation_new();
    printing = operation;
    gtk_print_operation_set_job_name(operation, "Flatpak GTK print probe");
    gtk_print_operation_set_n_pages(operation, 4);
    GtkPrintSettings *settings = gtk_print_settings_new();
    gtk_print_settings_set_n_copies(settings, 1);
    if (g_str_equal(mode, "layout")) {
        GtkPageRange range = {1, 3};
        gtk_print_settings_set_print_pages(settings, GTK_PRINT_PAGES_RANGES);
        gtk_print_settings_set_page_ranges(settings, &range, 1);
        gtk_print_settings_set_reverse(settings, TRUE);
        gtk_print_settings_set_number_up(settings, 2);
        gtk_print_settings_set_scale(settings, 75);
    }
    gtk_print_operation_set_print_settings(operation, settings);
    g_object_unref(settings);
    gtk_print_operation_set_allow_async(operation, TRUE);
    g_signal_connect(operation, "draw-page", G_CALLBACK(draw_page), NULL);
    g_signal_connect(operation, "done", G_CALLBACK(done), NULL);
    GError *error = NULL;
    GtkPrintOperationResult initial = gtk_print_operation_run(operation,
        GTK_PRINT_OPERATION_ACTION_PRINT_DIALOG, GTK_WINDOW(parent), &error);
    if (error) {
        g_printerr("GTK Print: %s\n", error->message); g_error_free(error); failed = TRUE;
    }
    if (initial != GTK_PRINT_OPERATION_RESULT_IN_PROGRESS && !completed) done(operation, initial, NULL);
    g_main_loop_run(loop);
    if (g_main_context_find_source_by_id(NULL, controller)) g_source_remove(controller);
    if (g_main_context_find_source_by_id(NULL, deadline)) g_source_remove(deadline);
    g_io_channel_unref(input); g_main_loop_unref(loop);
    g_object_unref(operation); gtk_widget_destroy(parent);
    gboolean expected = result == (g_str_equal(mode, "cancel") ? GTK_PRINT_OPERATION_RESULT_CANCEL : GTK_PRINT_OPERATION_RESULT_APPLY);
    gboolean success = completed && released && !failed && expected;
    g_print("DONE %s %d\n", mode, success);
    return success ? 0 : 1;
}

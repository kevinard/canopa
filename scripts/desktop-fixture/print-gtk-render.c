/* Disposable application-side compatibility probe, not a portal backend. */
#include <gtk/gtk.h>

static GMainLoop *loop;
static gboolean completed;
static GtkPrintOperationResult final_result;
static guint draws;
static const char *received;

static gboolean output_received(gpointer user_data) {
    (void)user_data;
    if (completed && g_file_test(received, G_FILE_TEST_IS_REGULAR)) {
        g_main_loop_quit(loop);
        return G_SOURCE_REMOVE;
    }
    return G_SOURCE_CONTINUE;
}

static void draw_page(GtkPrintOperation *operation, GtkPrintContext *context,
                      gint page, gpointer user_data) {
    (void)operation;
    (void)user_data;
    cairo_t *cr = gtk_print_context_get_cairo_context(context);
    char label[32];
    g_snprintf(label, sizeof label, "PAGE-%d", page + 1);
    cairo_select_font_face(cr, "sans", CAIRO_FONT_SLANT_NORMAL,
                          CAIRO_FONT_WEIGHT_NORMAL);
    cairo_set_font_size(cr, 24.0);
    cairo_move_to(cr, 40.0, 60.0);
    cairo_show_text(cr, label);
    draws++;
}

static void done(GtkPrintOperation *operation, GtkPrintOperationResult result,
                 gpointer user_data) {
    (void)operation;
    (void)user_data;
    completed = TRUE;
    final_result = result;
    if (result != GTK_PRINT_OPERATION_RESULT_APPLY && g_main_loop_is_running(loop))
        g_main_loop_quit(loop);
}

int main(int argc, char **argv) {
    gtk_init(&argc, &argv);
    received = g_getenv("CANOPA_PRINT_PROBE_RECEIVED");
    if (!received)
        return 2;
    loop = g_main_loop_new(NULL, FALSE);
    GtkWidget *parent = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_title(GTK_WINDOW(parent), "Canopa print client probe");
    gtk_window_set_default_size(GTK_WINDOW(parent), 320, 160);
    gtk_widget_show(parent);
    GtkPrintOperation *operation = gtk_print_operation_new();
    gtk_print_operation_set_job_name(operation, "Canopa prepared rendering probe");
    const char *pages = g_getenv("CANOPA_PRINT_PROBE_PAGES");
    int page_count = pages ? (int)g_ascii_strtoll(pages, NULL, 10) : 4;
    if (page_count < 1 || page_count > 10000)
        return 2;
    gtk_print_operation_set_n_pages(operation, page_count);
    GtkPrintSettings *settings = gtk_print_settings_new();
    const char *copies = g_getenv("CANOPA_PRINT_PROBE_COPIES");
    int copy_count = copies ? (int)g_ascii_strtoll(copies, NULL, 10) : 1;
    if (copy_count < 1 || copy_count > 9999)
        return 2;
    gtk_print_settings_set_n_copies(settings, copy_count);
    gtk_print_settings_set_collate(settings,
        g_strcmp0(g_getenv("CANOPA_PRINT_PROBE_COLLATE"), "true") == 0);
    gtk_print_operation_set_print_settings(operation, settings);
    g_object_unref(settings);
    gtk_print_operation_set_allow_async(operation, TRUE);
    g_signal_connect(operation, "draw-page", G_CALLBACK(draw_page), NULL);
    g_signal_connect(operation, "done", G_CALLBACK(done), NULL);
    GError *error = NULL;
    GtkPrintOperationResult result = gtk_print_operation_run(
        operation, GTK_PRINT_OPERATION_ACTION_PRINT_DIALOG,
        GTK_WINDOW(parent), &error);
    g_timeout_add(10, output_received, NULL);
    if (result == GTK_PRINT_OPERATION_RESULT_IN_PROGRESS ||
        (result == GTK_PRINT_OPERATION_RESULT_APPLY &&
         !g_file_test(received, G_FILE_TEST_IS_REGULAR)))
        g_main_loop_run(loop);
    if (error) {
        g_printerr("Print probe failed: %s\n", error->message);
        g_error_free(error);
    }
    if (!completed)
        final_result = result;
    g_print("{\"result\":%d,\"draw_calls\":%u}\n", final_result, draws);
    g_object_unref(operation);
    gtk_widget_destroy(parent);
    g_main_loop_unref(loop);
    return final_result == GTK_PRINT_OPERATION_RESULT_APPLY ? 0 : 1;
}

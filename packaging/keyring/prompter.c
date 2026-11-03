#define GCR_API_SUBJECT_TO_CHANGE
#include <gcr/gcr.h>
#include <sys/prctl.h>
#include <string.h>
#include "session.h"

#ifndef CANOPA_PINENTRY
#define CANOPA_PINENTRY "/usr/bin/canopa-pinentry"
#endif

#define PROMPTER_NAME "org.gnome.keyring.SystemPrompter"
#define MAX_PASSWORD 8192

enum {
    PROP_TITLE = 1,
    PROP_MESSAGE,
    PROP_DESCRIPTION,
    PROP_WARNING,
    PROP_PASSWORD_NEW,
    PROP_PASSWORD_STRENGTH,
    PROP_CHOICE_LABEL,
    PROP_CHOICE_CHOSEN,
    PROP_CALLER_WINDOW,
    PROP_CONTINUE_LABEL,
    PROP_CANCEL_LABEL,
    N_PROPERTIES,
};

typedef struct {
    GObject parent;
    GValue properties[N_PROPERTIES];
    GcrPrompt *stock;
    GSubprocess *child; /* Owned by the pending task. */
    char *password;
    gboolean closed;
} CanopaPrompt;

typedef GObjectClass CanopaPromptClass;

static const char *const property_names[] = {
    NULL, "title", "message", "description", "warning", "password-new",
    "password-strength", "choice-label", "choice-chosen", "caller-window",
    "continue-label", "cancel-label",
};

static void prompt_interface_init(GcrPromptIface *iface);
G_DEFINE_TYPE_WITH_CODE(CanopaPrompt, canopa_prompt, G_TYPE_OBJECT,
                        G_IMPLEMENT_INTERFACE(GCR_TYPE_PROMPT, prompt_interface_init))

static void canopa_prompt_init(CanopaPrompt *self)
{
    for (guint id = 1; id < N_PROPERTIES; id++) {
        GParamSpec *spec = g_object_class_find_property(G_OBJECT_GET_CLASS(self), property_names[id]);
        g_value_init(&self->properties[id], G_PARAM_SPEC_VALUE_TYPE(spec));
        g_param_value_set_default(spec, &self->properties[id]);
    }
}

static void get_property(GObject *object, guint id, GValue *value, GParamSpec *spec)
{
    (void)spec;
    g_value_copy(&((CanopaPrompt *)object)->properties[id], value);
}

static void set_property(GObject *object, guint id, const GValue *value, GParamSpec *spec)
{
    (void)spec;
    GValue *dest = &((CanopaPrompt *)object)->properties[id];
    g_value_reset(dest);
    g_value_copy(value, dest);
}

static void close_prompt(GcrPrompt *prompt)
{
    CanopaPrompt *self = (CanopaPrompt *)prompt;
    self->closed = TRUE;
    if (self->child)
        g_subprocess_force_exit(self->child);
    if (self->stock) {
        GcrPrompt *stock = g_steal_pointer(&self->stock);
        gcr_prompt_close(stock);
        gtk_widget_destroy(GTK_WIDGET(stock));
        g_object_unref(stock);
    }
}

static void finalize(GObject *object)
{
    CanopaPrompt *self = (CanopaPrompt *)object;
    if (self->stock) {
        gtk_widget_destroy(GTK_WIDGET(self->stock));
        g_object_unref(self->stock);
    }
    g_clear_pointer(&self->password, gcr_secure_memory_free);
    for (guint id = 1; id < N_PROPERTIES; id++)
        g_value_unset(&self->properties[id]);
    G_OBJECT_CLASS(canopa_prompt_parent_class)->finalize(object);
}

static void canopa_prompt_class_init(CanopaPromptClass *klass)
{
    klass->get_property = get_property;
    klass->set_property = set_property;
    klass->finalize = finalize;
    for (guint id = 1; id < N_PROPERTIES; id++)
        g_object_class_override_property(klass, id, property_names[id]);
}

typedef struct {
    GSubprocess *child;
    char *buffer;
    gsize length;
    gboolean password;
    gboolean read_ok;
} Operation;

static void operation_free(gpointer data)
{
    Operation *operation = data;
    g_clear_object(&operation->child);
    g_clear_pointer(&operation->buffer, gcr_secure_memory_free);
    g_free(operation);
}

static void cancel_task(GTask *task)
{
    Operation *operation = g_task_get_task_data(task);
    if (operation->password)
        g_task_return_pointer(task, NULL, NULL);
    else
        g_task_return_int(task, GCR_PROMPT_REPLY_CANCEL);
}

static void child_exited(GObject *source, GAsyncResult *result, gpointer data)
{
    g_autoptr(GTask) task = data;
    CanopaPrompt *self = g_task_get_source_object(task);
    Operation *operation = g_task_get_task_data(task);
    GSubprocess *child = G_SUBPROCESS(source);
    self->child = NULL;
    if (!g_subprocess_wait_finish(child, result, NULL) ||
        !g_subprocess_get_successful(child) || !operation->read_ok || self->closed) {
        cancel_task(task);
        return;
    }
    if (!operation->password) {
        g_task_return_int(task, operation->length == 0
            ? GCR_PROMPT_REPLY_CONTINUE : GCR_PROMPT_REPLY_CANCEL);
        return;
    }
    if (memchr(operation->buffer, '\0', operation->length) ||
        !g_utf8_validate(operation->buffer, operation->length, NULL)) {
        cancel_task(task);
        return;
    }
    operation->buffer[operation->length] = '\0';
    g_value_set_int(&self->properties[PROP_PASSWORD_STRENGTH], operation->length != 0);
    g_object_notify(G_OBJECT(self), "password-strength");
    g_task_return_pointer(task, g_steal_pointer(&operation->buffer), gcr_secure_memory_free);
}

static void child_read(GObject *source, GAsyncResult *result, gpointer data)
{
    GTask *task = data;
    Operation *operation = g_task_get_task_data(task);
    operation->read_ok = g_input_stream_read_all_finish(
        G_INPUT_STREAM(source), result, &operation->length, NULL) &&
        operation->length <= MAX_PASSWORD;
    if (!operation->read_ok)
        g_subprocess_force_exit(operation->child);
    g_subprocess_wait_async(operation->child, NULL, child_exited, task);
}

static const char *text(CanopaPrompt *self, guint id)
{
    const char *value = g_value_get_string(&self->properties[id]);
    return value ? value : "";
}

static char *option(const char *name, const char *value)
{
    // The bridge only puts display text on the command line, capped at a UTF-8 boundary.
    gsize length = strlen(value);
    if (length > 2048) {
        length = 2048;
        while (((unsigned char)value[length] & 0xc0) == 0x80)
            length--;
    }
    return g_strdup_printf("--%s=%.*s", name, (int)length, value);
}

static gboolean start_canopa(CanopaPrompt *self, GTask *task, int display)
{
    Operation *operation = g_task_get_task_data(task);
    g_autoptr(GSubprocessLauncher) launcher = g_subprocess_launcher_new(
        G_SUBPROCESS_FLAGS_STDIN_PIPE | G_SUBPROCESS_FLAGS_STDOUT_PIPE);
    g_subprocess_launcher_take_fd(launcher, display, 3);
    g_subprocess_launcher_setenv(launcher, "WAYLAND_SOCKET", "3", TRUE);
    const char *message = text(self, PROP_MESSAGE);
    const char *details = text(self, PROP_DESCRIPTION);
    g_autofree char *body = g_strconcat(message, *message && *details ? "\n" : "", details, NULL);
    g_autofree char *title = option("title", text(self, PROP_TITLE));
    g_autofree char *description = option("description", body);
    g_autofree char *warning = option("warning", text(self, PROP_WARNING));
    g_autofree char *accept = option("accept", text(self, PROP_CONTINUE_LABEL));
    g_autofree char *cancel = option("cancel", text(self, PROP_CANCEL_LABEL));
    const char *mode = operation->password
        ? (g_value_get_boolean(&self->properties[PROP_PASSWORD_NEW]) ? "--password-new" : NULL)
        : "--confirm";
    const char *argv[] = { CANOPA_PINENTRY, "keyring-dialog", title, description,
                           warning, accept, cancel, mode, NULL };
    operation->child = g_subprocess_launcher_spawnv(launcher, argv, NULL);
    if (!operation->child)
        return FALSE;
    self->child = operation->child;
    // Keep the stdin pipe open. Canopa closes the dialog if the prompter disappears.
    operation->buffer = gcr_secure_memory_alloc(MAX_PASSWORD + 1);
    g_input_stream_read_all_async(g_subprocess_get_stdout_pipe(operation->child),
        operation->buffer, MAX_PASSWORD + 1, G_PRIORITY_DEFAULT, g_task_get_cancellable(task),
        child_read, g_object_ref(task));
    return TRUE;
}

static void stock_finished(GObject *source, GAsyncResult *result, gpointer data)
{
    g_autoptr(GTask) task = data;
    CanopaPrompt *self = g_task_get_source_object(task);
    Operation *operation = g_task_get_task_data(task);
    GcrPrompt *stock = GCR_PROMPT(source);
    g_autoptr(GError) error = NULL;
    if (self->closed) {
        cancel_task(task);
        return;
    }
    g_object_get_property(source, "choice-chosen", &self->properties[PROP_CHOICE_CHOSEN]);
    g_object_notify(G_OBJECT(self), "choice-chosen");
    if (operation->password) {
        const char *password = gcr_prompt_password_finish(stock, result, &error);
        g_object_get_property(source, "password-strength", &self->properties[PROP_PASSWORD_STRENGTH]);
        g_object_notify(G_OBJECT(self), "password-strength");
        g_task_return_pointer(task, password ? gcr_secure_memory_strdup(password) : NULL,
                              gcr_secure_memory_free);
    } else {
        GcrPromptReply reply = gcr_prompt_confirm_finish(stock, result, &error);
        g_task_return_int(task, error ? GCR_PROMPT_REPLY_CANCEL : reply);
    }
}

static GdkDisplay *stock_display(char **environment)
{
    static gboolean initialized;
    if (!initialized) {
        gtk_init_check(NULL, NULL);
        initialized = TRUE;
    }
    const char *name = environment ? g_environ_getenv(environment, "WAYLAND_DISPLAY") : NULL;
    if (!name || !*name)
        name = environment ? g_environ_getenv(environment, "DISPLAY") : NULL;
    if (!name || !*name)
        return gdk_display_get_default();

    GdkDisplayManager *manager = gdk_display_manager_get();
    g_autoptr(GSList) displays = gdk_display_manager_list_displays(manager);
    for (GSList *item = displays; item; item = item->next) {
        GdkDisplay *display = item->data;
        if (strcmp(name, gdk_display_get_name(display)) == 0) {
            gdk_display_manager_set_default_display(manager, display);
            return display;
        }
    }
    GdkDisplay *display = gdk_display_open(name);
    if (display)
        gdk_display_manager_set_default_display(manager, display);
    return display;
}

static void start_stock(CanopaPrompt *self, GTask *task, char **environment)
{
    if (!self->stock) {
        GdkDisplay *display = stock_display(environment);
        if (!display) {
            cancel_task(task);
            return;
        }
        self->stock = GCR_PROMPT(g_object_ref_sink(g_object_new(
            GCR_TYPE_PROMPT_DIALOG, "screen", gdk_display_get_default_screen(display), NULL)));
    }
    for (guint id = 1; id < N_PROPERTIES; id++) {
        if (id != PROP_PASSWORD_STRENGTH)
            g_object_set_property(G_OBJECT(self->stock), property_names[id], &self->properties[id]);
    }
    Operation *operation = g_task_get_task_data(task);
    if (operation->password)
        gcr_prompt_password_async(self->stock, g_task_get_cancellable(task),
                                  stock_finished, g_object_ref(task));
    else
        gcr_prompt_confirm_async(self->stock, g_task_get_cancellable(task),
                                 stock_finished, g_object_ref(task));
}

static void start_prompt(GcrPrompt *prompt, GCancellable *cancellable,
                         GAsyncReadyCallback callback, gpointer data, gboolean password)
{
    CanopaPrompt *self = (CanopaPrompt *)prompt;
    g_autoptr(GTask) task = g_task_new(prompt, cancellable, callback, data);
    Operation *operation = g_new0(Operation, 1);
    operation->password = password;
    g_task_set_task_data(task, operation, operation_free);
    if (self->closed || self->child) {
        cancel_task(task);
        return;
    }
    g_clear_pointer(&self->password, gcr_secure_memory_free);
    g_value_set_int(&self->properties[PROP_PASSWORD_STRENGTH], 0);
    g_autoptr(GDBusConnection) bus = g_bus_get_sync(G_BUS_TYPE_SESSION, NULL, NULL);
    g_auto(GStrv) environment = bus ? user_manager_environment(bus) : NULL;
    // GCR also serves prompts with a caller-supplied checkbox.
    if (!self->stock && !*text(self, PROP_CHOICE_LABEL)) {
        int display = connect_canopa_display(bus, environment);
        if (display >= 0 && start_canopa(self, task, display))
            return;
    }
    start_stock(self, task, environment);
}

static void password_async(GcrPrompt *prompt, GCancellable *cancellable,
                           GAsyncReadyCallback callback, gpointer data)
{
    start_prompt(prompt, cancellable, callback, data, TRUE);
}

static const char *password_finish(GcrPrompt *prompt, GAsyncResult *result, GError **error)
{
    CanopaPrompt *self = (CanopaPrompt *)prompt;
    g_clear_pointer(&self->password, gcr_secure_memory_free);
    self->password = g_task_propagate_pointer(G_TASK(result), error);
    return self->password;
}

static void confirm_async(GcrPrompt *prompt, GCancellable *cancellable,
                          GAsyncReadyCallback callback, gpointer data)
{
    start_prompt(prompt, cancellable, callback, data, FALSE);
}

static GcrPromptReply confirm_finish(GcrPrompt *prompt, GAsyncResult *result, GError **error)
{
    (void)prompt;
    return g_task_propagate_int(G_TASK(result), error) == GCR_PROMPT_REPLY_CONTINUE
        ? GCR_PROMPT_REPLY_CONTINUE : GCR_PROMPT_REPLY_CANCEL;
}

static void prompt_interface_init(GcrPromptIface *iface)
{
    iface->prompt_password_async = password_async;
    iface->prompt_password_finish = password_finish;
    iface->prompt_confirm_async = confirm_async;
    iface->prompt_confirm_finish = confirm_finish;
    iface->prompt_close = close_prompt;
}

typedef struct {
    GMainLoop *loop;
    GcrSystemPrompter *prompter;
    guint idle;
    gboolean acquired;
} Service;

static gboolean idle_exit(gpointer data)
{
    Service *service = data;
    service->idle = 0;
    g_main_loop_quit(service->loop);
    return G_SOURCE_REMOVE;
}

static void prompting_changed(GObject *object, GParamSpec *spec, gpointer data)
{
    (void)object;
    (void)spec;
    Service *service = data;
    g_clear_handle_id(&service->idle, g_source_remove);
    if (!gcr_system_prompter_get_prompting(service->prompter))
        service->idle = g_timeout_add_seconds(15, idle_exit, service);
}

static void name_acquired(GDBusConnection *bus, const gchar *name, gpointer data)
{
    (void)bus;
    (void)name;
    Service *service = data;
    service->acquired = TRUE;
}

static void name_lost(GDBusConnection *bus, const gchar *name, gpointer data)
{
    (void)bus;
    (void)name;
    g_main_loop_quit(((Service *)data)->loop);
}

int main(void)
{
    if (prctl(PR_SET_DUMPABLE, 0, 0, 0, 0) < 0)
        return 1;
    g_autoptr(GDBusConnection) bus = g_bus_get_sync(G_BUS_TYPE_SESSION, NULL, NULL);
    if (!bus)
        return 1;
    g_autoptr(GMainLoop) loop = g_main_loop_new(NULL, FALSE);
    g_autoptr(GcrSystemPrompter) prompter = gcr_system_prompter_new(
        GCR_SYSTEM_PROMPTER_SINGLE, canopa_prompt_get_type());
    Service service = { .loop = loop, .prompter = prompter };
    g_signal_connect(prompter, "notify::prompting", G_CALLBACK(prompting_changed), &service);
    gcr_system_prompter_register(prompter, bus);
    guint owner = g_bus_own_name_on_connection(bus, PROMPTER_NAME, G_BUS_NAME_OWNER_FLAGS_NONE,
                                              name_acquired, name_lost, &service, NULL);
    prompting_changed(NULL, NULL, &service);
    g_main_loop_run(loop);
    g_clear_handle_id(&service.idle, g_source_remove);
    g_signal_handlers_disconnect_by_data(prompter, &service);
    gcr_system_prompter_unregister(prompter, FALSE);
    g_bus_unown_name(owner);
    return service.acquired ? 0 : 1;
}

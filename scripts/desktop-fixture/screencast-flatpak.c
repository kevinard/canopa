/* Real sandbox caller: portal identity, restricted PipeWire FD and actual pixels.
 * Reuse the maintained raw-frame reader, not a second capture implementation. */
#define _GNU_SOURCE
#include <gio/gio.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#define main original_consumer_main
#include "screencast-consumer.c"
#undef main

#define APP "org.canopa.PortalScreenCastProbe"
#define FRONT "org.freedesktop.portal.Desktop"
#define PATH "/org/freedesktop/portal/desktop"
#define CAST "org.freedesktop.portal.ScreenCast"

static GDBusConnection *bus;
static GHashTable *responses;
static char *session_path;
static gboolean session_closed;
static unsigned request_serial;

static void fatal(const char *message) {
    g_printerr("FAIL %s\n", message);
    exit(1);
}
static gboolean deadline(gpointer unused) {
    (void)unused;
    fatal("portal deadline");
    return G_SOURCE_REMOVE;
}
static GVariant *call(const char *path, const char *interface,
                      const char *method, GVariant *parameters) {
    GError *error = NULL;
    GVariant *reply = g_dbus_connection_call_sync(bus, FRONT, path, interface, method,
        parameters, NULL, G_DBUS_CALL_FLAGS_NONE, 10000, NULL, &error);
    if (!reply) fatal(error ? error->message : "portal call failed");
    return reply;
}
static void response(GDBusConnection *c, const char *sender, const char *path,
                     const char *interface, const char *signal, GVariant *value,
                     gpointer unused) {
    (void)c; (void)sender; (void)interface; (void)signal; (void)unused;
    if (g_hash_table_size(responses) >= 8) fatal("too many responses");
    g_hash_table_insert(responses, g_strdup(path), g_variant_ref(value));
}
static void closed(GDBusConnection *c, const char *sender, const char *path,
                   const char *interface, const char *signal, GVariant *value,
                   gpointer unused) {
    (void)c; (void)sender; (void)interface; (void)signal; (void)value; (void)unused;
    if (session_path && !strcmp(path, session_path)) {
        session_closed = TRUE;
        puts("SESSION_CLOSED");
    }
}
static void options(GVariantBuilder *builder) {
    g_variant_builder_init(builder, G_VARIANT_TYPE_VARDICT);
    char token[32];
    snprintf(token, sizeof(token), "cast%u", ++request_serial);
    g_variant_builder_add(builder, "{sv}", "handle_token", g_variant_new_string(token));
}
static GVariant *request(const char *method, GVariant *parameters, guint32 expected) {
    GVariant *reply = call(PATH, CAST, method, parameters);
    const char *borrowed;
    g_variant_get(reply, "(&o)", &borrowed);
    char *path = g_strdup(borrowed);
    guint timer = g_timeout_add_seconds(35, deadline, NULL);
    while (!g_hash_table_contains(responses, path)) g_main_context_iteration(NULL, TRUE);
    g_source_remove(timer);
    GVariant *answer = g_hash_table_lookup(responses, path), *values;
    guint32 status;
    g_variant_get(answer, "(u@a{sv})", &status, &values);
    printf("RESPONSE %s %u\n", method, status);
    if (status != expected) fatal("unexpected request response");
    g_hash_table_remove(responses, path);
    g_variant_unref(reply); g_free(path);
    return values;
}

struct visibility {
    struct pw_main_loop *loop;
    uint32_t selected, forbidden;
    int sequence;
    bool selected_seen, forbidden_seen, done, failed;
};
static void global(void *data, uint32_t id, uint32_t permissions, const char *type,
                   uint32_t version, const struct spa_dict *props) {
    (void)permissions; (void)version; (void)props;
    struct visibility *visible = data;
    if (strcmp(type, PW_TYPE_INTERFACE_Node)) return;
    printf("VISIBLE_NODE %u\n", id);
    visible->selected_seen |= id == visible->selected;
    visible->forbidden_seen |= id == visible->forbidden;
}
static void global_remove(void *data, uint32_t id) {
    struct visibility *visible = data;
    if (id == visible->selected || id == visible->forbidden)
        printf("REMOVED_NODE %u\n", id);
}
static void roundtrip(void *data, uint32_t id, int sequence) {
    struct visibility *visible = data;
    if (id == PW_ID_CORE && sequence == visible->sequence) {
        visible->done = true;
        pw_main_loop_quit(visible->loop);
    }
}
static void core_error(void *data, uint32_t id, int sequence, int result, const char *message) {
    (void)id; (void)sequence;
    struct visibility *visible = data;
    fprintf(stderr, "PipeWire error %d: %s\n", result, message);
    visible->failed = true;
    pw_main_loop_quit(visible->loop);
}
static void media_deadline(void *data, uint64_t expirations) {
    (void)expirations;
    struct visibility *visible = data;
    visible->failed = true;
    pw_main_loop_quit(visible->loop);
}
static void pixels(const struct capture *capture) {
    char *body = NULL; gsize length = 0;
    if (!g_file_get_contents("/tmp/canopa-frame/frame.rgba", &body, &length, NULL)
        || length != (gsize)capture->format.size.width * capture->format.size.height * 4
        || length > MAX_FRAME) fatal("frame length");
    const unsigned colors[4][3] = {{180,20,40},{10,40,140},{20,150,40},{180,154,24}};
    unsigned counts[4] = {0}, bounds[4][4];
    for (unsigned i = 0; i < 4; i++) {
        bounds[i][0] = bounds[i][1] = UINT32_MAX;
        bounds[i][2] = bounds[i][3] = 0;
    }
    for (unsigned y = 0; y < capture->format.size.height; y++) {
        for (unsigned x = 0; x < capture->format.size.width; x++) {
            const unsigned char *pixel = (unsigned char *)body + ((gsize)y * capture->format.size.width + x) * 4;
            for (unsigned i = 0; i < 4; i++) {
                if (abs((int)pixel[0] - (int)colors[i][0]) > 2
                    || abs((int)pixel[1] - (int)colors[i][1]) > 2
                    || abs((int)pixel[2] - (int)colors[i][2]) > 2) continue;
                counts[i]++;
                if (x < bounds[i][0]) bounds[i][0] = x;
                if (y < bounds[i][1]) bounds[i][1] = y;
                if (x + 1 > bounds[i][2]) bounds[i][2] = x + 1;
                if (y + 1 > bounds[i][3]) bounds[i][3] = y + 1;
            }
        }
    }
    char *hash = g_compute_checksum_for_data(G_CHECKSUM_SHA256, (const guchar *)body, length);
    printf("FRAME %u %u %s\n", capture->format.size.width, capture->format.size.height, hash);
    for (unsigned i = 0; i < 4; i++) {
        if (!counts[i]) fatal("source pattern missing");
        printf("COLOR %u %u %u %u %u %u\n", i, counts[i], bounds[i][0], bounds[i][1], bounds[i][2], bounds[i][3]);
    }
    g_free(hash); g_free(body);
    unlink("/tmp/canopa-frame/frame.rgba");
}

int main(int argc, char **argv) {
    if (argc != 4) return 2;
    const char *mode = argv[1];
    if (strcmp(mode, "cancel") && strcmp(mode, "stop") && strcmp(mode, "exit")) return 2;
    char *end; errno = 0;
    unsigned long forbidden = strtoul(argv[3], &end, 10);
    if (errno || *end || forbidden > UINT32_MAX) return 2;
    setvbuf(stdout, NULL, _IOLBF, 0);
    GKeyFile *info = g_key_file_new();
    if (!g_key_file_load_from_file(info, "/.flatpak-info", 0, NULL)) fatal("not a Flatpak");
    char *identity = g_key_file_get_string(info, "Application", "name", NULL);
    if (!identity || strcmp(identity, APP)) fatal("wrong sandbox identity");
    printf("IDENTITY %s\n", identity); g_free(identity); g_key_file_unref(info);
    struct sockaddr_un address = {.sun_family = AF_UNIX};
    if (strlen(argv[2]) >= sizeof(address.sun_path)) return 2;
    strcpy(address.sun_path, argv[2]);
    int direct = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    if (direct < 0) fatal("probe socket");
    if (!connect(direct, (struct sockaddr *)&address, sizeof(address))) fatal("host PipeWire socket exposed");
    if (errno != ENOENT && errno != EACCES) fatal("unexpected host socket denial");
    close(direct); puts("HOST_PIPEWIRE_DENIED");
    GError *error = NULL;
    bus = g_bus_get_sync(G_BUS_TYPE_SESSION, NULL, &error);
    if (!bus) fatal(error ? error->message : "session bus");
    GVariant *private = g_dbus_connection_call_sync(bus, "org.freedesktop.impl.portal.desktop.canopa", PATH,
        "org.freedesktop.DBus.Properties", "GetAll", g_variant_new("(s)", "org.freedesktop.impl.portal.ScreenCast"),
        NULL, G_DBUS_CALL_FLAGS_NO_AUTO_START, 3000, NULL, &error);
    if (private || !error) fatal("private backend exposed");
    if (g_error_matches(error, G_IO_ERROR, G_IO_ERROR_TIMED_OUT)) fatal("private denial timed out");
    g_clear_error(&error); puts("PRIVATE_BACKEND_DENIED");
    responses = g_hash_table_new_full(g_str_hash, g_str_equal, g_free, (GDestroyNotify)g_variant_unref);
    g_dbus_connection_signal_subscribe(bus, FRONT, "org.freedesktop.portal.Request", "Response", NULL, NULL, 0, response, NULL, NULL);
    g_dbus_connection_signal_subscribe(bus, FRONT, "org.freedesktop.portal.Session", "Closed", NULL, NULL, 0, closed, NULL, NULL);
    GVariantBuilder opts; options(&opts);
    g_variant_builder_add(&opts, "{sv}", "session_handle_token", g_variant_new_string("sandbox"));
    GVariant *values = request("CreateSession", g_variant_new("(a{sv})", &opts), 0);
    const char *borrowed;
    if (!g_variant_lookup(values, "session_handle", "&s", &borrowed)) fatal("session missing");
    session_path = g_strdup(borrowed); g_variant_unref(values);
    options(&opts);
    g_variant_builder_add(&opts, "{sv}", "types", g_variant_new_uint32(2));
    g_variant_builder_add(&opts, "{sv}", "cursor_mode", g_variant_new_uint32(1));
    values = request("SelectSources", g_variant_new("(oa{sv})", session_path, &opts), 0); g_variant_unref(values);
    options(&opts);
    values = request("Start", g_variant_new("(osa{sv})", session_path, "", &opts), !strcmp(mode, "cancel") ? 1 : 0);
    if (!strcmp(mode, "cancel")) {
        if (g_variant_lookup_value(values, "streams", NULL)) fatal("cancel returned streams");
        puts("CANCELLED_NO_STREAMS");
        return 0;
    }
    GVariant *streams = g_variant_lookup_value(values, "streams", G_VARIANT_TYPE("a(ua{sv})"));
    if (!streams || g_variant_n_children(streams) != 1) fatal("one stream required");
    GVariant *properties; uint32_t node; uint64_t serial; guint32 kind;
    g_variant_get_child(streams, 0, "(u@a{sv})", &node, &properties);
    if (!g_variant_lookup(properties, "pipewire-serial", "t", &serial) || !serial
        || !g_variant_lookup(properties, "source_type", "u", &kind) || kind != 2) fatal("stream identity");
    printf("STREAM %u %"PRIu64"\n", node, serial);
    GUnixFDList *descriptors = NULL;
    GVariant *reply = g_dbus_connection_call_with_unix_fd_list_sync(bus, FRONT, PATH, CAST, "OpenPipeWireRemote",
        g_variant_new("(oa{sv})", session_path, NULL), G_VARIANT_TYPE("(h)"), 0, 10000, NULL, &descriptors, NULL, &error);
    if (!reply || !descriptors || g_unix_fd_list_get_length(descriptors) != 1) fatal("remote FD missing");
    gint32 handle; g_variant_get(reply, "(h)", &handle);
    int remote = g_unix_fd_list_get(descriptors, handle, &error);
    if (remote < 0) fatal("remote FD invalid");
    g_object_unref(descriptors); g_variant_unref(reply);
    pw_init(NULL, NULL);
    struct capture capture = {.directory = "/tmp/canopa-frame", .mode = 1, .result = 1};
    if (g_mkdir_with_parents(capture.directory, 0700)) fatal("private frame directory");
    capture.loop = pw_main_loop_new(NULL);
    if (!capture.loop) fatal("PipeWire loop");
    struct pw_context *context = pw_context_new(pw_main_loop_get_loop(capture.loop), NULL, 0);
    struct pw_core *core = context ? pw_context_connect_fd(context, remote, NULL, 0) : NULL;
    if (!core) fatal("restricted core");
    struct visibility visible = {.loop = capture.loop, .selected = node, .forbidden = (uint32_t)forbidden};
    const struct pw_core_events core_events = {.version = PW_VERSION_CORE_EVENTS, .done = roundtrip, .error = core_error};
    const struct pw_registry_events registry_events = {.version = PW_VERSION_REGISTRY_EVENTS, .global = global, .global_remove = global_remove};
    struct spa_hook core_listener, registry_listener;
    pw_core_add_listener(core, &core_listener, &core_events, &visible);
    struct pw_registry *registry = pw_core_get_registry(core, PW_VERSION_REGISTRY, 0);
    pw_registry_add_listener(registry, &registry_listener, &registry_events, &visible);
    struct spa_source *timer = pw_loop_add_timer(pw_main_loop_get_loop(capture.loop), media_deadline, &visible);
    struct timespec limit = {.tv_sec = 15};
    pw_loop_update_timer(pw_main_loop_get_loop(capture.loop), timer, &limit, NULL, false);
    visible.sequence = pw_core_sync(core, PW_ID_CORE, 0);
    pw_main_loop_run(capture.loop);
    if (!visible.done || visible.failed || !visible.selected_seen || visible.forbidden_seen) fatal("restricted registry");
    puts("REGISTRY_RESTRICTED");
    char target[32]; snprintf(target, sizeof(target), "%"PRIu64, serial);
    capture.stream = pw_stream_new(core, "Canopa sandbox frame consumer", pw_properties_new(
        PW_KEY_MEDIA_TYPE, "Video", PW_KEY_MEDIA_CATEGORY, "Capture", PW_KEY_MEDIA_ROLE, "Screen",
        PW_KEY_TARGET_OBJECT, target, NULL));
    struct spa_hook stream_listener;
    pw_stream_add_listener(capture.stream, &stream_listener, &events, &capture);
    uint8_t storage[1024]; struct spa_pod_builder builder = SPA_POD_BUILDER_INIT(storage, sizeof(storage));
    const struct spa_pod *format = spa_pod_builder_add_object(&builder, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat,
        SPA_FORMAT_mediaType, SPA_POD_Id(SPA_MEDIA_TYPE_video), SPA_FORMAT_mediaSubtype, SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw),
        SPA_FORMAT_VIDEO_format, SPA_POD_CHOICE_ENUM_Id(3, SPA_VIDEO_FORMAT_BGRx, SPA_VIDEO_FORMAT_BGRx, SPA_VIDEO_FORMAT_BGRA));
    if (pw_stream_connect(capture.stream, PW_DIRECTION_INPUT, PW_ID_ANY,
        PW_STREAM_FLAG_AUTOCONNECT | PW_STREAM_FLAG_MAP_BUFFERS, &format, 1) < 0) fatal("stream connect");
    pw_main_loop_run(capture.loop);
    if (capture.result || visible.failed) fatal("frame capture");
    pixels(&capture); puts("FRAME_READY");
    if (!strcmp(mode, "exit")) {
        char command[32];
        if (!fgets(command, sizeof(command), stdin) || strcmp(command, "exit\n")) fatal("exit command");
        puts("EXIT_WITHOUT_SESSION_CLOSE");
    } else {
        guint timeout = g_timeout_add_seconds(30, deadline, NULL);
        while (!session_closed) g_main_context_iteration(NULL, TRUE);
        g_source_remove(timeout);
        GVariant *late = g_dbus_connection_call_sync(bus, FRONT, PATH, CAST, "OpenPipeWireRemote",
            g_variant_new("(oa{sv})", session_path, NULL), NULL, 0, 3000, NULL, &error);
        if (late || !error || g_error_matches(error, G_IO_ERROR, G_IO_ERROR_TIMED_OUT)) fatal("closed session remote accepted");
        g_clear_error(&error); puts("CLOSED_REMOTE_DENIED");
    }
    pw_stream_destroy(capture.stream); pw_proxy_destroy((struct pw_proxy *)registry);
    pw_core_disconnect(core); pw_context_destroy(context); pw_main_loop_destroy(capture.loop); pw_deinit();
    g_dbus_connection_close_sync(bus, NULL, NULL);
    return 0;
}

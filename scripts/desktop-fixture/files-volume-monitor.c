/* Private native acceptance provider. No real mounts or device access.
 * Loaded only through the fixture process's GIO_EXTRA_MODULES directory.
 * Implements the public GIO extension point, not application test hooks. */
#include <gio/gio.h>
#include <stdio.h>
#include <string.h>

typedef struct { GObject parent; } FixtureVolume;
typedef struct { GObjectClass parent; } FixtureVolumeClass;
typedef struct { GObject parent; } FixtureMount;
typedef struct { GObjectClass parent; } FixtureMountClass;
typedef struct { GObject parent; } FixtureDrive;
typedef struct { GObjectClass parent; } FixtureDriveClass;
#ifdef CANOPA_FIXTURE_NATIVE_MONITOR
typedef struct { GNativeVolumeMonitor parent; } FixtureMonitor;
typedef struct { GNativeVolumeMonitorClass parent; } FixtureMonitorClass;
#define FIXTURE_MONITOR_TYPE G_TYPE_NATIVE_VOLUME_MONITOR
#define FIXTURE_MONITOR_EXTENSION G_NATIVE_VOLUME_MONITOR_EXTENSION_POINT_NAME
#else
typedef struct { GVolumeMonitor parent; } FixtureMonitor;
typedef struct { GVolumeMonitorClass parent; } FixtureMonitorClass;
#define FIXTURE_MONITOR_TYPE G_TYPE_VOLUME_MONITOR
#define FIXTURE_MONITOR_EXTENSION G_VOLUME_MONITOR_EXTENSION_POINT_NAME
#endif

static void volume_iface(GVolumeIface *iface);
static void mount_iface(GMountIface *iface);
static void drive_iface(GDriveIface *iface);
G_DEFINE_DYNAMIC_TYPE_EXTENDED(FixtureVolume, fixture_volume, G_TYPE_OBJECT, 0,
    G_IMPLEMENT_INTERFACE_DYNAMIC(G_TYPE_VOLUME, volume_iface))
G_DEFINE_DYNAMIC_TYPE_EXTENDED(FixtureMount, fixture_mount, G_TYPE_OBJECT, 0,
    G_IMPLEMENT_INTERFACE_DYNAMIC(G_TYPE_MOUNT, mount_iface))
G_DEFINE_DYNAMIC_TYPE_EXTENDED(FixtureDrive, fixture_drive, G_TYPE_OBJECT, 0,
    G_IMPLEMENT_INTERFACE_DYNAMIC(G_TYPE_DRIVE, drive_iface))
G_DEFINE_DYNAMIC_TYPE(FixtureMonitor, fixture_monitor, FIXTURE_MONITOR_TYPE)

static GVolume *volume;
static GMount *mount_item;
static GDrive *drive_item;
static GVolumeMonitor *monitor;
static gboolean present = TRUE, mounted = TRUE, removed = FALSE;
static char *root;

static void record(const char *event) {
    char *path = g_build_filename(root, "events", NULL);
    FILE *stream = fopen(path, "a");
    if (stream) { fprintf(stream, "%s\n", event); fclose(stream); }
    g_free(path);
}
static gboolean marker(const char *name) {
    char *path = g_build_filename(root, name, NULL);
    gboolean exists = g_file_test(path, G_FILE_TEST_EXISTS);
    g_free(path);
    return exists;
}
static char *name(gpointer object) { (void)object; return g_strdup("Fixture USB"); }
static char *uuid(gpointer object) { (void)object; return marker("no-uuid") ? NULL : g_strdup(marker("replacement-device") ? "canopa-private-replacement" : "canopa-private-volume"); }
static GIcon *icon(gpointer object) { (void)object; return g_themed_icon_new("drive-removable-media-symbolic"); }
static gboolean has_drive(void) { return marker("drive-stop") || marker("drive-eject"); }
static GDrive *drive(gpointer object) { (void)object; return has_drive() ? g_object_ref(drive_item) : NULL; }
static gboolean yes(gpointer object) { (void)object; return TRUE; }
static gboolean no(gpointer object) { (void)object; return FALSE; }
static GFile *location(gpointer object) {
    (void)object;
    char *path = g_build_filename(root, "Fixture USB", NULL);
    GFile *file = g_file_new_for_path(path);
    g_free(path);
    return file;
}
static GMount *get_mount(GVolume *object) { (void)object; return mounted && present ? g_object_ref(mount_item) : NULL; }
static GVolume *get_volume(GMount *object) { (void)object; return g_object_ref(volume); }
static char *identifier(GVolume *object, const char *kind) {
    (void)object;
    return !strcmp(kind, G_VOLUME_IDENTIFIER_KIND_CLASS) ? g_strdup("device") : NULL;
}
static char **identifiers(GVolume *object) {
    (void)object;
    char **values = g_new0(char *, 2);
    values[0] = g_strdup(G_VOLUME_IDENTIFIER_KIND_CLASS);
    return values;
}
static gboolean finish(gpointer object, GAsyncResult *result, GError **error) {
    (void)object;
    return g_task_propagate_boolean(G_TASK(result), error);
}
#define ACCESSORS(prefix, Type) \
    static char *prefix##_name(Type *object) { return name(object); } \
    static char *prefix##_uuid(Type *object) { return uuid(object); } \
    static GIcon *prefix##_icon(Type *object) { return icon(object); } \
    static GDrive *prefix##_drive(Type *object) { return drive(object); } \
    static GFile *prefix##_location(Type *object) { return location(object); } \
    static gboolean prefix##_yes(Type *object) { return yes(object); } \
    static gboolean prefix##_no(Type *object) { return no(object); } \
    static gboolean prefix##_finish(Type *object, GAsyncResult *result, GError **error) { return finish(object, result, error); }
ACCESSORS(volume, GVolume)
ACCESSORS(mount, GMount)
static gboolean complete_mount(gpointer data) {
    GTask *task = data;
    if (g_task_return_error_if_cancelled(task)) {
        record("mount cancelled");
        g_object_unref(task);
        return G_SOURCE_REMOVE;
    }
    if (marker("hold-mount") && !marker("release-mount")) return G_SOURCE_CONTINUE;
    if (!present) {
        g_task_return_new_error(task, G_IO_ERROR, G_IO_ERROR_NOT_FOUND, "Fixture device disconnected");
    } else {
        mounted = TRUE;
        g_signal_emit_by_name(monitor, "mount-added", mount_item);
        g_signal_emit_by_name(monitor, "volume-changed", volume);
        record("mount completed");
        g_task_return_boolean(task, TRUE);
    }
    g_object_unref(task);
    return G_SOURCE_REMOVE;
}
static gboolean complete_unmount(gpointer data) {
    GTask *task = data;
    if (g_task_return_error_if_cancelled(task)) {
        record("unmount cancelled");
        g_object_unref(task);
        return G_SOURCE_REMOVE;
    }
    if (marker("hold-unmount") && !marker("release-unmount")) return G_SOURCE_CONTINUE;
    mounted = FALSE;
    g_signal_emit_by_name(monitor, "mount-removed", mount_item);
    g_signal_emit_by_name(g_task_get_source_object(task), "unmounted");
    g_signal_emit_by_name(monitor, "volume-changed", volume);
    record("unmount completed");
    g_task_return_boolean(task, TRUE);
    g_object_unref(task);
    return G_SOURCE_REMOVE;
}
static void mount_reply(GMountOperation *operation, GMountOperationResult result, gpointer data) {
    GTask *task = data;
    g_signal_handlers_disconnect_by_data(operation, task);
    if (result != G_MOUNT_OPERATION_HANDLED) {
        record("mount interaction cancelled");
        g_task_return_new_error(task, G_IO_ERROR, G_IO_ERROR_CANCELLED, "Fixture mount interaction cancelled");
        g_object_unref(task);
        return;
    }
    if (marker("password-mount")) {
        if (g_strcmp0(g_mount_operation_get_username(operation), "private-user") ||
            g_strcmp0(g_mount_operation_get_domain(operation), "private-domain") ||
            g_strcmp0(g_mount_operation_get_password(operation), "private-secret") ||
            g_mount_operation_get_password_save(operation) != G_PASSWORD_SAVE_NEVER) {
            g_task_return_new_error(task, G_IO_ERROR, G_IO_ERROR_PERMISSION_DENIED, "Fixture credentials refused");
            g_object_unref(task);
            return;
        }
        record("credentials accepted without saving");
    } else if (marker("question-mount") && g_mount_operation_get_choice(operation) != 1) {
        g_task_return_new_error(task, G_IO_ERROR, G_IO_ERROR_CANCELLED, "Fixture question cancelled");
        g_object_unref(task);
        return;
    }
    complete_mount(task);
}
static gboolean current_question(gpointer data) {
    GTask *task = data;
    GCancellable *cancel = g_task_get_cancellable(task);
    if (g_task_get_completed(task) || (cancel && g_cancellable_is_cancelled(cancel))) return G_SOURCE_REMOVE;
    if (!marker("release-question")) return G_SOURCE_CONTINUE;
    GMountOperation *operation = g_object_get_data(G_OBJECT(task), "mount-operation");
    const char *choices[] = { "Cancel", "Continue", NULL };
    g_signal_emit_by_name(operation, "ask-question", "Current fixture question", choices);
    record("current question offered");
    return G_SOURCE_REMOVE;
}
static void mount_async(GVolume *object, GMountMountFlags flags, GMountOperation *operation,
                        GCancellable *cancel, GAsyncReadyCallback callback, gpointer data) {
    (void)flags;
    GTask *task = g_task_new(object, cancel, callback, data);
    record("mount requested");
    if (marker("fail-mount")) {
        g_task_return_new_error(task, G_IO_ERROR, G_IO_ERROR_PERMISSION_DENIED, "Fixture mount refused");
        g_object_unref(task);
    } else if (marker("password-mount") || marker("question-mount")) {
        g_assert(operation);
        g_object_set_data_full(G_OBJECT(task), "mount-operation", g_object_ref(operation), g_object_unref);
        g_signal_connect(operation, "reply", G_CALLBACK(mount_reply), task);
        if (marker("password-mount")) {
            g_signal_emit_by_name(operation, "ask-password", "Unlock private fixture", "", "",
                G_ASK_PASSWORD_NEED_PASSWORD | G_ASK_PASSWORD_NEED_USERNAME | G_ASK_PASSWORD_NEED_DOMAIN);
        } else {
            const char *old[] = { "Cancel", "Old choice", NULL };
            const char *current[] = { "Cancel", "Continue", NULL };
            g_signal_emit_by_name(operation, "ask-question", "Old fixture question", old);
            if (marker("hold-question")) {
                g_timeout_add_full(G_PRIORITY_DEFAULT, 30, current_question, g_object_ref(task), g_object_unref);
            } else {
                g_signal_emit_by_name(operation, "ask-question", "Current fixture question", current);
            }
        }
    } else if (marker("hold-mount")) {
        g_timeout_add(30, complete_mount, task);
    } else {
        complete_mount(task);
    }
}
static void unmount_async(GMount *object, GMountUnmountFlags flags, GMountOperation *operation,
                          GCancellable *cancel, GAsyncReadyCallback callback, gpointer data) {
    (void)operation;
    GTask *task = g_task_new(object, cancel, callback, data);
    record(flags == G_MOUNT_UNMOUNT_NONE ? "unmount requested without force" : "UNEXPECTED forced unmount");
    if (marker("fail-unmount")) {
        g_task_return_new_error(task, G_IO_ERROR, G_IO_ERROR_BUSY, "Fixture volume is busy");
        g_object_unref(task);
    } else if (marker("hold-unmount")) {
        g_timeout_add(30, complete_unmount, task);
    } else {
        complete_unmount(task);
    }
}
static gboolean complete_drive(gpointer data) {
    GTask *task = data;
    const char *action = g_object_get_data(G_OBJECT(task), "drive-action");
    char *event;
    if (g_task_return_error_if_cancelled(task)) {
        event = g_strdup_printf("%s cancelled", action);
    } else if (marker("hold-drive") && !marker("release-drive")) {
        return G_SOURCE_CONTINUE;
    } else if (!present) {
        event = g_strdup_printf("%s disconnected", action);
        g_task_return_new_error(task, G_IO_ERROR, G_IO_ERROR_NOT_FOUND, "Fixture drive disconnected");
    } else {
        gboolean was_mounted = mounted;
        removed = TRUE;
        present = FALSE;
        mounted = FALSE;
        if (was_mounted) {
            g_signal_emit_by_name(monitor, "mount-removed", mount_item);
            g_signal_emit_by_name(mount_item, "unmounted");
        }
        g_signal_emit_by_name(monitor, "volume-removed", volume);
        g_signal_emit_by_name(volume, "removed");
        g_signal_emit_by_name(monitor, "drive-disconnected", drive_item);
        g_signal_emit_by_name(drive_item, "disconnected");
        event = g_strdup_printf("%s completed", action);
        g_task_return_boolean(task, TRUE);
    }
    record(event);
    g_free(event);
    g_object_unref(task);
    return G_SOURCE_REMOVE;
}
static void drive_remove(GDrive *object, GMountUnmountFlags flags, GMountOperation *operation,
                         GCancellable *cancel, GAsyncReadyCallback callback, gpointer data,
                         const char *action) {
    (void)operation;
    GTask *task = g_task_new(object, cancel, callback, data);
    g_object_set_data(G_OBJECT(task), "drive-action", (gpointer)action);
    char *event = g_strdup_printf(flags == G_MOUNT_UNMOUNT_NONE ? "%s requested without force" : "UNEXPECTED forced %s", action);
    record(event);
    g_free(event);
    if (marker("fail-drive")) {
        g_task_return_new_error(task, G_IO_ERROR, G_IO_ERROR_BUSY, "Fixture drive is busy");
        g_object_unref(task);
    } else if (marker("hold-drive")) {
        g_timeout_add(30, complete_drive, task);
    } else {
        complete_drive(task);
    }
}
static void stop_async(GDrive *object, GMountUnmountFlags flags, GMountOperation *operation,
                       GCancellable *cancel, GAsyncReadyCallback callback, gpointer data) {
    drive_remove(object, flags, operation, cancel, callback, data, "stop");
}
static void eject_async(GDrive *object, GMountUnmountFlags flags, GMountOperation *operation,
                        GCancellable *cancel, GAsyncReadyCallback callback, gpointer data) {
    drive_remove(object, flags, operation, cancel, callback, data, "eject");
}
static char *drive_name(GDrive *object) { return name(object); }
static GIcon *drive_icon(GDrive *object) { return icon(object); }
static gboolean drive_present(GDrive *object) { (void)object; return present; }
static gboolean drive_no(GDrive *object) { return no(object); }
static gboolean drive_can_stop(GDrive *object) { (void)object; return marker("drive-stop"); }
static gboolean drive_finish(GDrive *object, GAsyncResult *result, GError **error) { return finish(object, result, error); }
static GList *drive_volumes(GDrive *object) { (void)object; return present ? g_list_append(NULL, g_object_ref(volume)) : NULL; }
static GDriveStartStopType drive_start_stop_type(GDrive *object) { (void)object; return G_DRIVE_START_STOP_TYPE_SHUTDOWN; }
static void drive_iface(GDriveIface *iface) {
    iface->get_name = drive_name;
    iface->get_icon = drive_icon;
    iface->get_symbolic_icon = drive_icon;
    iface->has_volumes = drive_present;
    iface->get_volumes = drive_volumes;
    iface->is_media_removable = drive_present;
    iface->is_removable = drive_present;
    iface->has_media = drive_present;
    iface->is_media_check_automatic = drive_present;
    iface->can_poll_for_media = drive_no;
    iface->can_start = drive_no;
    iface->can_start_degraded = drive_no;
    iface->can_stop = drive_can_stop;
    /* Power-off fixtures also support eject, proving the preferred capability. */
    iface->can_eject = drive_present;
    iface->get_start_stop_type = drive_start_stop_type;
    iface->stop = stop_async;
    iface->stop_finish = drive_finish;
    iface->eject_with_operation = eject_async;
    iface->eject_with_operation_finish = drive_finish;
}
static void volume_iface(GVolumeIface *iface) {
    iface->get_name = volume_name;
    iface->get_uuid = volume_uuid;
    iface->get_icon = volume_icon;
    iface->get_symbolic_icon = volume_icon;
    iface->get_drive = volume_drive;
    iface->get_mount = get_mount;
    iface->can_mount = volume_yes;
    iface->can_eject = volume_no;
    iface->get_identifier = identifier;
    iface->enumerate_identifiers = identifiers;
    iface->should_automount = volume_no;
    iface->get_activation_root = volume_location;
    iface->mount_fn = mount_async;
    iface->mount_finish = volume_finish;
}
static void mount_iface(GMountIface *iface) {
    iface->get_root = mount_location;
    iface->get_default_location = mount_location;
    iface->get_name = mount_name;
    iface->get_uuid = mount_uuid;
    iface->get_icon = mount_icon;
    iface->get_symbolic_icon = mount_icon;
    iface->get_drive = mount_drive;
    iface->get_volume = get_volume;
    iface->can_unmount = mount_yes;
    iface->can_eject = mount_no;
    iface->unmount_with_operation = unmount_async;
    iface->unmount_with_operation_finish = mount_finish;
}
static gboolean supported(void) { return g_getenv("CANOPA_FIXTURE_VOLUMES") != NULL; }
static GList *volumes(GVolumeMonitor *object) { (void)object; return present ? g_list_append(NULL, g_object_ref(volume)) : NULL; }
static GList *mounts(GVolumeMonitor *object) { (void)object; return present && mounted ? g_list_append(NULL, g_object_ref(mount_item)) : NULL; }
static GList *drives(GVolumeMonitor *object) { (void)object; return present && has_drive() ? g_list_append(NULL, g_object_ref(drive_item)) : NULL; }
static gboolean update(gpointer data) {
    (void)data;
    gboolean available = !marker("unplugged") && !removed;
    if (available != present) {
        present = available;
        if (present) {
            if (has_drive()) g_signal_emit_by_name(monitor, "drive-connected", drive_item);
            g_signal_emit_by_name(monitor, "volume-added", volume);
            if (mounted) g_signal_emit_by_name(monitor, "mount-added", mount_item);
        } else {
            if (mounted) g_signal_emit_by_name(monitor, "mount-removed", mount_item);
            g_signal_emit_by_name(monitor, "volume-removed", volume);
            g_signal_emit_by_name(volume, "removed");
            if (has_drive()) {
                g_signal_emit_by_name(monitor, "drive-disconnected", drive_item);
                g_signal_emit_by_name(drive_item, "disconnected");
            }
            if (marker("disconnect-unmount")) mounted = FALSE;
        }
        record(present ? "device reconnected" : "device disconnected");
    }
    return G_SOURCE_CONTINUE;
}
static void fixture_monitor_class_init(FixtureMonitorClass *class) {
    GVolumeMonitorClass *vtable = G_VOLUME_MONITOR_CLASS(class);
    vtable->is_supported = supported;
    vtable->get_volumes = volumes;
    vtable->get_mounts = mounts;
    vtable->get_connected_drives = drives;
}
static void fixture_monitor_init(FixtureMonitor *self) {
    root = g_strdup(g_getenv("CANOPA_FIXTURE_VOLUMES"));
    g_assert(root && g_path_is_absolute(root) && strstr(root, "/canopa-files-native-"));
    monitor = G_VOLUME_MONITOR(self);
    volume = g_object_new(fixture_volume_get_type(), NULL);
    mount_item = g_object_new(fixture_mount_get_type(), NULL);
    drive_item = g_object_new(fixture_drive_get_type(), NULL);
    mounted = !marker("unmounted");
    present = !marker("unplugged");
    record("private provider initialized");
    g_timeout_add(100, update, NULL);
}
static void fixture_volume_init(FixtureVolume *self) { (void)self; }
static void fixture_volume_class_init(FixtureVolumeClass *class) { (void)class; }
static void fixture_volume_class_finalize(FixtureVolumeClass *class) { (void)class; }
static void fixture_mount_init(FixtureMount *self) { (void)self; }
static void fixture_mount_class_init(FixtureMountClass *class) { (void)class; }
static void fixture_mount_class_finalize(FixtureMountClass *class) { (void)class; }
static void fixture_drive_init(FixtureDrive *self) { (void)self; }
static void fixture_drive_class_init(FixtureDriveClass *class) { (void)class; }
static void fixture_drive_class_finalize(FixtureDriveClass *class) { (void)class; }
static void fixture_monitor_class_finalize(FixtureMonitorClass *class) { (void)class; }

G_MODULE_EXPORT void g_io_module_load(GIOModule *module) {
    fixture_volume_register_type(G_TYPE_MODULE(module));
    fixture_mount_register_type(G_TYPE_MODULE(module));
    fixture_drive_register_type(G_TYPE_MODULE(module));
    fixture_monitor_register_type(G_TYPE_MODULE(module));
    g_io_extension_point_implement(FIXTURE_MONITOR_EXTENSION,
        fixture_monitor_get_type(), "canopa-private-volume-fixture", 1000);
}
G_MODULE_EXPORT void g_io_module_unload(GIOModule *module) { (void)module; }
G_MODULE_EXPORT char **g_io_module_query(void) {
    char **points = g_new0(char *, 2);
    points[0] = g_strdup(FIXTURE_MONITOR_EXTENSION);
    return points;
}

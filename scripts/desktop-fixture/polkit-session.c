/* Native-session test double. Credentials and receipts stay on a private test bus. */
#define POLKIT_AGENT_I_KNOW_API_IS_SUBJECT_TO_CHANGE
#include <polkitagent/polkitagent.h>
#include <unistd.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct { GObject parent; char *cookie,*root,*mode; guint uid,step; gboolean done; GSource *source; } FixtureSession;
typedef struct { GObjectClass parent; } FixtureSessionClass;
G_DEFINE_TYPE(FixtureSession,fixture_session,G_TYPE_OBJECT)
static void record(FixtureSession *s,const char *event) {
    char *path=g_build_filename(s->root,"polkit-session-events",NULL);FILE *f=fopen(path,"a");
    if(f){fprintf(f,"%s %u\n",event,s->uid);fclose(f);}g_free(path);
}
static void source_stop(FixtureSession *s) { if(s->source){g_source_destroy(s->source);g_source_unref(s->source);s->source=NULL;} }
static void finalize(GObject *object) {
    FixtureSession *s=(FixtureSession *)object;source_stop(s);g_free(s->cookie);g_free(s->root);g_free(s->mode);
    G_OBJECT_CLASS(fixture_session_parent_class)->finalize(object);
}
static void fixture_session_init(FixtureSession *s G_GNUC_UNUSED) {}
static void fixture_session_class_init(FixtureSessionClass *klass) {
    G_OBJECT_CLASS(klass)->finalize=finalize;
    g_signal_new("request",G_TYPE_FROM_CLASS(klass),G_SIGNAL_RUN_LAST,0,NULL,NULL,NULL,G_TYPE_NONE,2,G_TYPE_STRING,G_TYPE_BOOLEAN);
    g_signal_new("show-info",G_TYPE_FROM_CLASS(klass),G_SIGNAL_RUN_LAST,0,NULL,NULL,NULL,G_TYPE_NONE,1,G_TYPE_STRING);
    g_signal_new("show-error",G_TYPE_FROM_CLASS(klass),G_SIGNAL_RUN_LAST,0,NULL,NULL,NULL,G_TYPE_NONE,1,G_TYPE_STRING);
    g_signal_new("completed",G_TYPE_FROM_CLASS(klass),G_SIGNAL_RUN_LAST,0,NULL,NULL,NULL,G_TYPE_NONE,1,G_TYPE_BOOLEAN);
}
PolkitAgentSession *polkit_agent_session_new(PolkitIdentity *identity,const gchar *cookie) {
    const char *root=getenv("CANOPA_POLKIT_FIXTURE_ROOT");const char *address=getenv("DBUS_SYSTEM_BUS_ADDRESS");
    if(!root || !address || strstr(address,"/run/dbus/system_bus_socket")) _exit(95);
    if (getenv("CANOPA_POLKIT_ASSERT_ENV") && (getenv("POLKIT_DEBUG") || getenv("GREETD_SOCK") || getenv("NOTIFY_SOCKET"))) _exit(97);
    FixtureSession *s=g_object_new(fixture_session_get_type(),NULL);s->root=g_strdup(root);s->cookie=g_strdup(cookie);
    s->uid=(guint)polkit_unix_user_get_uid(POLKIT_UNIX_USER(identity));
    char *path=g_build_filename(root,"polkit-mode",NULL);g_file_get_contents(path,&s->mode,NULL,NULL);g_free(path);
    if(!s->mode)s->mode=g_strdup("password");record(s,"new");return (PolkitAgentSession *)s;
}
static void receipt(FixtureSession *s) {
    GError *error=NULL;GDBusConnection *bus=g_bus_get_sync(G_BUS_TYPE_SYSTEM,NULL,&error);
    GVariantBuilder builder;g_variant_builder_init(&builder,G_VARIANT_TYPE("a{sv}"));g_variant_builder_add(&builder,"{sv}","uid",g_variant_new_uint32(s->uid));
    GVariant *result=bus?g_dbus_connection_call_sync(bus,"org.freedesktop.PolicyKit1","/org/freedesktop/PolicyKit1/Authority","org.freedesktop.PolicyKit1.Authority","AuthenticationAgentResponse2",g_variant_new("(us(s@a{sv}))",(guint)getuid(),s->cookie,"unix-user",g_variant_builder_end(&builder)),NULL,G_DBUS_CALL_FLAGS_NONE,1000,NULL,&error):NULL;
    if(error){g_error_free(error);_exit(96);}if(result)g_variant_unref(result);if(bus)g_object_unref(bus);
    s->done=TRUE;record(s,"receipt");g_signal_emit_by_name(s,"completed",TRUE);
}
static gboolean touch(gpointer data) {
    FixtureSession *s=data;char *path=g_build_filename(s->root,"polkit-touch",NULL);gboolean exists=g_file_test(path,G_FILE_TEST_EXISTS);g_free(path);
    if(!exists)return G_SOURCE_CONTINUE;
    g_object_ref(s);source_stop(s);if(!s->done)receipt(s);g_object_unref(s);return G_SOURCE_REMOVE;
}
void polkit_agent_session_initiate(PolkitAgentSession *session) {
    FixtureSession *s=(FixtureSession *)session;record(s,"initiate");
    if(!strcmp(s->mode,"device")){
        g_signal_emit_by_name(s,"show-info","Touch your security key");
        s->source=g_timeout_source_new(25);g_source_set_callback(s->source,touch,s,NULL);g_source_attach(s->source,g_main_context_get_thread_default());
    }else g_signal_emit_by_name(s,"request",s->step?"One-time code:":"Password:",s->step!=0);
}
void polkit_agent_session_response(PolkitAgentSession *session,const gchar *response) {
    FixtureSession *s=(FixtureSession *)session;if(s->done)return;
    gboolean accepted=!strcmp(response,s->step?"123456":"canopa-fixture-password");record(s,accepted?"accepted":"rejected");
    if(!accepted){s->done=TRUE;g_signal_emit_by_name(s,"show-error","Authentication failed");g_signal_emit_by_name(s,"completed",FALSE);return;}
    if(!strcmp(s->mode,"otp") && s->step==0){s->step=1;g_signal_emit_by_name(s,"show-info","Enter a fresh one-time code");g_signal_emit_by_name(s,"request","One-time code:",TRUE);return;}
    receipt(s);
}
static gboolean late(gpointer data){FixtureSession *s=data;record(s,"late");g_signal_emit_by_name(s,"completed",TRUE);return G_SOURCE_REMOVE;}
void polkit_agent_session_cancel(PolkitAgentSession *session) {
    FixtureSession *s=(FixtureSession *)session;source_stop(s);if(s->done)return;s->done=TRUE;record(s,"cancel");
    if(!strcmp(s->mode,"blocked-cancel")){sleep(60);return;}
    if(!strcmp(s->mode,"late")){GSource *source=g_timeout_source_new(100);g_source_set_callback(source,late,g_object_ref(s),g_object_unref);g_source_attach(source,g_main_context_get_thread_default());g_source_unref(source);}
    g_signal_emit_by_name(s,"completed",FALSE);
}

/* Canopa's native polkit adapter. Policy and credential verification stay in polkit. */
#define POLKIT_AGENT_I_KNOW_API_IS_SUBJECT_TO_CHANGE
#include <polkitagent/polkitagent.h>
#include <glib-unix.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>

typedef struct {
    uint32_t kind;
    uint64_t generation, prompt;
    const char *action, *message, *text;
    const uint32_t *uids;
    size_t count;
    uint32_t selected, echo, subject_pid;
} Message;
typedef void (*Emit)(const Message *, void *);
typedef struct _CanopaAgent {
    PolkitAgentListener parent;
    GMainContext *context;
    GMainLoop *loop;
    GDBusConnection *bus;
    Emit emit;
    void *data;
    GTask *task;
    GCancellable *cancel;
    gulong cancel_id;
    PolkitAgentSession *session;
    PolkitIdentity *identities[32];
    uint32_t uids[32], count, selected, attempts;
    uint64_t generation, prompt;
    gboolean awaiting, ending, ready, stopping;
    gint64 retry_after;
    char *cookie;
    guint deadline;
    uint32_t timeout;
    GMutex owner_lock;
    char *owner;
    int result;
} CanopaAgent;
typedef struct { PolkitAgentListenerClass parent; } CanopaAgentClass;
G_DEFINE_TYPE(CanopaAgent, canopa_agent, POLKIT_AGENT_TYPE_LISTENER)
static GMutex global_lock;
static CanopaAgent *global_agent;
static void erase(char *value, size_t length) {
    volatile unsigned char *p=(volatile unsigned char *)value;
    for (size_t i=0;i<length;i++) p[i]=0;
}

static void emit(CanopaAgent *a, uint32_t kind, const char *text, gboolean echo) {
    Message m = { .kind=kind, .generation=a->generation, .prompt=a->prompt,
                  .text=text ? text : "", .uids=a->uids, .count=a->count,
                  .selected=a->selected, .echo=echo };
    a->emit(&m, a->data);
}
static void stop_session(CanopaAgent *a) {
    a->awaiting=FALSE;
    if (a->session) {
        PolkitAgentSession *s=a->session;
        a->session=NULL;
        g_signal_handlers_disconnect_by_data(s,a);
        polkit_agent_session_cancel(s);
        g_object_unref(s);
    }
}
static void finish(CanopaAgent *a, gboolean finished) {
    if (!a->task || a->ending) return;
    a->ending=TRUE;
    if (a->deadline) { GSource *s=g_main_context_find_source_by_id(a->context,a->deadline); if (s) g_source_destroy(s); a->deadline=0; }
    if (a->cancel_id) { g_signal_handler_disconnect(a->cancel,a->cancel_id); a->cancel_id=0; }
    stop_session(a);
    if (finished) g_task_return_boolean(a->task,TRUE);
    else g_task_return_new_error(a->task,POLKIT_ERROR,POLKIT_ERROR_CANCELLED,"Authentication cancelled");
    g_clear_object(&a->task); g_clear_object(&a->cancel);
    for (uint32_t i=0;i<a->count;i++) g_clear_object(&a->identities[i]);
    a->count=0;
    if (a->cookie) { erase(a->cookie,strlen(a->cookie)); g_clear_pointer(&a->cookie,g_free); }
    emit(a,6,"",FALSE);
    a->ending=FALSE;
}
static void cancelled(GCancellable *cancel G_GNUC_UNUSED, gpointer data) { finish(data,FALSE); }
static gboolean expired(gpointer data) { CanopaAgent *a=data; a->deadline=0; finish(a,FALSE); return G_SOURCE_REMOVE; }
static gboolean valid_text(const char *s, size_t maximum) {
    return s && strlen(s)<=maximum && g_utf8_validate(s,-1,NULL);
}
typedef struct {
    GWeakRef agent;
    uint64_t generation;
    char *action;
} PolicyLookup;

static void policy_lookup_done(GObject *bus, GAsyncResult *result, gpointer data) {
    PolicyLookup *lookup=data;
    GVariant *reply=g_dbus_connection_call_finish(G_DBUS_CONNECTION(bus),result,NULL);
    CanopaAgent *a=g_weak_ref_get(&lookup->agent);
    if (reply && a && !a->stopping && a->task && a->generation==lookup->generation) {
        GVariant *actions=g_variant_get_child_value(reply,0);
        GVariantIter iter;
        g_variant_iter_init(&iter,actions);
        GVariant *entry;
        while ((entry=g_variant_iter_next_value(&iter))) {
            const char *id, *publisher;
            g_variant_get_child(entry,0,"&s",&id);
            gboolean match=g_str_equal(id,lookup->action);
            if (match) {
                g_variant_get_child(entry,3,"&s",&publisher);
                if (valid_text(publisher,96) && *publisher) emit(a,11,publisher,FALSE);
            }
            g_variant_unref(entry);
            if (match) break;
        }
        g_variant_unref(actions);
    }
    g_clear_pointer(&reply,g_variant_unref);
    g_clear_object(&a);
    g_weak_ref_clear(&lookup->agent);
    g_free(lookup->action);
    g_free(lookup);
}

/* Optional presentation context. A slow or missing catalog cannot delay PAM. */
static void lookup_policy_publisher(CanopaAgent *a, const char *action) {
    if (!a->owner) return;
    PolicyLookup *lookup=g_new0(PolicyLookup,1);
    g_weak_ref_init(&lookup->agent,a);
    lookup->generation=a->generation;
    lookup->action=g_strdup(action);
    g_dbus_connection_call(a->bus,a->owner,"/org/freedesktop/PolicyKit1/Authority",
        "org.freedesktop.PolicyKit1.Authority","EnumerateActions",g_variant_new("(s)",""),
        G_VARIANT_TYPE("(a(ssssssuuua{ss}))"),G_DBUS_CALL_FLAGS_NO_AUTO_START,500,
        a->cancel,policy_lookup_done,lookup);
}
static void request(PolkitAgentSession *s, const char *text, gboolean echo, gpointer data) {
    CanopaAgent *a=data;
    if (s!=a->session || !a->task) return;
    if (!valid_text(text,2048)) { finish(a,FALSE); return; }
    a->prompt++; a->awaiting=TRUE; emit(a,2,text,echo);
}
static void info(PolkitAgentSession *s, const char *text, gpointer data) {
    CanopaAgent *a=data;
    if (s==a->session && a->task && valid_text(text,2048)) emit(a,3,text,FALSE);
}
static void error(PolkitAgentSession *s, const char *text, gpointer data) {
    CanopaAgent *a=data;
    if (s==a->session && a->task && valid_text(text,2048)) emit(a,4,text,FALSE);
}
static void completed(PolkitAgentSession *s, gboolean gained, gpointer data) {
    CanopaAgent *a=data;
    if (s!=a->session || !a->task) return;
    if (gained) { finish(a,TRUE); return; }
    stop_session(a); a->attempts++;
    /* A completed conversation is not an authorization receipt. Let the
     * authority report denial when all attempts failed without a receipt. */
    if (a->attempts>=3) { finish(a,TRUE); return; }
    a->retry_after=g_get_monotonic_time()+2*G_USEC_PER_SEC;
    emit(a,5,"Authentication failed. Try again.",FALSE);
}
static void start_session(CanopaAgent *a, uint32_t selected) {
    if (!a->task || selected>=a->count || g_get_monotonic_time()<a->retry_after) return;
    stop_session(a); a->selected=selected; a->prompt++;
    emit(a,7,"Waiting for authentication…",FALSE);
    a->session=polkit_agent_session_new(a->identities[selected],a->cookie);
    if (!a->session) { finish(a,FALSE); return; }
    g_signal_connect(a->session,"request",G_CALLBACK(request),a);
    g_signal_connect(a->session,"show-info",G_CALLBACK(info),a);
    g_signal_connect(a->session,"show-error",G_CALLBACK(error),a);
    g_signal_connect(a->session,"completed",G_CALLBACK(completed),a);
    polkit_agent_session_initiate(a->session);
}
static void begin(PolkitAgentListener *listener, const gchar *action, const gchar *message,
                  const gchar *icon G_GNUC_UNUSED, PolkitDetails *details,
                  const gchar *cookie, GList *identities, GCancellable *cancel,
                  GAsyncReadyCallback callback, gpointer user_data) {
    CanopaAgent *a=(CanopaAgent *)listener;
    GTask *task=g_task_new(listener,cancel,callback,user_data);
    if (a->stopping || a->task || !valid_text(action,256) || !*action || !valid_text(message,512) ||
        !valid_text(cookie,4096) || !*cookie || g_list_length(identities)>32) {
        g_task_return_new_error(task,POLKIT_ERROR,POLKIT_ERROR_CANCELLED,"Request unavailable");
        g_object_unref(task); return;
    }
    a->count=0; a->selected=0;
    for (GList *p=identities;p;p=p->next) {
        if (!POLKIT_IS_UNIX_USER(p->data)) continue;
        gint uid=polkit_unix_user_get_uid(POLKIT_UNIX_USER(p->data));
        if (uid<0) continue;
        gboolean duplicate=FALSE;
        for (uint32_t i=0;i<a->count;i++) if (a->uids[i]==(uint32_t)uid) duplicate=TRUE;
        if (duplicate) continue;
        a->uids[a->count]=(uint32_t)uid; a->identities[a->count]=g_object_ref(p->data);
        if ((uid_t)uid==getuid()) a->selected=a->count;
        a->count++;
    }
    if (!a->count) {
        g_task_return_new_error(task,POLKIT_ERROR,POLKIT_ERROR_FAILED,"No supported identities");
        g_object_unref(task); return;
    }
    a->generation++; a->prompt=0; a->attempts=0; a->retry_after=0;
    a->task=task; a->cookie=g_strdup(cookie); a->cancel=cancel ? g_object_ref(cancel) : g_cancellable_new();
    a->cancel_id=g_signal_connect(a->cancel,"cancelled",G_CALLBACK(cancelled),a);
    GSource *timer=g_timeout_source_new_seconds(a->timeout);
    g_source_set_callback(timer,expired,a,NULL); a->deadline=g_source_attach(timer,a->context); g_source_unref(timer);
    uint32_t subject_pid=0;
    const char *pid=details ? polkit_details_lookup(details,"polkit.subject-pid") : NULL;
    if (pid && *pid && strnlen(pid,11)<=10 && strspn(pid,"0123456789")==strlen(pid)) {
        guint64 value=g_ascii_strtoull(pid,NULL,10);
        if (value<=INT32_MAX) subject_pid=(uint32_t)value;
    }
    Message m={ .kind=1,.generation=a->generation,.action=action,.message=message,
                .uids=a->uids,.count=a->count,.selected=a->selected,.subject_pid=subject_pid };
    a->emit(&m,a->data);
    if (g_cancellable_is_cancelled(a->cancel)) finish(a,FALSE);
    else {
        start_session(a,a->selected);
        if (a->task) lookup_policy_publisher(a,action);
    }
}
static gboolean begin_finish(PolkitAgentListener *listener G_GNUC_UNUSED, GAsyncResult *result, GError **error_out) {
    return g_task_propagate_boolean(G_TASK(result),error_out);
}
static void canopa_agent_init(CanopaAgent *a) { g_mutex_init(&a->owner_lock); }
static void finalize(GObject *object) {
    CanopaAgent *a=(CanopaAgent *)object;
    g_free(a->owner); g_mutex_clear(&a->owner_lock);
    g_main_loop_unref(a->loop); g_main_context_unref(a->context);
    G_OBJECT_CLASS(canopa_agent_parent_class)->finalize(object);
}
static void canopa_agent_class_init(CanopaAgentClass *klass) {
    G_OBJECT_CLASS(klass)->finalize=finalize;
    PolkitAgentListenerClass *parent=POLKIT_AGENT_LISTENER_CLASS(klass);
    parent->initiate_authentication=begin; parent->initiate_authentication_finish=begin_finish;
}

/* The library normally relies on system-bus UID-0 policy. Also pin calls to the
 * authority's unique owner, including on private test buses without that policy. */
static GDBusMessage *filter(GDBusConnection *bus, GDBusMessage *message, gboolean incoming, gpointer data) {
    CanopaAgent *a=data;
    if (incoming && g_dbus_message_get_message_type(message)==G_DBUS_MESSAGE_TYPE_METHOD_CALL &&
        g_strcmp0(g_dbus_message_get_path(message),"/org/canopa/PolicyKit1/AuthenticationAgent")==0) {
        g_mutex_lock(&a->owner_lock);
        gboolean allowed=a->owner && g_strcmp0(a->owner,g_dbus_message_get_sender(message))==0;
        g_mutex_unlock(&a->owner_lock);
        GVariant *body=g_dbus_message_get_body(message);
        if (!allowed || (body && g_variant_get_size(body)>65536)) {
            GDBusMessage *reply=g_dbus_message_new_method_error_literal(message,"org.freedesktop.DBus.Error.AccessDenied","Request refused");
            g_dbus_connection_send_message(bus,reply,G_DBUS_SEND_MESSAGE_FLAGS_NONE,NULL,NULL);
            g_object_unref(reply); g_object_unref(message); return NULL;
        }
    }
    return message;
}
static void owner_changed(GObject *authority, GParamSpec *property G_GNUC_UNUSED, gpointer data) {
    CanopaAgent *a=data;
    char *owner=polkit_authority_get_owner(POLKIT_AUTHORITY(authority));
    g_mutex_lock(&a->owner_lock);
    gboolean changed=g_strcmp0(owner,a->owner)!=0;
    g_free(a->owner); a->owner=owner;
    g_mutex_unlock(&a->owner_lock);
    if (changed && a->ready) { a->stopping=TRUE; a->result=2; emit(a,9,"Authorization service changed",FALSE); finish(a,FALSE); g_main_loop_quit(a->loop); }
}
static gboolean stop_signal(gpointer data) { CanopaAgent *a=data; a->stopping=TRUE; emit(a,10,"",FALSE); finish(a,FALSE); g_main_loop_quit(a->loop); return G_SOURCE_CONTINUE; }
static void bus_closed(GDBusConnection *bus G_GNUC_UNUSED, gboolean remote G_GNUC_UNUSED, GError *error_value G_GNUC_UNUSED, gpointer data) {
    CanopaAgent *a=data; a->result=2; emit(a,9,"System bus disconnected",FALSE); stop_signal(a);
}

typedef struct {
    CanopaAgent *agent;
    uint64_t generation,prompt;
    uint32_t action,index;
    char response[1024];
} Dispatch;
static void dispatch_free(gpointer data) {
    Dispatch *d=data; g_object_unref(d->agent);
    volatile unsigned char *p=(volatile unsigned char *)d;
    for (size_t i=0;i<sizeof(*d);i++) p[i]=0;
    g_free(d);
}
static gboolean dispatch(gpointer data) {
    Dispatch *d=data; CanopaAgent *a=d->agent;
    if (d->action==0) { stop_signal(a); return G_SOURCE_REMOVE; }
    if (!a->task || d->generation!=a->generation) return G_SOURCE_REMOVE;
    if (d->action==1) finish(a,FALSE);
    else if (d->action==2 && d->prompt==a->prompt) start_session(a,d->index);
    else if (d->action==3 && d->prompt==a->prompt && a->awaiting && a->session) {
        a->awaiting=FALSE; polkit_agent_session_response(a->session,d->response);
    }
    return G_SOURCE_REMOVE;
}
void canopa_polkit_command(uint64_t generation, uint64_t prompt, uint32_t action,
                           uint32_t index, const char *response, size_t length) {
    if (length>=1024 || (length && (!response || memchr(response,0,length)))) return;
    g_mutex_lock(&global_lock);
    CanopaAgent *a=global_agent;
    if (a) {
        Dispatch *d=g_new0(Dispatch,1); d->agent=g_object_ref(a);
        d->generation=generation; d->prompt=prompt; d->action=action; d->index=index;
        if (length) memcpy(d->response,response,length);
        GSource *source=g_idle_source_new(); g_source_set_callback(source,dispatch,d,dispatch_free);
        g_source_attach(source,a->context); g_source_unref(source);
    }
    g_mutex_unlock(&global_lock);
}
int canopa_polkit_run(const char *session_id, uint32_t timeout, Emit callback, void *data) {
    GError *failure=NULL;
    g_mutex_lock(&global_lock);
    if (global_agent) { g_mutex_unlock(&global_lock); return 1; }
    CanopaAgent *a=g_object_new(canopa_agent_get_type(),NULL);
    a->emit=callback; a->data=data; a->timeout=timeout;
    a->context=g_main_context_new(); a->loop=g_main_loop_new(a->context,FALSE);
    global_agent=a; g_mutex_unlock(&global_lock);
    g_main_context_push_thread_default(a->context);
    PolkitAuthority *authority=polkit_authority_get_sync(NULL,&failure);
    GDBusConnection *bus=g_bus_get_sync(G_BUS_TYPE_SYSTEM,NULL,authority ? &failure : NULL);
    gpointer registration=NULL; guint filter_id=0,term=0,interrupt=0;
    gulong owner_id=0,closed_id=0;
    if (!authority || !bus) { a->result=1; goto out; }
    a->bus=bus;
    g_dbus_connection_set_exit_on_close(bus,FALSE);
    a->owner=polkit_authority_get_owner(authority);
    filter_id=g_dbus_connection_add_filter(bus,filter,g_object_ref(a),g_object_unref);
    owner_id=g_signal_connect(authority,"notify::owner",G_CALLBACK(owner_changed),a);
    closed_id=g_signal_connect(bus,"closed",G_CALLBACK(bus_closed),a);
    PolkitSubject *subject=polkit_unix_session_new(session_id);
    registration=polkit_agent_listener_register(POLKIT_AGENT_LISTENER(a),POLKIT_AGENT_REGISTER_FLAGS_NONE,
                  subject,"/org/canopa/PolicyKit1/AuthenticationAgent",NULL,&failure);
    g_object_unref(subject);
    if (!registration) { a->result=1; goto out; }
    GSource *signal=g_unix_signal_source_new(SIGTERM); g_source_set_callback(signal,stop_signal,a,NULL); term=g_source_attach(signal,a->context); g_source_unref(signal);
    signal=g_unix_signal_source_new(SIGINT); g_source_set_callback(signal,stop_signal,a,NULL); interrupt=g_source_attach(signal,a->context); g_source_unref(signal);
    a->ready=TRUE; emit(a,0,"",FALSE);
    if (!a->stopping) g_main_loop_run(a->loop);
out:
    a->stopping=TRUE;
    g_mutex_lock(&global_lock); global_agent=NULL; g_mutex_unlock(&global_lock);
    finish(a,FALSE);
    if (owner_id) g_signal_handler_disconnect(authority,owner_id);
    if (closed_id) g_signal_handler_disconnect(bus,closed_id);
    if (filter_id) g_dbus_connection_remove_filter(bus,filter_id);
    if (registration) polkit_agent_listener_unregister(registration);
    if (term) { GSource *s=g_main_context_find_source_by_id(a->context,term); if(s) g_source_destroy(s); }
    if (interrupt) { GSource *s=g_main_context_find_source_by_id(a->context,interrupt); if(s) g_source_destroy(s); }
    if (failure) {
        /* Registration errors precede any credential conversation. */
        g_printerr("canopa-polkit: cannot register authentication agent: %s\n",failure->message);
        g_error_free(failure);
    }
    g_clear_object(&authority); g_clear_object(&bus);
    for (int i=0;i<1024 && g_main_context_pending(a->context);i++) g_main_context_iteration(a->context,FALSE);
    int result=a->result;
    g_main_context_pop_thread_default(a->context);
    g_object_unref(a);
    return result;
}

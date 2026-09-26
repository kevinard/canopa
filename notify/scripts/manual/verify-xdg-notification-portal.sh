#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
CANOPA_BIN="${CANOPA_NOTIFY_BIN:-target/debug/canopa-notify}"
PORTAL_FRONTEND="${XDG_DESKTOP_PORTAL_BIN:-/usr/lib/xdg-desktop-portal}"
PORTAL_GTK="${XDG_DESKTOP_PORTAL_GTK_BIN:-/usr/lib/xdg-desktop-portal-gtk}"
FD_ICON_CLIENT="${PORTAL_FD_ICON_CLIENT:-$SCRIPT_DIR/portal-fd-icon-client.py}"
EXPECTED_VERSION="${PORTAL_EXPECT_VERSION:-1}"
PORTAL_OBJECT="/org/freedesktop/portal/desktop"
PORTAL_INTERFACE="org.freedesktop.portal.Notification"

usage() {
    cat <<EOF
Usage: $(basename "$0")

Run the installed XDG Notification portal frontend and GTK backend against a
headless Canopa Notify daemon on a private D-Bus session. The live desktop
notification server is never replaced.

Environment:
  CANOPA_NOTIFY_BIN              Canopa binary. Default: $CANOPA_BIN
  XDG_DESKTOP_PORTAL_BIN         Portal frontend. Default: $PORTAL_FRONTEND
  XDG_DESKTOP_PORTAL_GTK_BIN     GTK portal backend. Default: $PORTAL_GTK
  PORTAL_EXPECT_VERSION          Expected public Notification version. Default: 1
EOF
}

require_command() {
    local command_name="$1"
    if ! command -v "$command_name" >/dev/null 2>&1; then
        echo "Missing required command: $command_name" >&2
        exit 1
    fi
}

require_executable() {
    local executable="$1"
    if [[ ! -x "$executable" ]]; then
        echo "Missing executable: $executable" >&2
        exit 1
    fi
}

wait_for_command() {
    local description="$1"
    shift
    local attempt
    for attempt in {1..100}; do
        if "$@" >/dev/null 2>&1; then
            return 0
        fi
        sleep 0.05
    done
    echo "Timed out waiting for $description" >&2
    return 1
}

wait_for_file_text() {
    local file="$1"
    local text="$2"
    local attempt
    for attempt in {1..100}; do
        if grep -F -- "$text" "$file" >/dev/null 2>&1; then
            return 0
        fi
        sleep 0.05
    done
    echo "Timed out waiting for '$text' in $file" >&2
    return 1
}

assert_file_lacks() {
    local file="$1"
    local text="$2"
    if grep -F -- "$text" "$file" >/dev/null 2>&1; then
        echo "Unexpected '$text' in $file" >&2
        return 1
    fi
}

active_json() {
    "$CANOPA_BIN" ctl --socket "$FIXTURE_SOCKET" active --json
}

wait_for_active_filter() {
    local filter="$1"
    local attempt
    local state
    for attempt in {1..100}; do
        state="$(active_json)"
        if jq -e "$filter" >/dev/null 2>&1 <<<"$state"; then
            printf '%s\n' "$state"
            return 0
        fi
        sleep 0.05
    done
    echo "Canopa state did not satisfy jq filter: $filter" >&2
    printf '%s\n' "$state" >&2
    return 1
}

control_request() {
    local request="$1"
    printf '%s\n' "$request" | socat - "UNIX-CONNECT:$FIXTURE_SOCKET"
}

sandboxed_run() {
    bwrap \
        --info-fd 3 \
        --tmpfs / \
        --ro-bind /usr /usr \
        --symlink usr/bin /bin \
        --symlink usr/lib /lib \
        --symlink usr/lib /lib64 \
        --ro-bind /etc /etc \
        --ro-bind /run /run \
        --ro-bind /tmp /tmp \
        --dev /dev \
        --proc /proc \
        --ro-bind "$FLATPAK_INFO" /.flatpak-info \
        --ro-bind "$FD_ICON_CLIENT" /portal-fd-icon-client.py \
        "$@" \
        3>"$BWRAP_INFO"
}

sandboxed_portal_call() {
    local method="$1"
    shift
    sandboxed_run gdbus call \
        --session \
        --dest org.freedesktop.portal.Desktop \
        --object-path "$PORTAL_OBJECT" \
        --method "$PORTAL_INTERFACE.$method" \
        "$@"
}

sandboxed_fd_icon_call() {
    sandboxed_run python3 /portal-fd-icon-client.py "$1"
}

cleanup() {
    local pid
    for pid in "${FIXTURE_PIDS[@]:-}"; do
        if kill -0 "$pid" >/dev/null 2>&1; then
            kill "$pid" >/dev/null 2>&1 || true
            wait "$pid" 2>/dev/null || true
        fi
    done
    if [[ -n "${FIXTURE_DIR:-}" && "$FIXTURE_DIR" == /tmp/canopa-portal-fixture.* ]]; then
        rm -rf -- "$FIXTURE_DIR"
    fi
    if [[ -n "${FLATPAK_INSTANCE_DIR:-}" &&
        "$FLATPAK_INSTANCE_DIR" == "${XDG_RUNTIME_DIR:?}"/.flatpak/canopa-portal-fixture-* ]]; then
        rm -rf -- "$FLATPAK_INSTANCE_DIR"
    fi
}

run_inside_private_bus() {
    FIXTURE_DIR="$(mktemp -d /tmp/canopa-portal-fixture.XXXXXX)"
    FIXTURE_SOCKET="$FIXTURE_DIR/control.sock"
    local database="$FIXTURE_DIR/history.db"
    local signals="$FIXTURE_DIR/portal-signals.log"
    local fdo_calls="$FIXTURE_DIR/fdo-notify.log"
    local instance_id="canopa-portal-fixture-$BASHPID"
    FLATPAK_INFO="$FIXTURE_DIR/flatpak-info"
    FLATPAK_INSTANCE_DIR="${XDG_RUNTIME_DIR:?}/.flatpak/$instance_id"
    BWRAP_INFO="$FLATPAK_INSTANCE_DIR/bwrapinfo.json"
    FIXTURE_PIDS=()
    trap cleanup EXIT INT TERM

    mkdir -p "$FLATPAK_INSTANCE_DIR"

    printf '%s\n' \
        '[Application]' \
        'name=org.canopa.PortalFixture' \
        'runtime=org.freedesktop.Platform/x86_64/25.08' \
        '' \
        '[Instance]' \
        "instance-id=$instance_id" \
        >"$FLATPAK_INFO"

    export XDG_CURRENT_DESKTOP="${XDG_CURRENT_DESKTOP:-niri}"

    "$CANOPA_BIN" daemon \
        --headless \
        --socket "$FIXTURE_SOCKET" \
        --database "$database" \
        --dev-bus-name org.freedesktop.Notifications \
        >"$FIXTURE_DIR/canopa.log" 2>&1 &
    FIXTURE_PIDS+=("$!")
    wait_for_command "Canopa control socket" \
        "$CANOPA_BIN" ctl --socket "$FIXTURE_SOCKET" status

    "$PORTAL_GTK" >"$FIXTURE_DIR/portal-gtk.log" 2>&1 &
    FIXTURE_PIDS+=("$!")
    wait_for_command "GTK Notification portal backend" \
        busctl --user get-property \
        org.freedesktop.impl.portal.desktop.gtk \
        "$PORTAL_OBJECT" \
        org.freedesktop.impl.portal.Notification \
        version

    "$PORTAL_FRONTEND" --verbose >"$FIXTURE_DIR/portal.log" 2>&1 &
    FIXTURE_PIDS+=("$!")
    wait_for_command "XDG Notification portal frontend" \
        busctl --user get-property \
        org.freedesktop.portal.Desktop \
        "$PORTAL_OBJECT" \
        "$PORTAL_INTERFACE" \
        version

    local public_version
    local backend_version
    local supported_options
    public_version="$(busctl --user get-property \
        org.freedesktop.portal.Desktop "$PORTAL_OBJECT" "$PORTAL_INTERFACE" version | awk '{print $2}')"
    backend_version="$(busctl --user get-property \
        org.freedesktop.impl.portal.desktop.gtk "$PORTAL_OBJECT" \
        org.freedesktop.impl.portal.Notification version | awk '{print $2}')"
    supported_options="$(busctl --user get-property \
        org.freedesktop.portal.Desktop "$PORTAL_OBJECT" "$PORTAL_INTERFACE" SupportedOptions)"

    echo "Portal frontend Notification version: $public_version"
    echo "GTK backend Notification version: $backend_version"
    echo "Portal SupportedOptions: $supported_options"
    if [[ "$public_version" != "$EXPECTED_VERSION" ]]; then
        echo "Expected public Notification version $EXPECTED_VERSION, got $public_version" >&2
        exit 1
    fi
    if (( public_version < 2 )) && [[ "$supported_options" != "a{sv} 0" ]]; then
        echo "Expected empty SupportedOptions for portal v$public_version, got $supported_options" >&2
        exit 1
    fi

    dbus-monitor \
        --session \
        "type='signal',interface='org.freedesktop.portal.Notification',member='ActionInvoked'" \
        >"$signals" 2>&1 &
    FIXTURE_PIDS+=("$!")
    sleep 0.1

    dbus-monitor \
        --session \
        "type='method_call',interface='org.freedesktop.Notifications',member='Notify'" \
        >"$fdo_calls" 2>&1 &
    FIXTURE_PIDS+=("$!")

    sandboxed_run python3 /portal-fd-icon-client.py --serve \
        >"$FIXTURE_DIR/portal-app.log" 2>&1 &
    FIXTURE_PIDS+=("$!")
    wait_for_command "sandbox portal application" \
        busctl --user status org.canopa.PortalFixture

    local initial_notification
    initial_notification="{'title': <'Portal initial'>, 'body': <'Plain fallback'>, 'markup-body': <'<b>Markup only</b>'>, 'icon': <('themed', <['mail-unread-symbolic']>)>, 'priority': <'high'>, 'default-action': <'portal.default'>, 'default-action-target': <'thread-42'>, 'buttons': <[{'action': <'portal.open'>, 'label': <'Open'>, 'target': <'button-7'>, 'purpose': <'im.reply-with-text'>}]>, 'display-hint': <['transient', 'hide-on-lock-screen']>, 'category': <'im.received'>, 'sound': <'silent'>}"

    sandboxed_portal_call AddNotification portal-replace "$initial_notification" >/dev/null
    local initial_state
    initial_state="$(wait_for_active_filter \
        'length == 1 and .[0].accepted_notification.summary == "Portal initial"')"
    jq -e '
        .[0].revision == 1 and
        .[0].accepted_notification.body == "Plain fallback" and
        .[0].accepted_notification.app_name == "" and
        .[0].accepted_notification.app_identity == "org.canopa.PortalFixture" and
        .[0].accepted_notification.urgency == "normal" and
        .[0].accepted_notification.category == null and
        .[0].accepted_notification.transient == false and
        .[0].accepted_notification.app_icon == {
            "kind": "icon_name", "value": "mail-unread-symbolic"
        } and
        [. [0].accepted_notification.actions[].key] == [
            "default", "portal.open::button-7"
        ]
    ' >/dev/null <<<"$initial_state"
    wait_for_file_text "$fdo_calls" "Portal initial"
    wait_for_file_text "$fdo_calls" "int32 -1"
    assert_file_lacks "$fdo_calls" "Markup only"
    assert_file_lacks "$fdo_calls" "category"
    assert_file_lacks "$fdo_calls" "transient"
    assert_file_lacks "$fdo_calls" "suppress-sound"
    assert_file_lacks "$fdo_calls" "sound-name"
    assert_file_lacks "$fdo_calls" "sound-file"
    local internal_id
    internal_id="$(jq -r '.[0].id' <<<"$initial_state")"

    local updated_notification
    updated_notification="{'title': <'Portal updated'>, 'body': <'Updated body'>, 'icon': <('themed', <['mail-unread-symbolic']>)>, 'priority': <'urgent'>, 'default-action': <'portal.default'>, 'default-action-target': <'thread-42'>, 'buttons': <[{'action': <'portal.open'>, 'label': <'Open'>, 'target': <'button-7'>}]>}"
    sandboxed_portal_call AddNotification portal-replace "$updated_notification" >/dev/null

    local updated_state
    updated_state="$(wait_for_active_filter \
        'length == 1 and .[0].revision == 2 and .[0].accepted_notification.summary == "Portal updated"')"
    jq -e --argjson id "$internal_id" '
        .[0].id == $id and
        .[0].accepted_notification.body == "Updated body" and
        .[0].accepted_notification.urgency == "critical"
    ' >/dev/null <<<"$updated_state"

    local panel_state
    panel_state="$(control_request panel-snapshot)"
    jq -e --arg id "$internal_id" '
        [.content.sections[].items[]] | length == 1 and .[0].id == $id
    ' >/dev/null <<<"$panel_state"

    [[ "$(control_request "invoke-action $internal_id portal.open::button-7")" == "ok" ]]
    wait_for_file_text "$signals" 'string "portal-replace"'
    wait_for_file_text "$signals" 'string "portal.open"'
    wait_for_file_text "$signals" 'string "button-7"'
    wait_for_active_filter 'length == 0' >/dev/null

    local default_notification
    default_notification="{'title': <'Portal default action'>, 'default-action': <'portal.default'>, 'default-action-target': <'thread-42'>}"
    sandboxed_portal_call AddNotification portal-default "$default_notification" >/dev/null
    local default_state
    default_state="$(wait_for_active_filter \
        'length == 1 and .[0].accepted_notification.summary == "Portal default action"')"
    local default_id
    default_id="$(jq -r '.[0].id' <<<"$default_state")"
    [[ "$(control_request "invoke-action $default_id default")" == "ok" ]]
    wait_for_file_text "$signals" 'string "portal-default"'
    wait_for_file_text "$signals" 'string "portal.default"'
    wait_for_file_text "$signals" 'string "thread-42"'
    wait_for_active_filter 'length == 0' >/dev/null

    sandboxed_portal_call AddNotification portal-remove \
        "{'title': <'Portal removal'>, 'body': <'Remove me'>}" >/dev/null
    wait_for_active_filter \
        'length == 1 and .[0].accepted_notification.summary == "Portal removal"' >/dev/null
    sandboxed_portal_call RemoveNotification portal-remove >/dev/null
    wait_for_active_filter 'length == 0' >/dev/null

    sandboxed_fd_icon_call portal-fd-icon >/dev/null
    local fd_icon_state
    fd_icon_state="$(wait_for_active_filter '
        length == 1 and
        .[0].accepted_notification.app_identity == "org.canopa.PortalFixture"
    ')"
    jq -e '
        .[0].accepted_notification.summary == "" and
        .[0].accepted_notification.body == "" and
        .[0].accepted_notification.has_image_payload == false and
        .[0].accepted_notification.app_icon == null
    ' >/dev/null <<<"$fd_icon_state"
    wait_for_file_text "$fdo_calls" 'string "image-data"'
    sandboxed_portal_call RemoveNotification portal-fd-icon >/dev/null
    wait_for_active_filter 'length == 0' >/dev/null

    panel_state="$(control_request panel-snapshot)"
    jq -e '
        [.content.sections[].items[] | select(.title == "Portal updated")] | length == 1
    ' >/dev/null <<<"$panel_state"

    if (( public_version < 2 )); then
        jq -e '
            [.content.sections[].items[] | select(.title == "Portal updated")][0] |
            .body == "Updated body"
        ' >/dev/null <<<"$panel_state"
        assert_file_lacks "$signals" "activation-token"
        echo "Portal v2-only fields: skipped (frontend negotiated version $public_version)"
        echo "  markup-body/display-hint/category/sound/button-purpose: filtered"
        echo "  file-descriptor icon: converted to freedesktop image-data and accepted by Canopa"
        echo "  inline reply/activation-token platform data: unavailable"
    else
        echo "Portal v2 is active; stable delivery assertions passed."
        echo "Inspect docs/xdg-notification-portal-v2.md before extending backend-specific v2 assertions."
    fi

    echo "PASS: real portal frontend -> GTK backend -> Canopa compatibility fixture"
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    usage
    exit 0
fi

if [[ "${1:-}" == "--inside-private-bus" ]]; then
    run_inside_private_bus
    exit 0
fi

if [[ $# -ne 0 ]]; then
    usage >&2
    exit 2
fi

require_command dbus-run-session
require_command gdbus
require_command busctl
require_command jq
require_command socat
require_command awk
require_command bwrap
require_command dbus-monitor
require_command python3
require_executable "$CANOPA_BIN"
require_executable "$PORTAL_FRONTEND"
require_executable "$PORTAL_GTK"
if [[ ! -r "$FD_ICON_CLIENT" ]]; then
    echo "Missing FD icon helper: $FD_ICON_CLIENT" >&2
    exit 1
fi

exec dbus-run-session -- "$0" --inside-private-bus

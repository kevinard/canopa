#!/usr/bin/env bash
set -euo pipefail

APP_NAME="${CANOPA_NOTIFY_APP_NAME:-Canopa Manual Test}"
SOCKET="${CANOPA_NOTIFY_SOCKET:-${XDG_RUNTIME_DIR:-/tmp}/canopa-notify/control.sock}"
CANOPA_BIN="${CANOPA_NOTIFY_BIN:-canopa-notify}"
DEFAULT_EXPIRE_MS="${CANOPA_NOTIFY_EXPIRE_MS:-12000}"

usage() {
    cat <<EOF
Usage: $(basename "$0") <scenario>

Scenarios:
  smoke       Send one ordinary banner notification.
  actions     Send a notification with default and named actions; click it.
  replace     Send a notification, then replace it using notify-send -p/-r.
  progress    Replace one notification several times using the value hint.
  sound       Send default, named, file-backed, and suppressed sound notifications.
  burst       Send a small stack of notifications.
  all         Run smoke, replace, progress, sound, and burst.

Environment:
  CANOPA_NOTIFY_APP_NAME   App name sent via notify-send. Default: $APP_NAME
  CANOPA_NOTIFY_SOCKET     Control socket for optional ctl output. Default: $SOCKET
  CANOPA_NOTIFY_BIN        canopa-notify binary for optional ctl output. Default: $CANOPA_BIN
  CANOPA_NOTIFY_EXPIRE_MS  Expire timeout passed to notify-send. Default: $DEFAULT_EXPIRE_MS
  CANOPA_NOTIFY_SOUND_FILE Local sound file for the sound-file case.

For ActivationToken / ActionInvoked validation, run this in another terminal:
  scripts/manual/watch-notification-signals.sh
EOF
}

require_command() {
    local command_name="$1"
    if ! command -v "$command_name" >/dev/null 2>&1; then
        echo "Missing required command: $command_name" >&2
        exit 1
    fi
}

notify_help() {
    notify-send --help 2>&1 || true
}

notify_supports() {
    local option="$1"
    notify_help | grep -F -- "$option" >/dev/null
}

server_info() {
    if command -v gdbus >/dev/null 2>&1; then
        echo "Notification server:"
        gdbus call \
            --session \
            --dest org.freedesktop.Notifications \
            --object-path /org/freedesktop/Notifications \
            --method org.freedesktop.Notifications.GetServerInformation \
            2>/dev/null || echo "  unable to query org.freedesktop.Notifications"
        echo
    fi
}

control_active() {
    if command -v "$CANOPA_BIN" >/dev/null 2>&1 && [[ -S "$SOCKET" ]]; then
        echo
        echo "Canopa active notifications:"
        "$CANOPA_BIN" ctl --socket "$SOCKET" active --json || true
        echo
    fi
}

send_smoke() {
    server_info
    notify-send \
        --app-name="$APP_NAME" \
        --expire-time="$DEFAULT_EXPIRE_MS" \
        "Canopa smoke" \
        "Expected: one banner appears, can be dismissed, and is listed as active."
    control_active
}

send_actions() {
    if ! notify_supports "--action"; then
        echo "This notify-send does not advertise --action support." >&2
        echo "Install a recent libnotify notify-send, then retry this scenario." >&2
        exit 1
    fi

    echo "Click the banner body for the default action, or click Open/Reply."
    echo "The selected action key should be printed below."
    local selected_action=""
    selected_action="$(
        notify-send \
            --app-name="$APP_NAME" \
            --expire-time="$DEFAULT_EXPIRE_MS" \
            --action=default=Default \
            --action=open=Open \
            --action=reply=Reply \
            "Canopa actions" \
            "Click the banner body or one of the action buttons."
    )"

    if [[ -n "$selected_action" ]]; then
        echo "notify-send selected action: $selected_action"
    else
        echo "notify-send did not report an action. The banner may have timed out or been closed."
    fi
}

send_replace() {
    if ! notify_supports "--print-id" || ! notify_supports "--replace-id"; then
        echo "This notify-send does not advertise --print-id and --replace-id support." >&2
        exit 1
    fi

    local notification_id=""
    notification_id="$(
        notify-send \
            --print-id \
            --app-name="$APP_NAME" \
            --expire-time=0 \
            "Canopa replace 1/3" \
            "This notification should be replaced in-place."
    )"

    echo "Sent notification id: $notification_id"
    sleep 1

    notify-send \
        --replace-id="$notification_id" \
        --app-name="$APP_NAME" \
        --expire-time=0 \
        "Canopa replace 2/3" \
        "Same sender id, updated summary and body."

    sleep 1

    notify-send \
        --replace-id="$notification_id" \
        --app-name="$APP_NAME" \
        --expire-time="$DEFAULT_EXPIRE_MS" \
        "Canopa replace 3/3" \
        "Final replacement. Expected: one banner/thread, not three."

    control_active
}

send_progress() {
    if ! notify_supports "--print-id" || ! notify_supports "--replace-id"; then
        echo "This notify-send does not advertise --print-id and --replace-id support." >&2
        exit 1
    fi

    local notification_id=""
    notification_id="$(
        notify-send \
            --print-id \
            --app-name="$APP_NAME" \
            --expire-time=0 \
            --hint=int:value:0 \
            --hint=string:category:transfer \
            "Canopa progress" \
            "Progress hint value: 0"
    )"

    echo "Sent progress notification id: $notification_id"

    local value
    for value in 25 50 75 100; do
        sleep 0.35
        notify-send \
            --replace-id="$notification_id" \
            --app-name="$APP_NAME" \
            --expire-time="$DEFAULT_EXPIRE_MS" \
            --hint=int:value:"$value" \
            --hint=string:category:transfer \
            "Canopa progress" \
            "Progress hint value: $value"
    done
    control_active
}

send_sound() {
    if ! notify_supports "--hint"; then
        echo "This notify-send does not advertise --hint support." >&2
        exit 1
    fi

    local sound_file="${CANOPA_NOTIFY_SOUND_FILE:-/usr/share/sounds/freedesktop/stereo/message-new-instant.oga}"

    echo "Expected: the first three notifications play a sound; the fourth is silent."
    notify-send \
        --app-name="$APP_NAME" \
        --expire-time="$DEFAULT_EXPIRE_MS" \
        "Canopa sound default" \
        "Expected: default notification sound."
    sleep 1

    notify-send \
        --app-name="$APP_NAME" \
        --expire-time="$DEFAULT_EXPIRE_MS" \
        --hint=string:sound-name:message-new-instant \
        "Canopa sound name" \
        "Expected: message-new-instant from the active sound theme."
    sleep 1

    if [[ -f "$sound_file" ]]; then
        notify-send \
            --app-name="$APP_NAME" \
            --expire-time="$DEFAULT_EXPIRE_MS" \
            --hint=string:sound-file:"$sound_file" \
            "Canopa sound file" \
            "Expected: local sound-file playback from $sound_file."
        sleep 1
    else
        echo "Skipping sound-file case; file does not exist: $sound_file" >&2
    fi

    notify-send \
        --app-name="$APP_NAME" \
        --expire-time="$DEFAULT_EXPIRE_MS" \
        --hint=boolean:suppress-sound:true \
        "Canopa suppress sound" \
        "Expected: visible notification with no notification sound."
    control_active
}

send_burst() {
    local i
    for i in 1 2 3 4; do
        notify-send \
            --app-name="$APP_NAME" \
            --expire-time="$DEFAULT_EXPIRE_MS" \
            "Canopa burst $i" \
            "Expected: banners stack without stealing focus."
        sleep 0.2
    done
    control_active
}

main() {
    require_command notify-send

    case "${1:-}" in
        smoke)
            send_smoke
            ;;
        actions)
            send_actions
            ;;
        replace)
            send_replace
            ;;
        progress)
            send_progress
            ;;
        sound)
            send_sound
            ;;
        burst)
            send_burst
            ;;
        all)
            send_smoke
            send_replace
            send_progress
            send_sound
            send_burst
            ;;
        -h|--help|"")
            usage
            ;;
        *)
            usage >&2
            exit 1
            ;;
    esac
}

main "$@"

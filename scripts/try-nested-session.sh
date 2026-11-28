#!/usr/bin/env bash
# Try the working tree's shell, notifications and compositor in a nested Niri
# window, on a private session bus and runtime directory: nothing touches your
# running session (whose shell, notification and wallpaper daemons keep their
# sockets in your real runtime directory). It uses the built-in default
# configuration, not ~/.config/canopa/config.kdl, and its own state and data
# (notification and launcher history), so your real ones are left alone. A test
# calendar with events around today fills the calendar's agenda.
#
#   bash scripts/try-nested-session.sh           # build, then open the window
#   bash scripts/try-nested-session.sh --no-build
#
# The compositor is the pinned fork built by scripts/build-niri.py into
# target/niri-fork, or CANOPA_NESTED_NIRI=/path/to/niri. Logs go to the printed
# directory. See docs/testing.md.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

if [[ "${1:-}" != "--no-build" ]]; then
    bash scripts/cargo.sh build --release \
        -p canopa-shell -p canopa-notify -p canopa-session --bin canopa-shell \
        --bin canopa-notify --bin canopa-wallpaper
fi
niri="${CANOPA_NESTED_NIRI:-target/niri-fork/release/niri}"
if [[ ! -x "$niri" ]]; then
    cat >&2 <<'MSG'
No compositor at target/niri-fork/release/niri. Build the pinned fork with
  python3 scripts/build-niri.py --archives target/fork-archives --download \
    --work-dir target/niri-fork-sources --fetch --release
or point CANOPA_NESTED_NIRI at a niri built from kevinard/niri.
MSG
    exit 1
fi
: "${WAYLAND_DISPLAY:?run this from your Wayland session}"

host_runtime="${XDG_RUNTIME_DIR:?}"
work="$(mktemp -d "${TMPDIR:-/tmp}/canopa-nested.XXXXXX")"
runtime="$work/runtime"
mkdir -m 700 "$runtime"
# Empty configuration and state: every Canopa program uses its embedded defaults.
mkdir -p "$work/config" "$work/state" "$work/data"

# A local calendar (Evolution's layout) with events relative to now.
calendar="$work/data/evolution/calendar/system"
mkdir -p "$calendar"
at() { date -d "$1" +%Y%m%dT%H%M00; }
day() { date -d "$1" +%Y%m%d; }
{
    printf 'BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Canopa//Nested trial//EN\r\n'
    event() { # uid, summary, location, DTSTART line, DTEND line, extra line
        printf 'BEGIN:VEVENT\r\nUID:%s@canopa-trial\r\nSUMMARY:%s\r\n' "$1" "$2"
        [[ -n "$3" ]] && printf 'LOCATION:%s\r\n' "$3"
        printf '%s\r\n%s\r\n' "$4" "$5"
        [[ -n "${6:-}" ]] && printf '%s\r\n' "$6"
        printf 'END:VEVENT\r\n'
    }
    event now "Focus time" "" "DTSTART:$(at '-30 minutes')" "DTEND:$(at '+30 minutes')"
    event soon "Design review" "Room 3" "DTSTART:$(at '+20 minutes')" "DTEND:$(at '+65 minutes')"
    event lunch "Lunch with Sam" "Café Lumière" \
        "DTSTART:$(day tomorrow)T123000" "DTEND:$(day tomorrow)T133000"
    event standup "Team stand-up" "" \
        "DTSTART:$(day '+2 days')T093000" "DTEND:$(day '+2 days')T094500" \
        "RRULE:FREQ=WEEKLY;COUNT=6"
    event release "Release day" "" \
        "DTSTART;VALUE=DATE:$(day '+3 days')" "DTEND;VALUE=DATE:$(day '+4 days')"
    event late "Deploy window" "" \
        "DTSTART:$(day '+4 days')T230000" "DTEND:$(day '+5 days')T010000"
    event offsite "Team offsite" "Lyon" \
        "DTSTART;VALUE=DATE:$(day '+9 days')" "DTEND;VALUE=DATE:$(day '+11 days')"
    printf 'END:VCALENDAR\r\n'
} >"$calendar/calendar.ics"
echo "Nested session files and logs: $work"

config="$work/niri.kdl"
cp packaging/niri/defaults.kdl "$config"
cat >>"$config" <<EOF

// Added for the nested trial session.
// A nested Niri is not a session instance: offer its D-Bus services anyway, such
// as the window previews the taskbar and workspace hover cards show.
debug {
    dbus-interfaces-in-non-session-instances
}
spawn-sh-at-startup "canopa-wallpaper daemon >'$work/wallpaper.log' 2>&1"
spawn-sh-at-startup "mkdir -p \"\$XDG_RUNTIME_DIR/canopa-notify\" && canopa-notify daemon --service --socket \"\$XDG_RUNTIME_DIR/canopa-notify/control.sock\" >'$work/notify.log' 2>&1"
spawn-sh-at-startup "canopa-shell >'$work/shell.log' 2>&1"
spawn-at-startup "foot"
EOF

# The nested Niri draws into your desktop through its absolute socket path; the
# programs inside get their own runtime directory, and keep your audio server.
case "$WAYLAND_DISPLAY" in
    /*) host_display="$WAYLAND_DISPLAY" ;;
    *) host_display="$host_runtime/$WAYLAND_DISPLAY" ;;
esac
export WAYLAND_DISPLAY="$host_display"
export PIPEWIRE_RUNTIME_DIR="${PIPEWIRE_RUNTIME_DIR:-$host_runtime}"
export PULSE_RUNTIME_PATH="${PULSE_RUNTIME_PATH:-$host_runtime/pulse}"
export XDG_RUNTIME_DIR="$runtime"
export XDG_CONFIG_HOME="$work/config"
export XDG_STATE_HOME="$work/state"
# Your applications and icons stay visible; the session's own data is private.
export XDG_DATA_DIRS="${XDG_DATA_HOME:-$HOME/.local/share}:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
export XDG_DATA_HOME="$work/data"
unset NIRI_SOCKET

# Key bindings spawn "canopa-shell" by name: put the fresh builds first.
export PATH="$repo/target/release:$PATH"
exec dbus-run-session -- "$niri" -c "$config" \
    >"$work/niri.log" 2>&1

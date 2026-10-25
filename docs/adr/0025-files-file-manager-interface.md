# Files and the standard FileManager1 interface

Status: scoped core and native activation verified, 2026-10-20. Extends
ADR0017/0022 and Files ADR0099. Complete packaging, platform focus and release
acceptance remain open.

Expose the [standard FileManager1 interface](https://wiki.freedesktop.org/www/Specifications/file-manager-interface/)
on the owning Files runtime's session-bus connection. Register
`org.freedesktop.FileManager1` at `/org/freedesktop/FileManager1`. Support
ShowFolders, ShowItems and ShowItemProperties with their URI-array and StartupId
arguments. The transport decodes a complete bounded local-URI request before
admitting any presentation. It never runs a command, file transfer or custom
action. Raw Unix names survive percent decoding and remain paths until display.

The renderer-independent Presentation model carries browser targets, explicit
selections or a Properties request. Both CLI opening and FileManager1 admission
use the same pending-opening holds and shutdown lock. The CLI wire protocol and
its existing Open/Resident/Quit frames remain version 1; presentation changes
are internal. An older running runtime is never replaced or given another state
writer to add this feature. Its existing CLI remains usable; finish and restart
it before relying on newly installed FileManager1 activation support.

Own the bus connection and worker before Iced starts. Registration must neither
replace another file manager nor queue behind it. Failure to acquire that name
does not prevent ordinary Files use. Shutdown closes admission, finishes accepted
work and presentation holds, retires the bus worker, then releases instance
ownership. Name acquisition and bus teardown have bounded waits. Core transport
tests run on an actual private session bus without a renderer.

Ship a session-bus activation file that starts `canopa-files --daemon` through
the session PATH. Cold FileManager1 activation therefore opts into residency;
ordinary CLI launch retains its normal idle-exit policy. `--quit` releases
residency while preserving accepted work. Activation inherits the graphical
session environment at process start. A runtime that started without a display
refuses presentation requests while its backend stays alive; never mutate its
environment from a later request.

The activation file selects Canopa as an on-demand FileManager1 provider when
installed. Packaging prepares metadata without changing the live bus, session
services or folder MIME defaults. The desktop entry keeps normal Exec launching:
FileManager1 does not implement the distinct desktop-entry DBusActivatable
application interface. Native acceptance must install the real activation file
only in private service directories, retain exact ownership of the activated
process and use the private GIO provider.

StartupId is bounded and carried with the presentation. The current Wayland
frontend uses compositor mapping policy; it does not treat an arbitrary startup
identifier as recent input or rewrite process-global activation environment.
External activation-token/focus acceptance remains part of the broader platform
gate. A method reply acknowledges accepted presentation, not mapped windows.

## Scoped verification

Core tests exercise raw URI grouping, complete-batch refusal, hidden selection,
read-only metadata and real private-bus admission. They cover all three methods,
headless refusal, pending-opening holds, stopping, exact name release and an
already owned name. The local-runtime build requires no Iced. Default library
tests pass 78 cases and all-target Clippy passes with warnings denied.

The actual shipped activation file passes 26 native checks across dark/100% and
light/125% in private service directories. Cold Properties creates no Browser;
grouped reveal preserves raw-byte and hidden selections in a committed 340px
window; Left still selects the parent folder just left. CLI and bus requests
use the same owner. Properties survives every Browser closing, scrolls to its
last item and retains the captured URI list. Quit retires the exact activated
process and bus name, and the next cold request creates a new owner.

Focused existing resident, operation/Undo and preview workflows also pass in both
appearances: 68 Files checks, plus five unchanged shared-transport probe checks.
The interrupted multi-case attempt retains its completed individual reports;
the unfinished light preview passes separately. No complete multi-case pass is
inferred from that interrupted runner. Formatting, Python syntax, activation
metadata, shell syntax and the full 178-scenario inventory also validate.

The first attempt exposed an invalid fixture-directory prefix. A subsequent
attempt exposed a weak screenshot assertion that could mistake resizing for
scrolling. Both artifacts are retained; the final fixture waits for committed
geometry and stable frames, and its narrow start/end screenshots were inspected.
These are private debug-build checks on the explicitly selected Niri companion;
they do not establish hosted release, distribution compatibility, performance or
activation-token acceptance. Evidence is in
[the FileManager1 record](../../.scratch/file-manager/file-manager-verification.json).

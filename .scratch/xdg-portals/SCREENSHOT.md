# Screenshot preparation

Status: Screenshot/PickColor workflow and Iced review implemented under ADR0040.
Screen/Area are in the shipped descriptor. Eleven native groups pass in both
appearances using actual shipped metadata; production multi-output PNG composition
passes separately. Required shared gate exits101 at the same ten Files integration
failures as the predecessor gate; scoped Portal/native checks pass. See the
implementation ledger. Full18-interface goal remains active.

The original investigation below is retained as source evidence. Current capture
implementation and terminal validation will be recorded in the final section.

The installed backend XML describes Screenshot v3: Screenshot and PickColor
use owned request handles; screenshot results return a URI and color results a
three-double RGB tuple in [0,1]. `AvailableTargets` is a bitmask, while each
request's optional `target` is a single supported value: screen 1, window 2,
area 4 or active window 8. Existing interactive, modal and
permission-store-checked options remain relevant. Version/target advertisement
must follow implemented and natively verified behavior.

The pinned [Niri compatibility service](https://github.com/YaLTeR/niri/blob/8ed0da44d974c32c6877d2f4630c314da0717ecb/src/dbus/gnome_shell_screenshot.rs)
implements Screenshot and PickColor in Rust. Its Screenshot ignores the supplied
filename and flash flag and uses a shared result receiver. The
[compositor handler](https://github.com/YaLTeR/niri/blob/8ed0da44d974c32c6877d2f4630c314da0717ecb/src/niri.rs)
captures all outputs through Niri's screenshot machinery and returns the resulting
path. PickColor has its own reply channel. This is not a per-portal request-file
or cancellable selection contract. Do not assume an arbitrary filename becomes
a private result file or use overlapping calls without considering correlation.

The installed Niri IPC also has screenshot-screen/window actions. The existing
desktop fixture uses screenshot-screen with an explicit private path; its
`capture` method uses grim's Wayland screencopy transport for actual native frames.
Those successful native image captures demonstrate a different capability from
the earlier D-Bus introspection probe. They do not prove a Canopa Rust capture
adapter, selection/consent, caller cancellation or ScreenCast media streaming.

Resolve capture ownership before implementing the interface:

- Choose a Rust compositor adapter whose result, timeout and cancellation can be
  bound to one admitted portal request. Investigate the existing IPC actions and
  Wayland screencopy route as well as the compatibility service.
- Own and bound pixels/files before review; validate a returned compositor path,
  regular-file/size/type assumptions and format. A helper must not publish an
  arbitrary compositor/user path as an app grant. Keep original user screenshots
  outside speculative cancellation cleanup.
- Present selection/review through Iced, with explicit sharing. Preserve the
  request owner's Close/caller-loss lifetime and avoid leaving a compositor UI
  open after a cancelled request. Unknown/global compositor selection ownership
  needs investigation before relying on its picker.
- Return an owned, readable result URI only after confirmation. The frontend
  retains document export and permission policy. Define bounded cache lifetime
  and failure behavior, including original-source removal.
- Verify every advertised target, actual pixel content, multi-output/scaling,
  window identity/close races, color normalization, cancellation and concurrency.
  Use only private fixture windows/outputs and both Canopa appearances.

The ScreenCast GBM requirement in NEXT.md concerns Niri's Mutter-compatible
streaming adapter. It should not be generalized into a claim that private
software-rendered screenshot capture is unavailable.

## Capture ownership follow-up, 2026-10-30

The pinned [Niri save path](https://github.com/YaLTeR/niri/blob/8ed0da44d974c32c6877d2f4630c314da0717ecb/src/niri.rs#L5190)
queues clipboard replacement for every encoded screenshot, including explicit-path
screen/window actions. Its encoder/file write runs on a spawned thread. The
[IPC action reply](https://github.com/YaLTeR/niri/blob/8ed0da44d974c32c6877d2f4630c314da0717ecb/src/ipc/server.rs#L355)
waits for action dispatch, not encoder completion. Explicit path alone therefore
does not give a no-side-effect, cancellable portal capture. Do not implement a
portal by silently using these actions and then deleting a speculative filename.

A disposable Weston/Niri26.04 (8ed0da4) inventory is retained at
/tmp/canopa-screenshot-wayland-capabilities-01/wayland-info.txt and inventory.json.
It exposes zwlr_screencopy_manager_v1 version3, zxdg_output_manager_v1 version3,
and zwlr_foreign_toplevel_manager_v1 version3. It does not expose
ext_image_copy_capture_manager_v1 or output/toplevel image-capture-source managers.
The probe acquired the inventory, then its initial parser assertion failed on
aligned whitespace; the saved inventory was parsed correctly offline. No new
frame was captured by this probe. Existing fixture captures remain separate
pixel evidence, not a Rust adapter claim. All compositor/bus processes retired.

The locally installed wayland-protocols-wlr0.3.12 XML deprecates wlr-screencopy in
favor of ext-image-copy-capture, but this pinned compositor exposes the former.
A compatibility adapter must bind the available protocol explicitly and fail
clearly when it is absent. Frame v3 enumerates buffers before buffer_done; accept
bounded supported wl_shm layouts, account for stride and y_invert, and destroy
frames on Ready/Failed/cancellation. Reject unexpected formats, excessive pixels,
invalid geometry or missing buffer support before allocating. Connection/dispatch
must be interruptible; dropping an async future around blocking_dispatch alone
would not establish cancellation or worker admission.

Proposed next seam, still unimplemented: an admitted Rust Capture owns one
Wayland connection and bounded full pixels, then Iced reviews an immutable frame.
Use screen/area and PickColor only if native acceptance establishes them. A
window crop is not an unobscured window capture. Do not advertise window/active
window targets via unsupported sources or substitute the clipboard-mutating IPC.
Explicit target support can be a truthful AvailableTargets subset under backend
v3; verify installed frontend behavior before choosing final version and policy.
A result cache needs private publication after explicit acceptance and a bounded
retention policy compatible with the frontend's asynchronous document export.
No descriptor, dependencies or production code changed for this investigation.

The installed frontend's1.22.1 source is already saved under
/tmp/canopa-xdp-lockdown-source-01/xdg-desktop-portal-1.22.1/src/screenshot.c.
It forwards AvailableTargets for backend version>=3 and rejects requested target
values outside that mask. Thus an implemented subset is compatible with this
frontend; it is not necessary to claim every target. Backend version>=2 also
triggers the frontend's non-interactive permission check (Access dialog when
needed), while interactive requests bypass that preliminary check. The backend
must still own its explicit capture review. Frontend response processing exports
the returned URI to the document portal for sandboxed apps in a worker; host apps
receive the original URI. Returning a temporary path and immediately removing it
would therefore race consumers. This is source inspection, not native Screenshot
frontend acceptance. The public review/cache contract remains unimplemented.

## Implemented adapter checkpoint

Source: crates/canopa-portal/src/capture.rs and capture/wayland.rs. The adapter
returns owned per-output upright physical pixels plus logical geometry, under
retained admission and a deadline. It does not expose a Screenshot D-Bus method,
start Iced review or publish an accepted file. Read ADR0039 for exact source,
format/worker/memory limits and display identity. Do not advertise the interface
from transport success alone.

All04 passes82 units and12 integrations (2 native entries ignored); Clippy02 and
headless01 pass. Native sway04 passes4 groups; niri03/125% passes3 groups, using
fixed harness SHA2569f8155da2c710356bb519a06625df1366eeb73e86caedf5aac4da032cc8e7e57.
All8 transforms are verified with an asymmetric C image on the private headless
multi-output compositor. Niri's fixed Flipped180 and125%->150% logical rounding
are verified separately. A full1280x800 source is retained when grim renders its
rounded geometry to1280x799; physical mode and uniform interior pixels are checked
independently, rather than truncating source pixels. Clipboard and fd retirement,
negative origins, mixed scales, hotplug, retained-memory admission/immutability,
socket loss and recovery are covered. Full logs/results are under
/tmp/canopa-screenshot-capture-*. Shared gate outcome belongs in IMPLEMENTATION.md.

Next integrate the actual request workflow and Iced presentation. Reuse one
Captures instance across Screenshot/PickColor; snapshots retain its permits through
review. Validate options/types/targets and sender-owned handles, show explicit
sharing/pixel selection, validate every helper result against immutable source
pixels, and publish a bounded private result only after acceptance. The full image
needs an owned helper transport; the existing1MiB JSON pipe cannot carry arbitrary
full-screen RGBA. Use sealed owned descriptors or another deliberate bounded
transport, never a caller/compositor pathname. Preserve Close/caller loss, foreign
parenting and acceptance semantics. Define result retention for asynchronous URI
export and verify actual frontend behavior and every advertised capability before
adding metadata. Unobscured window capture remains a separate source capability;
a screen crop does not implement it. All remaining roadmap interfaces stay active.


## Frontend permission hint follow-up

The current [GNOME screenshot handler](https://github.com/GNOME/xdg-desktop-portal-gnome/blob/main/src/screenshot.c)
reports backend version2. interactive=true delegates selection to the compositor;
otherwise it creates a review controller with permission_store_checked. Its
[review controller](https://github.com/GNOME/xdg-desktop-portal-gnome/blob/main/src/screenshotdialog.c)
sets skip_dialog from that flag and returns the captured file directly when the
frontend has already checked permission. Thus do not equate interactive=false
with a mandatory second review: the installed frontend already owns the access
permission flow. Decide and document Canopa behavior with both paths tested,
rather than accidentally changing permitted non-interactive capture into repeated
UI prompts. Interactive Screenshot/PickColor still need actual user selection.
This is source evidence and a pending workflow decision, not new production code
or native frontend acceptance. Avoid reusing GNOME's shared Niri screenshot call.


Shared gate01 is terminal101 at the same ten Files integration failure names as
the earlier Background baseline (596pass/10fail/41ignore). The catalog case also
fails alone10.19s. Files library372+34ignore, tooling, full fmt, Clippy5m20s,
inventory632 and all-target compilation10m36s pass. Later stages were not reached.
This gate is not green; all exact names/comparison/logs are in IMPLEMENTATION.md.
All capture jobs are terminal. No user configuration/install or vendor edits.

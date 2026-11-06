# 0112: Preserve frontend-owned ScreenCast PipeWire grants

Status: scoped real Flatpak verification, 2026-11-05. Included in the optional
desktop-sharing package; no live session or system configuration changed.

Canopa keeps public caller identity and PipeWire access in the standard XDG
frontend. A real Flatpak GIO/PipeWire client now reaches normal Rust/Iced consent
without the host Registry registration used by the earlier native fixtures.

On PipeWire 1.6.10 and WirePlumber 0.5.18, the frontend sets explicit core,
factory and selected-stream grants with denied fallback access. WirePlumber's
new portal permission manager subsequently attaches with permissive defaults.
The sandbox then sees unrelated nodes. Timing changes whether this appears in
the first registry round trip or during actual frame delivery. The baseline
receives the correct image and still fails the unrelated-node assertion.

Ship an original, optional WirePlumber Lua hook with the sharing companions.
For daemon-authenticated `pipewire.access=portal` connections with an empty
portal media-role list, select a permission manager with zero fallback access
and core `rwx` access. Its update contains no per-node entries, preserving the
frontend's explicit stream and factory grants. The camera role and other
clients continue through their existing policies. Explicit administrator access
configuration retains precedence. Older versions without PermissionManager
return without replacing their policy.

The manager also handles the newer PipeWire handoff gate by restoring only core
`rwx`. This callback has a focused Lua guard test; the actual installed 1.6.10
runtime does not establish acceptance for newer gate-enabled versions. The hook
loads as a `hooks.*` feature before event dispatch in the main/policy profiles.
No WirePlumber source copy, new native fork or library ABI is introduced.

`scripts/test-screencast-flatpak.py` creates private user/mount namespaces, bus,
hardware-GL Weston/Niri, PipeWire and policy-only WirePlumber. A tiny application
with no host filesystem/display/PipeWire permissions verifies its Flatpak identity,
private-backend denial, public session flow, restricted registry and exact pixels
through the provided FD. Three cases cover Cancel, Iced Stop and app exit without
Session.Close. Both positive cases produce the same exact 400×300 image and
retire their producer/control windows. The real app ID reaches the backend.

Evidence: target/portal-sharing-flatpak-87/native-dark-03/summary.json is the
failed stock-policy baseline with actual pixels and an unrelated node exposed.
native-dark-05/summary.json passes all four final groups on the unchanged
a30backend/6e3d Niri/277568 frontend. All workers retire, the private bind unmounts,
temporary paths disappear and the source pins remain unchanged. policy-check-01.log
covers camera/direct/unknown-role/config precedence and core-only ungating.

The test reuses the existing immutable runtime commit and adds only six installed
PipeWire client libraries to the probe application. No SDK/runtime download,
Cargo rebuild or cache deletion is needed. Failures from the initial incomplete
client configuration and invalid Lua permission string remain failed evidence.
This is GIO window capture at dark/100%, not universal toolkit/device/distribution
acceptance. Print remains separately opt-in and deferred.

References: [PipeWire portal responsibility](https://docs.pipewire.org/page_module_portal.html),
[WirePlumber access policy](https://pipewire.pages.freedesktop.org/wireplumber/policies/client_access.html),
[custom hook loading](https://pipewire.pages.freedesktop.org/wireplumber/scripting/custom_scripts.html).

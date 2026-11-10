# Fork inventory

Status: needs-triage

Reviewed 2026-11-10 by reading every patch, `vendor/provenance.json`, the
recipe READMEs and the ADRs, and by comparing upstream sources (fetched with
`gh`). Nothing was built. "Upstream status" reflects that date. This file
supports workstream W2 of `.scratch/distro-readiness/PRD.md`.

Recommendation key: **Upstream** (send the change), **Replace** (adopt an
upstream mechanism that now exists), **Keep** (local until an upstream path
exists), **Drop** (no longer needed).

## Overview

| Fork | Base | Patch size | Used by | Main verdict |
| --- | --- | --- | --- | --- |
| winit | 0.30.13 | 29 files, +3,690/−60 | Files, portal, Settings (via iced_winit) | Port to 0.31; upstream focus, inhibitor, parents |
| iced_winit | 0.14.0 | 6 files, +660/−41 | all ordinary Iced windows | Upstream transient/dialog/token/event batching |
| iced_runtime | 0.14.0 | 4 files, +632 | same | Public API for the above |
| iced_layershell | 0.19.1 (waycrate) | 7 files, +261/−16 | Shell, Notify | Replace output/token parts; upstream the rest |
| layershellev | 0.19.1 (waycrate) | 3 files, +243 | Shell, Notify | Same |
| accesskit_unix | 0.24.0 | 1 file, +4/−2 | Shell, Notify, session UI | **Drop**: fixed in 0.24.1 |
| reis | 0.7.1 | 2 files, +54 | portal | Upstream `read_chunk` |
| uucore | 0.12.0 | 1 file, +51 | Files | Upstream or reimplement (~50 lines) |
| Niri + Smithay | Niri b22eb842, Smithay 8a270232 | 41 files, +11,191/−230 (Niri); 8 files, +279 (Smithay) | the packaged compositor | Upstream fixes; replace shortcuts; keep capture/remote input optional |
| xdg-desktop-portal | 1.22.1 | 4 patches, +460/−97 | Canopa sessions (packaged) | **Upstream** all four |
| CUPS | 2.4.20 | 4 patches, +191/−31 | opt-in Print only | Upstream 5 fixes; drop private runtime while Print is opt-in |

Nothing in the repository records an upstream issue or PR for any fork
(ADR 0001: "No upstream pull request or public fork is published").

## winit 0.30.13

Commit `e9809ef5`. Only `iced_winit` depends on it. Layer surfaces (Shell,
Notify) are unaffected.

| # | Area | Size | What it changes | Why | Upstream (2026-11-10) | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| W1 | Wayland drag-and-drop, source and target | ~1,515 lines (41%): `drag_drop.rs`, `handlers.rs`, `pipes.rs` | Native data-device drags from the exact left press, file-URI offers, Copy/Move negotiation, offers retained after Leave/Drop, completion after the origin window closes, pipe I/O off the dispatch thread (≤16 workers, 16 MiB, 5 s) | Files drag and drop; winit 0.30 has none (ADR 0023, 0024; Files ADR 0010/0094/0095) | Merged on master (#4571, 0.31 betas); 0.30 backport refused (#4810). Upstream uses `latest_button_serial()` rather than the exact press | **Replace** with 0.31's API when Iced moves; upstream the exact-origin, retained-offer and bounded-pipe guarantees |
| W2 | Keyboard-shortcut inhibition scoped to focus | ~545 (15%) | keyboard-shortcuts-inhibit-v1 manager per surface and seat; Active only when every focused seat acknowledges | GlobalShortcuts editor must receive the keys it configures (ADR 0068) | Issue #2776 open; no code upstream | **Upstream** |
| W3 | Source-owned shortcut recording | ~1,130 (31%) | Captures base keysym plus effective modifiers before normal dispatch, swallows repeats, cancels on focus/keymap/layout change; xkb helpers | Iced key events lack effective modifiers and arrive after text fields react (ADR 0069) | Nothing upstream | **Keep**; upstream only the xkb helpers |
| W4 | Focus and activation tied to input | ~405 (11%) | Implements Wayland `focus_window()` (empty upstream) with recent-input tokens; `request_activation_token_for_input()` | Files inspector refocused the wrong Browser (ADR 0021); AppChooser needs a fresh token (ADR 0028) | `focus_window` still `{}`; issue #3633 open | **Upstream** |
| W5 | Transient and foreign parents before first configure | ~105 (3%) | `with_transient_parent`, `with_foreign_parent` (xdg-foreign-v2 plus xdg-dialog-v1 modal) | Quick Look kept its tiled size when parented late (ADR 0019); portal dialogs parent across processes (ADR 0026) | Issue #4256, PR #4627 open; no xdg-foreign | **Upstream** |

Several hunks are not rustfmt-formatted; clean them before submission.

## iced_winit and iced_runtime 0.14.0

Iced commit `3997291f`. Iced master (0.15-dev) still uses winit 0.30.13, so the
0.31 move has not started.

| # | Change | Why | Depends on winit | Verdict |
| --- | --- | --- | --- | --- |
| W1/R1 | `window::open_transient` (`OpenResponse`, parent in `CreateWindow`, set before first configure) | Floating Quick Look and dialogs at requested size (ADR 0019) | W5 | **Upstream** |
| W2/R1 | `window::open_dialog[_with_activation]` | Portal FileChooser/AppChooser parenting (ADR 0026, 0028) | W5 | **Upstream** |
| W3/R1 | `window::request_activation_token` (serial-matched, 1 s expiry) | AppChooser hands a fresh token to the chosen app (ADR 0028) | W4 | **Upstream** |
| W4 | Input-transition boundary: stop a batch at the first `State::Outdated` and requeue the tail | Alt+2, Tab, typing ran against the old Preferences tree; Ctrl+L dropped the first character (ADR 0020) | none | **Upstream**: correctness bug still on master (conflicts with master's new `update` signature) |
| W5/R2 | Drag-and-drop adapter and types (`window::drag_drop`) | Files DnD (ADR 0023) | W1 | **Keep** until an upstream API is agreed |
| W6/R3 | Shortcut inhibition and recording adapter | ADR 0068/0069 | W2, W3 | **Keep**; upstream the inhibitor alone |
| W7 | `try_next` → `try_recv` | futures-channel deprecation under strict Clippy | none | **Drop** at rebase (master already has it) |

Minor: `OpenDialog` on non-Wayland backends opens an unparented window instead
of returning `None` as documented.

## iced_layershell and layershellev 0.19.1 (waycrate)

Commit `5e39311c`. Upstream was renamed to `exwlshellev` / `iced_exwlshell` and
rewritten (v0.21.4, 2026-11-06), so a rebase is a port.

| # | Change | Why | Upstream | Verdict |
| --- | --- | --- | --- | --- |
| L1/E1 | `WaylandOutputSnapshot` event; forces `Message: From<WaylandOutputSnapshot>` | Shell plans bars from output names before any window exists (Shell ADR 0021) | 0.21 has OutputAdded/Updated/Removed | **Replace** |
| L2/E2 | `SetIdleInhibition` (idle-inhibit-v1, per unit, idempotent) | Shell idle-inhibitor module (Shell ADR 0017) | none | **Upstream** |
| L3/E3 | `RequestActivationToken` (xdg-activation from last button/touch serial, 2 s input window, 1 s reply, ≤64 pending) | Notify banner actions, Shell launches | master has `RequestActivationToken(ActivationTokenSink)` | **Replace**; port bounds if missing. Check: keyboard input records no serial, so keyboard-triggered launches may get no token |
| L4 | Retired-window cleanup after output loss (alias lookup, purge pending actions) | Actions retried forever after output loss | unverified on 0.21 | **Upstream** |
| L5 | Handle `RedrawAll`/`RelayoutAll` (ignored upstream) | Idle bars kept blank icons after async image load | not handled on master | **Upstream** |
| L6 | Explicit `None` for the new runtime actions | Required by iced_runtime additions | — | Keep while R1 exists |

## accesskit_unix 0.24.0

`src/atspi/bus.rs`, +4/−2: wrap `emit_cache_add`/`emit_cache_remove` payloads
in one-element tuples so zbus sends the AT-SPI struct signature; Orca 50.2
rejected the flattened `so` signature. Fixed upstream by #819, released in
**0.24.1** (2026-10-25). **Drop**: upgrade, then rerun the checks in
`vendor/accesskit_unix/LOCAL-PATCHES.md` (signature test, native test, Orca).
**Done 2026-11-10:** fork removed, workspace on crates.io 0.24.1; Notify's wire
signature and native accessibility tests pass. 0.24.1 requires tokio 1.54, which
deprecates `AsyncFd::new`; the nine call sites now use `AsyncFd::register` with
SAFETY notes. Orca check still to repeat on the live session.

## reis 0.7.1

`Context::read_chunk()` (+54): one bounded `recv` of at most 2,048 bytes,
1 MiB pending limit, rejects received descriptors, separates EOF from
WouldBlock. Needed so RemoteDesktop and InputCapture notice revocation between
bounded reads (ADR 0046, 0062, 0111); upstream `read()` loops unbounded and
accepts FDs. **Upstream** (small, opt-in).

## uucore 0.12.0

`buf_copy::copy_with_progress` (+51): splice through a per-call pipe in 64 KiB
chunks with a progress/cancellation callback, falling back to bounded
`io::copy`. Files needs progress and cancellation on already-open descriptors
(Files ADR 0153, 0155). Upstream exposes only `copy_fast`. **Upstream** as an
optional variant; otherwise reimplement with rustix `splice` and drop uucore,
which Files uses for nothing else.

## Niri and Smithay

Cumulative recipes, each extending the previous one:

| Recipe | Extends | Pins | Adds | Lines added over the previous recipe (approx.) |
| --- | --- | --- | --- | --- |
| niri-source-cancellation | — | Niri 8ed0da44 (26.04), Smithay ff5fa7df | Smithay drag-source lifetime fix | +81/−26 (Smithay) |
| niri-screencast-resume | — | Niri b22eb842, Smithay 8a270232 | Screencast resume fix | +13 |
| niri-desktop-compat | screencast-resume | same | Ports the drag-source fix to 8a270232 | +78/−26 (Smithay) |
| niri-remote-input | desktop-compat | same | RemoteDesktop keyboard | +1,257, Smithay +8 |
| niri-desktop-pointer | remote-input | same | RemoteDesktop pointer, stream identity, capture lifetime | ~+2,540/−195, Smithay +23 |
| niri-input-capture | desktop-pointer | same | InputCapture pointer | ~+1,117, Smithay keyboard fix |
| niri-global-shortcuts | input-capture | same | GlobalShortcuts leases | ~+3,318 |
| niri-input-capture-keyboard | global-shortcuts | same | InputCapture keyboard | ~+1,501 |
| niri-remote-touch | input-capture-keyboard | same | RemoteDesktop touch (**shipped**) | ~+1,410 |

No recipe changes `niri-config`; the escape chord Ctrl+Alt+Escape is
hardcoded. EI translation lives in `canopa-portal` (via reis); Niri speaks only
private sockets.

### Bug fixes (small, general; upstream now)

| Change | Why | Upstream | Verdict |
| --- | --- | --- | --- |
| Smithay: `is_alive()` on Pointer/TouchGrab; DnD grabs cancel when their `wl_data_source` is destroyed; dead grab unset on source destruction | GTK `drag_cancel` still produced a Drop on release in Niri (Sway is correct); broke Files cancellation (ADR 0024) | Not in Smithay master; #1968 is related but different | **Upstream to Smithay** |
| Niri `pw_utils.rs` (+13): reset damage trackers and cursor state on each PipeWire `Streaming` transition | Static windows never delivered a frame after pipewiresrc's pause/resume (ADR 0043) | Still `is_active = true; redraw()` on main | **Upstream to Niri** |
| Smithay keyboard `release_source` forwards a key-up only for forwarded presses and re-advertises modifiers | Consumed keys leaked lone key-ups on source disconnect (ADR 0065) | Not upstream | **Upstream to Smithay** |
| Smithay `KeyboardHandle::keymap_id()` | Detect keymap changes without serializing | Absent | **Upstream to Smithay** |
| Pointer button ownership (`pointer_buttons.rs`, suppressed-button API, `VirtualPointerDevice::is_alive`) | Overlapping virtual pointers duplicated presses; one source's loss released another's buttons; mid-drag loss became a drop | Not upstream | **Upstream to Niri/Smithay** |
| Cast target pinned on the compositor loop (no lookup by output name at StartCast) | Connector names can outlive their Output (ADR 0048) | PR #1966's `mapping-id` uses the output name, the same flaw | **Upstream** |
| Recording stops when its creator's D-Bus name disappears | SIGKILLed worker left recording alive (ADR 0056) | Issue #3258 proposes this | **Upstream**, referencing #3258 |
| rustfmt import reordering in `handlers/mod.rs` | none | — | **Drop** |

### Private D-Bus services (features)

| Service | Recipe | What it provides | Why | Upstream | Verdict |
| --- | --- | --- | --- | --- | --- |
| `org.canopa.Niri.RemoteInput1`: `OpenKeyboard`, `OpenKeyboardSymbols`, `KeyboardSnapshot` | remote-input | Socket lease per source with real keymap/modifiers; source-owned releases; keysym commands | RemoteDesktop keyboard (ADR 0046) | PR #1966 (Mutter RemoteDesktop on Smithay libei, with touch) open, active 2026-11-08 | **Keep, then replace with #1966**; push source-owned release and keymap retirement into it |
| `RemoteInput1.OpenPointer`, `OpenMappedPointer`; `canopa-source-id`, `mapping-id`, `canopa-physical-scale` stream parameters | desktop-pointer | Relative and stream-mapped absolute pointer; per-Output identity | Absolute input bound to the approved recording (ADR 0049–0053) | #1966 | **Keep, then replace**; use standard `mapping-id` |
| `RemoteInput1.OpenMappedTouch` | remote-touch | 40-byte touch commands on live stream mappings; contact registry where physical touch wins | RemoteDesktop touch (ADR 0117) | #1966 reports working touch | **Keep, then replace**; offer contact rules to #1966 |
| `org.canopa.Niri.InputCapture1` v1 (pointer) and v2 (keyboard, keymap revisions) | input-capture, input-capture-keyboard | Pointer barriers on outer zone edges; keyboard routing before seat XKB with separate capture state | InputCapture portal (ADR 0059–0063, 0111) | No upstream work; no standard protocol | **Keep, optional**; propose a Mutter-compatible interface on Smithay EIS |
| `org.canopa.Niri.GlobalShortcuts1` | global-shortcuts | Up to 16 leases; atomic replace; Activated/Deactivated with source timestamps; per-keyboard press ownership; conflict detection with binds and a11y grabs | GlobalShortcuts portal (ADR 0066, 0068) | **Upstream merged `org.gnome.Shell` GrabAccelerator (PR #4744, 2026-11-07, unreleased)** | **Re-evaluate now**: try the upstream API; upstream timestamp and ownership gaps |

Where the shipped patch lives: about 10,100 added lines are new modules
(`dbus/global_shortcuts*` 3,162, `dbus/remote_input*` 2,525,
`dbus/input_capture*` 2,462, `input/remote_touch*` 638, plus 134 tests). About
1,050 lines touch 12 upstream files, mostly `src/input/mod.rs` (+510/−50) and
`src/dbus/mutter_screen_cast.rs` (+272/−157). Every upstream input refactor
conflicts there.

## xdg-desktop-portal 1.22.1 frontend

Installed at `/usr/lib/canopa/xdg-desktop-portal`; a systemd drop-in runs it
only in Canopa sessions. Default descriptor routes ScreenCast, RemoteDesktop
and Clipboard to Canopa; InputCapture, GlobalShortcuts and Usb need
`--extra-portals`.

| Patch | Change | Why | Upstream main | Verdict |
| --- | --- | --- | --- | --- |
| frontend (+229/−16) | RemoteDesktop `Notify*` admitted on the D-Bus loop into one FIFO worker per session; 64-message/4 KiB/no-FD budget; overflow revokes the session | Worker threads reordered input; a 32-event burst failed on stock 1.22.1 (ADR 0047, 0053, 0058) | Still threaded; no ordering | **Upstream**. Cached authorization uses an API main made private (2026-07-31): rework first |
| session-owner (+132/−7) | Pin the backend proxy to its unique owner; on loss, latch revocation, bounded `Session.Close` to the old owner, close the public session | After SIGKILL of the backend, clients never got `Session.Closed`; a replacement backend could receive old cleanup (ADR 0054, 0055) | No owner watching | **Upstream** |
| input-capture-phase (+2/−19) | Repeated `Enable` keeps Active; `Release` waits for backend signals | Stale phase denied Clipboard reads; rejected activation desynced state (ADR 0062, 0063) | Unchanged | **Upstream** |
| usb-ownership (+97/−55) | Validate grants as a unique subset, never more writable; no late grants after Close; recheck/reopen cached FDs; `O_CLOEXEC`; fix ref leak | Implements upstream TODOs for Canopa's USB consent (ADR 0064) | TODOs and leak still present | **Upstream** |

## CUPS 2.4.20

Used only by the opt-in Print backend (`--experimental-print`); packaged
sessions route Print to xdg-desktop-portal-gtk and the system libcups. The
private `libcups.so.2` is loaded from a sealed memfd by isolated workers
(ADR 0101, 0104). The IPP backend runs only if an administrator configures it.
The receiver is a test fixture.

| Patch | Change | Why | Upstream | Verdict |
| --- | --- | --- | --- | --- |
| auth: qop/default MD5 | Negotiate `qop=auth`; missing algorithm means MD5 | RFC 7616 challenges without `algorithm=` were rejected (ADR 0101) | Fixed on master/2.5, not 2.4.x | **Replace** at 2.5; ask for backport |
| auth: legacy `opaque` | Echo `opaque` in RFC 2069 replies | Required by servers | Still missing | **Upstream** |
| auth: DigestOptions | Apply the parsed setting | DenyMD5 silently ignored | Fixed in 2.4.x (950643d9, unreleased 2.4.21) | **Drop** at 2.4.21 |
| auth: credential buffers | 128-byte users, 2,048-byte userpass, refuse truncation, capability symbol | Stock client truncates at 190 bytes (ADR 0094, 0101) | Still 256-byte | **Upstream** the truncation refusal; larger sizes only if required |
| auth: quoted pairs | Decode `\"`/`\\` in challenge fields, escape in Digest output | 30 of 48 oracle cases failed; consent showed a mis-decoded realm (ADR 0103) | Still raw | **Upstream** |
| auth: incomplete headers | EOF in headers is an error even with a cached 401 | `cupsSendRequest` looped forever (ADR 0106) | Still present | **Upstream** (a hang) |
| custom-media | `media-bottom-margin` written to `.bottom`, not `.top` | Custom paper kept a 12.7 mm bottom margin (ADR 0109) | Still present, also libcups3 | **Upstream** (one-line bug) |
| ipp-backend | Re-encode explicit `printer-resolution` after option conversion | PPD path dropped the chosen resolution (ADR 0110) | Not handled | **Upstream**; keep optional |
| ipp-receiver (test-only) | Inclusive range check; keep declared MIME formats | Valid custom sizes rejected (ADR 0107, 0108) | Still present | **Upstream** |

While Print stays opt-in, the release bundle could drop the private runtime and
keep it as an experimental extra.

## Stale documentation found

**Fixed 2026-11-10** (ADR 0001/0024/0047/0055, vendor guide, portal overlay
README and candidate.json, new READMEs for the two recipes).

- ADR 0024 ("Arch package selection") and `docs/vendor-maintenance.md`
  (around lines 148–155) still say the package builds the source-cancellation
  pair on 8ed0da44; it builds `niri-remote-touch` on b22eb842.
- `xdg-desktop-portal-input-order/candidate.json`, its README, ADR 0047, ADR 0055
  and `docs/vendor-maintenance.md` still say the patched frontend is "not
  installed or selected"; commit 8ede03f packages and selects it.
- `niri-source-cancellation/` and `niri-desktop-pointer/` have no README.

## Suggested order

1. **Drop now:** accesskit_unix (upgrade to 0.24.1), iced_winit W7, the
   rustfmt-only Niri hunk; fix the stale documentation.
2. **Upstream small bug fixes (days each):** Smithay drag-source lifetime,
   `release_source`, `keymap_id`; Niri screencast resume, creator lifetime
   (#3258), cast-target pinning, pointer button ownership; the four
   xdg-desktop-portal patches; the five CUPS fixes; reis `read_chunk`; uucore
   progress; iced_winit event batching; layershell idle inhibit, retired-window
   cleanup, RedrawAll.
3. **Re-evaluate GlobalShortcuts** against upstream Niri #4744. It is the largest
   private block (~3.2k lines plus `input/mod.rs` hooks).
4. **Track PR #1966** and plan RemoteInput and touch as a rebase onto it,
   contributing source ownership and stream identity.
5. **Port layershell to exwlshellev 0.21**, replacing output snapshots and
   activation tokens with upstream equivalents.
6. **Prepare winit 0.31**: upstream W2, W4, W5 there; move Files DnD to the
   upstream data-transfer API; keep W3 local.
7. **InputCapture:** open an upstream discussion; keep it optional meanwhile.

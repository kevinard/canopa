# GlobalShortcuts contract and Niri registration boundary

Historical research checkpoint. The newer experimental Rust/Iced v2 implementation
and scoped acceptance are recorded in [session evidence](global-shortcuts/session-evidence.json)
and ADR0067. Statements about unimplemented work below describe the original
investigation, rather than the current backend.

Status: research only, 2026-11-01. GlobalShortcuts remains unimplemented and
unadvertised. No build, native probe, live service call, production-source edit,
installation or routing change was performed. This note is the only written
artifact. The ongoing shared-workspace validation is independent of this research.

A subsequent, separate [native follow-up](global-shortcuts/README.md) reproduces
the predicted modifiers-first missing release on the frozen compositor. The
contract/source findings below remain the original research scope; GlobalShortcuts
is still unimplemented.

The [roadmap:15–20][roadmap] records eleven packaged interfaces; ScreenCast,
RemoteDesktop, Clipboard and InputCapture remain experimental, while
GlobalShortcuts, Print and Usb need implementation. The inspected
[service construction:550–569][main] and [descriptor:3][descriptor] contain no
GlobalShortcuts registration. The Rust/Iced backend and standard-frontend ownership
split in [ADR0026][adr26], activation ownership in [ADR0032][adr32], and original
backend supervision in [ADR0055][adr55] remain applicable.

## Sources and exact scope

The public and backend contracts are the official
[public reference][public-doc], [backend reference][impl-doc], and tagged 1.22.1
[public XML][public-upstream]/[backend XML][impl-upstream]. Exact local evidence
uses the canonical Canopa-patched frontend at
`/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1`, not the installed
frontend. Niri evidence uses the prepared, patched tree at
`target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8`.
Its directory revision is an upstream base pin, not a claim that the prepared
tree equals upstream. Niri conclusions below are source inspection, with
behavioral deductions explicitly marked; no physical acceptance follows.

## Contract to implement

Both interfaces currently describe **version 2**, with lowercase read-only
`version:u`. The frontend publishes the backend version clamped to 1–2 and only
forwards ConfigureShortcuts for backend v2.
[Frontend:646–647, 821–824][frontend-configure].

| Method | Public `org.freedesktop.portal.GlobalShortcuts` | Backend `org.freedesktop.impl.portal.GlobalShortcuts` |
| --- | --- | --- |
| CreateSession | `(options:a{sv}) → request:o` | `(handle:o, session_handle:o, app_id:s, options:a{sv}) → response:u, results:a{sv}` |
| BindShortcuts | `(session_handle:o, shortcuts:a(sa{sv}), parent_window:s, options:a{sv}) → request:o` | `(handle:o, session_handle:o, shortcuts:a(sa{sv}), parent_window:s, options:a{sv}) → response:u, results:a{sv}` |
| ListShortcuts | `(session_handle:o, options:a{sv}) → request:o` | `(handle:o, session_handle:o) → response:u, results:a{sv}` |
| ConfigureShortcuts, v2 | `(session_handle:o, parent_window:s, options:a{sv}) → ()` | Same arguments and empty reply; **no Request handle or response tuple**. |

Backend signatures: [XML:47–135][impl-xml]. Public Create/Bind/List complete
through `Request.Response`. Public Create returns `session_handle` as variant
type **`s`**, despite containing an object path; backend Create defines no result
keys. The other two return `shortcuts:a(sa{sv})`.
[Public XML:68–77, 120–140, 172–186][public-xml].

Create accepts string `handle_token` and `session_handle_token`; Bind and List
accept `handle_token`. The pinned frontend does not forward a Bind
`activation_token`: only Configure defines that input option.
[Frontend option tables:232–235, 399–401, 532–534, 628–630][frontend-options].

Bind input tuples carry `description:s` and optional `preferred_trigger:s`.
Returned tuples instead carry `description:s` and `trigger_description:s` and may
be any subset, including empty. The trigger is a preference for user review,
not application authority. Bind may be attempted once per session; v2
ConfigureShortcuts subsequently opens configuration, accepting optional
`activation_token:s`. List before Bind describes earlier successful bindings
for that application; after Bind it describes active session bindings.
[Public XML:94–140, 155–180, 189–209][public-bind].

The [XDG shortcut grammar][shortcut-spec] uses `+` between named modifiers
`CTRL`, `ALT`, `SHIFT`, `NUM`, `LOGO` and an xkbcommon keysym name. Layout/base-level
resolution belongs to the implementation. It is not the same language as Niri
KDL binds: `Mod` is compositor-specific; `LOGO` needs translation to Logo/Super;
numeric modifier masks must be derived from the actual keymap. Niri's normal
`Modifiers` lacks NUM, so blindly reusing that type loses a specified modifier.
[Niri binds:179–189][niri-modifiers].

### Session and request ownership

The frontend supplies authenticated application attribution, rejects empty
application IDs, filters options, and forwards only `description` and
`preferred_trigger` input properties. Its inspected Bind handler does **not**
track the once-only attempt or reject duplicate IDs. Backend validation must
cover those invariants itself.
[Frontend:253–260, 358–396, 404–487][frontend-bind].

The public session lookup checks both original unique sender and application ID.
Canonical session supervision pins the original backend owner, latches loss or
replacement, denies further lookup and retires the original owner without
redirecting cleanup to a replacement. This already covers GlobalShortcuts v1 in
ADR0055's frontend fixture; it does not establish a working shortcut backend.
[xdp-session.c:69–113, 288–352, 442–483][frontend-session], [ADR0055][adr55].

**Proposed backend policy:** create a distinct session kind with stored invoking
frontend unique sender, app ID, terminal state and one Bind-attempt latch. Later
Bind/List/Configure calls have no `app_id` argument: resolve it from that stored
session, rejecting foreign owners/kinds. Export impl Session and per-operation
Request objects; Close/disconnect/cancellation retire only their owned work.
Reuse [Requests:83–154][requests] for bounded request ownership and cancellation;
do not mistake the frontend's unique sender for the original application's
public sender. A canceled or failed Bind must not leave an active provisional
registration. Consent persistence is a separate application/action-ID store;
historical entries do not by themselves install live compositor grabs.

### Signals, timing and activation context

Both sides use `Activated(o session_handle, s shortcut_id, t timestamp,
a{sv} options)`, `Deactivated` with the same signature, and
`ShortcutsChanged(o session_handle, a(sa{sv}) shortcuts)`.
The timestamp is a required **u64 millisecond event time with unspecified epoch**.
It is not a Unix timestamp, XKB serial or application-supplied time.
`activation_token:s` is optional in both activation/deactivation options.
[Backend XML:138–202][impl-signals], [public reference][public-doc].
**Inference:** portal receipt time is not evidence of compositor event time;
the existing a11y signal cannot recover the latter after D-Bus delay.

The historical [v1 public XML][v1-public] and [v1 backend XML][v1-impl] already
require the timestamp argument. V1 does not offer an escape from missing event
time; v2 adds ConfigureShortcuts. Neither the current v2 contract nor its signal
signature requires a nonempty activation token. Token omission can be honest;
token fabrication is unnecessary.

The frontend forwards timestamps/options unchanged to the original session's
public sender and drops events for revoked/closed sessions. It creates no token.
[Frontend:719–792][frontend-signals]. Niri can mint an external token on a genuine
compositor-owned action, as Spawn already does. A background Wayland client's
serial-free token instead becomes urgency-only under Niri's normal policy.
[Niri input:1008–1014][niri-token], [activation handler:748–824][niri-activation].
**Inference:** if tokens are offered, mint them at the accepted compositor
shortcut edge, preserve event/session identity through delivery, and test actual
consumption. A portal process cannot reconstruct input authority from a keysym.

The XML does not prescribe auto-repeat policy, which modifier release ends an
activation, or a total ordering between user input and the asynchronous Bind
response. Define those policies explicitly. A useful proposed contract is one
activation per accepted press cycle, a corresponding deactivation when its
tracked trigger is released/canceled, and no repeat activation storm. Publish
rebind metadata after an atomic compositor commit; retire old press ownership
before the old trigger can be assigned elsewhere. Do not promise that every
activation waits for the application's Request.Response unless the implementation
actually enforces that barrier.

## What the pinned Niri actually provides

### Existing dynamic grabs: accessibility, not portal sessions

The service **does exist**: `org.freedesktop.a11y.Manager`, object
`/org/freedesktop/a11y/Manager`, interface
`org.freedesktop.a11y.KeyboardMonitor`. It is started in session instances, or
when the non-session D-Bus debug option is enabled.
[Registration:178–190][niri-dbus], [a11y:104–110, 566–581][niri-a11y-start].

| Existing capability | Exact inspected behavior |
| --- | --- |
| Dynamic registration | `SetKeyGrabs(modifiers:au, keystrokes:a(uu)) → ()`; keystroke tuples are `(keysym, effective XKB modifier mask)`. Replaces the calling unique sender's complete set. Empty arrays clear it. |
| Ownership | State is keyed by unique D-Bus sender; disconnect removes the client and rebuilds the grabbed-modifier set. This is connection ownership, without portal app/session IDs. |
| Events | Directed `KeyEvent(released:b, state:u, keysym:u, unichar:u, keycode:q)`. `released=false` means press; `state` is the current effective XKB mask; keycode is hardware-dependent and cast to u16. |
| Missing event metadata | No event timestamp, device/KeyboardSource identity, registration ID, portal session, serial or activation token. Niri has source time internally but does not send it in KeyEvent. |
| Admission/conflicts | Mutation returns success without a conflict result, accepted subset or count bound in this method. Multiple clients can register the same stroke; every matching client can receive it. No compositor/user-bind reservation check occurs here. |
| Broader privilege | WatchKeyboard observes all keyboard events; GrabKeyboard takes all keyboard events. A portal adapter must not require these to compensate for incomplete per-shortcut events. |

Sources: [SetKeyGrabs:192–231][niri-grabs], [KeyEvent:234–249 and
process_key:252–351][niri-events], [matching:364–403][niri-a11y-match],
[disconnect cleanup:506–545][niri-a11y-cleanup], [XKB input:626–658][niri-a11y-input].

**Important source-predicted failure, not a native result:** register Ctrl+a,
press Ctrl then a, release Ctrl before a. The a press matches and is delivered.
At the a release, `should_watch_keypress` rechecks the *current* effective mask,
which no longer contains Ctrl, so the per-keystroke client can miss its release
signal. The global suppressed-keys path may still consume that release. If a
modifier/group change also changes the keysym, even suppression matching can
differ. There is no per-client captured press recipient keyed by source/keycode
that guarantees paired release delivery. A similar held/rebind/disconnect case
cannot be solved merely by adding a portal-side map after events were omitted.
[Emission before suppression:270–342][niri-events], [current-mask
matching:383–403][niri-a11y-match].

Accessibility handling runs before ordinary bind matching, lock/screenshot
filters and the compositor's InputCapture escape check. It receives effective
XKB state before the current event changes that state. Normal remote/virtual
keyboard input reaches this path too; KeyEvent recipients cannot tell the
input's origin. This is not existing portal conflict, lock or source policy.
[on_keyboard:449–550, 623–659][niri-keyboard], [a11y XKB:638–658][niri-a11y-input].

### Existing normal-bind primitives and absent portal boundary

Niri's regular bind path already resolves modified/current-layout/ASCII fallback
keysyms, selects ordered bind sets, suppresses consumed keys, and tracks a
pending release by keycode independently of later modifiers. It has repeat,
cooldown, lock and inhibition policies; release-only binds have interruption
semantics, unlike press-and-release binds. Those are useful primitives, not
session-owned portal registration.
[Resolution:554–621][niri-keyboard], [matching/release:5066–5151][niri-match],
[bind precedence:5969–5995][niri-bind-order], [held cleanup:897–940][niri-held].

The inspected IPC Request enum has no dynamic binding request, and the D-Bus
module list has no GlobalShortcuts/GrabAccelerator service. Regular bind
iteration reads configuration/UI state rather than a portal registry.
[IPC:64–122][niri-ipc], [D-Bus modules:11–25][niri-dbus],
[bind iteration:5969–5995][niri-bind-order]. The Mutter ServiceChannel recognizes
service type 3's name but calls itself a stub and opens an unrestricted Wayland
connection; it is not shortcut registration or consent.
[ServiceChannel:18–36][niri-service-channel].

## Smallest faithful implementation seam — proposal, not support

1. **Add a narrow leased shortcut registry in Niri.** Preserve the a11y ABI and
   behavior. Reuse its proof that dynamic grabs and directed events are possible,
   but use the ordinary input matcher for explicit precedence. A bounded
   `Open/Replace/Close` lease API can carry stable registration IDs, atomic
   accepted/conflict results, generation and a nonblocking event stream containing
   `(registration, edge, event_time_ms)`. Bind lease lifetime to the original
   connection/FD; slow output retires that lease rather than blocking input.
   Existing bounded calloop/FD leases provide a local pattern, not shortcut
   semantics: [D-Bus channels:100–124][niri-dbus], [RemoteInput admission/open and
   feedback:44–61, 106–126, 260–297][niri-lease].
2. **Capture press ownership in the compositor.** Keep source/keycode,
   registration generation and recipient from the accepted press through its
   release. Handle modifiers-first release, layout change, source removal,
   repeat, rebind and revocation there. Preserve existing emergency/VT actions,
   a11y behavior, lock and inhibitor policies. Report admission conflicts with
   user binds and other leases; recompute on configuration/keymap changes.
   The XDG API does not dictate the conflict winner, so document the chosen
   policy. Initial proposal: reserve compositor/user binds and existing accepted
   leases, and let Iced offer another trigger rather than stealing one.
3. **Build the Rust session/consent owner independently.** Use the existing
   request/helper lifecycle with Iced/shared appearance to show application,
   action descriptions, triggers and conflicts. Commit only accepted choices;
   cancellation retains no new authority. Keep requested input separate from
   accepted results, persist accepted app/action bindings only under an explicit
   policy, and expose a working v2 reconfiguration window. Cancellation of
   configuration preserves the previous live set. Bound Configure calls even
   though this method has no Request object. Rebinding and saved history are
   portal policy, while actual match/admission and event time belong to Niri.
4. **Translate committed compositor events to directed impl signals.** Retain
   exact session/action/generation association, emit truthful descriptions, and
   revoke on frontend/backend/compositor owner change without following a new
   owner. Pin/validate the companion as existing workers do
   ([remote_input.rs:98–153][worker-owner]). Activation-token generation is a
   separately testable extension, not a reason to fake support or delay a
   token-free implementation that meets the remaining contract.

A smaller **a11y-only laboratory adapter** could demonstrate one nonconflicting
stroke and connection cleanup today. It cannot establish faithful release/time,
conflict, lock or source behavior. V1 can reduce UI/API scope by omitting
ConfigureShortcuts, but it does not remove the timestamp/release requirements;
the canonical frontend already supports v2, so v2 is the proposed target.

## Required acceptance before advertisement

None of these checks was run here. Source availability and a nested synthetic
input fixture are not physical-input or distribution acceptance.

| Layer | Meaningful evidence to retain |
| --- | --- |
| Wire/frontend | Exact introspection against pinned XML; separate backend `(u,a{sv})` outputs; public string-typed session result; real recognized app identity; v1/v2 version handling; private descriptor and actual standard frontend calls. |
| Consent/session | Deny missing/foreign owner and wrong session kind; duplicate IDs/Bind attempts; malformed/oversized trigger metadata; empty/partial/full grant; unchecked denial; Request.Close and caller loss during review/commit; List before/after Bind and across restart; v2 Configure cancellation and real rebind. |
| Input pairing | Press/hold/release with application unfocused; Ctrl released before trigger, Shift changing keysym, group/lock changes; repeated key and two keyboards holding the same key; device removal, focus/VT/suspend loss, held rebind/Close. Prove no missed/stale edge or unpaired ordinary-client event and source-time millisecond units. |
| Admission/policy | Collisions with configured/hardcoded binds, another portal session and a11y; lock/unlock, shortcut inhibitor and screenshot/MRU UI; config/keymap reload invalidation. Exercise physical, virtual keyboard and RemoteDesktop sources under the explicitly selected origin policy. |
| Native presentation | Actual Rust main executable, Iced consent/configuration windows, exported parent and incoming activation context, keyboard navigation and conflict choice in dark/100% and light/125%. Screenshots plus application-visible changed trigger metadata and new/old trigger behavior. |
| Activation context | Token-free signals work. If tokens are supplied, consume them on a real target window under normal compositor policy, check per-edge/session isolation, expiry/replay and closure while waiting; no global invalid-serial debug exception. |
| Failure/lifetime | Public app Close/disconnect, frontend loss/replacement/reacquisition, backend death/replacement, companion/UI failure and bus loss. Confirm public Closed where required, original-owner cleanup, removed grabs, bounded queues, terminal worker/FD handles and no revival on owner reacquisition. |
| Integration/distribution | Preserve existing bind/a11y and capture/remote-input regressions. Apply provenance and replay checks when the companion changes; run the required shared-workspace gate once after relevant edits. Separately verify real Flatpak, actual session/companion selection and packaging before normal routing. |

The next implementation decision is therefore the compositor lease/event seam,
followed by session-owned Iced consent and configuration. Existing dynamic a11y
grabs are real capability evidence; the missing portal contract, native matrix,
physical acceptance and compatible distribution remain explicit work.

[roadmap]: /home/kevinard/projects/canopa/.scratch/xdg-portals/ROADMAP.md:15
[main]: /home/kevinard/projects/canopa/crates/canopa-portal/src/main.rs:550
[descriptor]: /home/kevinard/projects/canopa/packaging/portal/canopa.portal:3
[adr26]: /home/kevinard/projects/canopa/docs/adr/0026-file-chooser-portal.md:5
[adr32]: /home/kevinard/projects/canopa/docs/adr/0032-panel-action-activation-context.md:17
[adr55]: /home/kevinard/projects/canopa/docs/adr/0055-frontend-session-owner-lifetime.md:1
[public-doc]: https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html
[impl-doc]: https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.GlobalShortcuts.html
[public-upstream]: https://github.com/flatpak/xdg-desktop-portal/blob/1.22.1/data/org.freedesktop.portal.GlobalShortcuts.xml
[impl-upstream]: https://github.com/flatpak/xdg-desktop-portal/blob/1.22.1/data/org.freedesktop.impl.portal.GlobalShortcuts.xml
[v1-public]: https://github.com/flatpak/xdg-desktop-portal/blob/1.18.4/data/org.freedesktop.portal.GlobalShortcuts.xml#L196-L225
[v1-impl]: https://github.com/flatpak/xdg-desktop-portal/blob/1.18.4/data/org.freedesktop.impl.portal.GlobalShortcuts.xml#L110-L139
[shortcut-spec]: https://specifications.freedesktop.org/shortcuts/latest/
[impl-xml]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/data/org.freedesktop.impl.portal.GlobalShortcuts.xml:47
[impl-signals]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/data/org.freedesktop.impl.portal.GlobalShortcuts.xml:138
[public-xml]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/data/org.freedesktop.portal.GlobalShortcuts.xml:68
[public-bind]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/data/org.freedesktop.portal.GlobalShortcuts.xml:94
[frontend-bind]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/global-shortcuts.c:358
[frontend-options]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/global-shortcuts.c:232
[frontend-configure]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/global-shortcuts.c:633
[frontend-signals]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/global-shortcuts.c:719
[frontend-session]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/xdp-session.c:69
[requests]: /home/kevinard/projects/canopa/crates/canopa-portal/src/requests.rs:83
[worker-owner]: /home/kevinard/projects/canopa/crates/canopa-portal/src/remote_input.rs:98
[niri-dbus]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/dbus/mod.rs:11
[niri-a11y-start]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/dbus/freedesktop_a11y.rs:104
[niri-grabs]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/dbus/freedesktop_a11y.rs:192
[niri-events]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/dbus/freedesktop_a11y.rs:234
[niri-a11y-match]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/dbus/freedesktop_a11y.rs:364
[niri-a11y-cleanup]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/dbus/freedesktop_a11y.rs:506
[niri-a11y-input]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/dbus/freedesktop_a11y.rs:626
[niri-keyboard]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/input/mod.rs:449
[niri-token]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/input/mod.rs:1008
[niri-activation]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/handlers/mod.rs:748
[niri-match]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/input/mod.rs:5066
[niri-bind-order]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/input/mod.rs:5969
[niri-held]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/input/mod.rs:897
[niri-ipc]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/niri-ipc/src/lib.rs:64
[niri-modifiers]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/niri-config/src/binds.rs:179
[niri-service-channel]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/dbus/mutter_service_channel.rs:18
[niri-lease]: /home/kevinard/projects/canopa/target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/dbus/remote_input.rs:44

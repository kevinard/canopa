# GlobalShortcuts lease implementation history

The public Rust/Iced v2 backend is now experimental and scoped-verified under
ADR0067; see [session evidence](session-evidence.json) for current work. The
preparation and implementation order below are historical.

2026-11-01. The [canonical compositor integration](COMPOSITOR.md) supersedes the
staged candidates below: 53 actual-source core tests and 50 scoped native groups
pass, with its required workspace gate complete: the same ten Files failures
remain, while all other Rust and post-Rust stages pass. The Rust transport passes
fifteen unit tests and three native groups; see transport-evidence.json.
Public Rust/Iced sessions and
configuration remain open. The historical preparation below is not a working
public portal or an advertisement decision. The USB checkpoint has completed its required gate and final native checks.
The forwarded-press source teardown correction below is now canonical and
passes 37 scoped native groups; its final required workspace gate completes with
the same ten Files failures and all other Rust/post-Rust stages passing.

## Registration core candidate

The stable source candidate is `/tmp/canopa-global-shortcuts-23/registry.rs`;
a checksum-bound evidence copy and test receipt live under
`target/portal-global-shortcuts-23/`. The initial combined registry/press/state/resolver candidate passes 31 isolated
tests using the recorded Niri nightly compiler and its existing xkbcommon build,
without another Cargo graph or per-case compositor copy. This does
not establish Niri compilation, source integration or input/session acceptance.

The core accepts bindings only after compositor keymap resolution. Each action
has a nonzero registration ID, a finite set of resolved base-layer keycode/layout slots and named logical
modifiers. Different layouts may map A and Q to the same physical positions
without a conflict: current-layout slots differ. Exact keysym synonyms still
conflict. Current non-ASCII keys can use Niri's first ASCII-layout fallback. Resolving to codes lets differently named keysyms that
share a physical trigger conflict honestly. A registration set commits entirely
or leaves its previous set intact. Conflicts cover the current desktop/a11y
reservations, another lease and duplicate triggers in the proposed set. Replies
contain at most one reason per action. Successful commits return the exact retired
generation/targets for press cleanup. Closing a lease frees only its own set;
lease and generation counters never wrap or alias old targets.

Bounds are 16 leases, 64 bindings per lease, 16 resolved codes per binding and
512 resolved codes per lease. The candidate treats Num Lock as optional unless
NUM was explicitly requested; both forms overlap for conflict checking. This is
a proposed Canopa policy, not an extra requirement of the
[shortcut grammar](https://specifications.freedesktop.org/shortcuts/latest/).
A real-XKB resolver now covers current-layout slots, named modifier indexes,
base-level resolution and ASCII fallback. Its original false cross-layout
conflict is retained as resolver-layout-red-02.log; native integration and the
broader modifier/layout matrix remain open. Unknown modifier bits never silently match an ordinary
combination. The compositor adapter must derive masks by XKB modifier names.

## Press and timestamp integration findings

The current Niri input filter runs after accessibility grabs and XKB seat-state
updates. Normal configured/hardcoded binds, lock/inhibitor/screenshot/MRU policy
and compositor escape must retain precedence. Portal capture of a new press
belongs after those checks. Release delivery must use captured press ownership,
not another current-modifier/keysym lookup.

The Smithay seat combines held keys across KeyboardSource identities and omits
the filter for duplicate presses/nonfinal releases. Virtual and private remote
keyboards already have distinct identities; physical devices currently share
KeyboardSource::MAIN. A source/keycode ledger and aggregate action activation
need explicit handling before/after that combined-seat transition. Merely adding
code to the existing filter closure misses some source edges. Device identifiers
are unique only within a backend instance and at a point in time, so a source
adapter must retire identities on removal and avoid reusing a stale press owner.
Two-keyboard/source-removal and layout-switch proofs remain mandatory.

`InputTime` stores microseconds in u64; `millis()` casts the quotient to u32.
Use `event.time().micros() / 1000` for the public u64 event time, retaining all
precision the backend actually supplied. Synthetic retirement can use the actual
monotonic clock's `as_micros() / 1000`; the convenience InputTime::now() also
passes through the wrapping millisecond constructor. Source timestamps need not
be strictly monotonic. Do not substitute D-Bus receipt time or invent an epoch.
These are exact inspected source facts, not physical input acceptance.

## Reproduced source-disconnect suppression bug

A separate native probe compares ordinary key cleanup, explicit consumed-key
release and consumed-key source disconnect on the unchanged frozen compositor
`9b9ba92c383332ff6f0c7cd2eecff0220d763a96b4fbb595dd2ce214a12b114e`.
Two successful baseline observations reproduce the defect:

- An unbound F7 reaches the receiver as press then release on disconnect.
- A desktop-bound F6 press and explicit release both stay suppressed.
- The same bound F6 held until private RemoteInput lease EOF reaches the receiver
  as a release alone. No focus/keymap transition appears in the observed interval.

The source and receiver are independent clients. An F8 marker before disconnect
proves the held press has been processed; a second marker after EOF orders the
receiver observation. No source teardown is used as a proxy for an explicit release.
All private processes are terminal and guarded source/binary pins match. See
source-close-baseline-02 and -03/summary.json; source-close-red-04 is the explicit
expected-clean failing regression. The first probe's timeout was a fixture error:
raw keyboard leases publish changed modifier state, not an acknowledgement for
every key. The corrected probe uses ordered receiver markers instead.

The root cause is in existing teardown: RemoteInput calls
KeyboardHandle::release_source, which updates seat state and forwards final key
releases without running Niri's normal suppression filter. Removing suppression
bookkeeping afterward cannot retract a release already sent. Virtual-keyboard
teardown is different: it sends its synthetic releases through the normal input
path before DeviceRemoved. Do not assume those paths are equivalent.

The first two-file callback candidate passed nine teardown cases, but the tenth
case reproduced a lone release after an accessibility-consumed press. Niri's
ordinary suppression set does not describe all seat-level interception.

The final canonical correction changes only Smithay's keyboard module relative
to the original combined recipe. It uses the existing forwarded_pressed_keys
ledger: source-owned state is always released, while ordinary key-up is sent
only for a forwarded press. Modifier state is still advertised. Niri's original
RemoteInput caller is restored, with no new callback API. The same-cache build
passes on the pinned nightly compiler. All ten final teardown cases, thirteen raw
and twelve typed keyboard groups pass on frozen compositor 77065c7a…. Two public
legacy-keyboard/EIS groups also pass with real monitor frames and three terminal
input workers. The final workspace gate (workspace-02) completes with the same
ten Files failures (596 passed, 41 ignored); every other Rust and post-Rust stage
passes, with unchanged guarded sources. Net storage falls 94,486,528 bytes during
that check as obsolete evidence is compressed. Wider physical and distribution
acceptance remain open. The earlier callback gate completes with
the same ten Files failures and all other Rust/post-Rust stages passing. This
prerequisite corrects ordinary Wayland delivery; it does not fix the separate
accessibility monitor's modifier-order signal-pairing gap or implement shortcuts.

## Next implementation order

1. Retain the completed correction's workspace result and all ten broader Files
   failures. The USB checkpoint is complete at its recorded scope.
2. Retain the canonical forwarded-press correction and its source/native proof
   while integrating the next feature; preserve the old frozen failing control.
3. Integrate the bounded registration core, source-aware press recipients and
   keymap/config revalidation with a narrow compositor-owned lease/event channel.
   Decide origin, inhibitor, repeat and conflict recovery policies explicitly;
   do not recover missing releases with a broad keyboard watcher.
4. Add owned Rust v2 sessions, once-only Bind, saved application/action choices,
   actual Iced selection/conflict editing and ConfigureShortcuts. Prove directed
   signals with source event time and actual old/new trigger behavior.
5. Complete the full native/lifetime/layout/physical/sandbox/distribution matrix
   and shared validation before normal advertisement. Token-free signals are
   permitted; any offered activation tokens require their own real source proof.


## Staged core checkpoint (historical)

The stable candidate directory now contains registry.rs, presses.rs, state.rs
and resolver.rs, plus engine.rs, wire.rs and host.rs. The current combined
candidate passes 51 tests with denied compiler warnings on the pinned nightly
and existing cached xkbcommon/serde/serde_json dependencies. It adds no Cargo graph.
Registry changes and held-recipient retirement share one State
boundary. A failed reconfiguration preserves the old live action; a committed
change retires its old generation before another press. Source/key cycles retain
the original target through modifier/layout changes, repeat and source loss.
Source-time u64 milliseconds are retained past the u32 wrap. Bounded overflow
retires authority and keeps suppression tombstones until actual seat-up.

The compositor adapter must still supply actual source identities, the current
per-key layout, ordinary/a11y reservations, post-update seat reconciliation and
lifecycle invalidation. Resolver candidates are layout-qualified; the previous
code-only design falsely made fr/us Ctrl+A and Ctrl+Q conflict. The corrected
real-keymap test accepts both and verifies each current-layout action. Shared
keymap access must remain borrowed on the compositor thread, and serialized
modifiers must be translated by the current keymap's names. No held release is
resolved again against the new layout.

The Engine retains approved logical definitions separately from live resolved
authority. A keymap/reservation update clears old sets before re-admitting them
on the compositor thread, retires old active recipients and reports the actual
accepted subset. Temporarily unavailable/conflicting choices retain no live
authority; later revalidation may restore the already-approved choices. FD Replace
requires the observed generation, so a stale configuration cannot overwrite a
newer set. Failure preserves the previous live set and pressed recipient.

Host owns the Engine and each actual nonblocking Unix socket together. Four-byte
little-endian lengths bound JSON to 32 KiB; dispatch handles at most eight commands.
Sequences are nonzero and strictly increasing. List and Replace reject unknown
fields; a retained red test shows Serde's unit List variant accepted an extra
field until it became an empty struct variant. Closed/malformed/blocked sockets
retire only their lease, retaining held-key suppression until seat-up. Socket
Drop shuts down the stream even if calloop retains a registration descriptor.
Deactivated precedes Changed after revalidation; event milliseconds preserve the
source u64 value. Native Niri input and public portal signal acceptance remain open.

The first socket run was restricted by sandbox EPERM on socket writes/shutdown.
An owned blocked test was identified by executable/start time and stopped with a
pidfd; the unchanged event-delivery case passes on the host. Host-07 then exposed
the actual unknown-field defect (50 pass/1 fail); host-09 passes all 51 with source
and binary pins unchanged. Receipts are core-host-07.json, wire-red-08.log and
core-host-09.json. Tests use finite client read/write timeouts, and obsolete raw
harnesses are archived losslessly instead of retaining each executable.

## Next Niri integration constraints

- Host::open returns the lease ID and a cloned nonblocking readability stream.
  Register that stream with calloop; dispatch removes it when Host::dispatch
  returns false. A failed calloop insertion must close the Host lease. D-Bus
  admission still needs finite pending/live permits and an authenticated backend
  adapter; no public portal advertisement follows from this private FD.
- Call raw_edge for every source edge, before Smithay can absorb duplicate
  presses or nonfinal releases. Capture new presses only after accessibility,
  configured/hardcoded binds, lock/inhibitor and compositor UI policy. Finish
  seat reconciliation even when input_intercept returns None. If a captured old
  cycle meets a newly grabbed a11y modifier, do not skip its seat release through
  the ModifierFirstPress early return. No new ordinary bind can claim that hold.
- Virtual and RemoteInput already have distinct KeyboardSource values. Physical
  devices share MAIN today; use backend device TypeId plus Device::id as the
  lookup key, assign a fresh nonzero source identity, and retire it on removal.
  Do not reuse a disconnected device's press owner or pass zero to the core.
  RemoteInput's direct release_source must separately notify Host::source_closed
  with actual post-release pressed_keys, since it does not run the input filter.
- Keep keymap access borrowed on the compositor thread. Inside input_intercept,
  use the already-held KeysymHandle XKB access rather than locking the keyboard
  again. Outside that filter, with_xkb_state can build owned numeric snapshots.
  Resolve the actual per-key layout; source times use micros()/1000 in u64.
- Revalidate only when keymap_id or actual configuration/a11y reservations
  change. Calling it each frame/input would continuously retire active actions.
  The config Rc itself is stable across reload; pointer identity is insufficient.
  Layout-group changes use current per-key candidates without rebuilding a map.
- Admission must reserve the ordinary matcher's modified/current-base/ASCII
  fallback behavior, generic modifiers, recent-window binds, VT/power actions
  and existing a11y grabs. A keysym-to-code-only reservation check is too weak.
  A11y reservation access/revision must preserve its existing ABI and remain
  separate from its known monitor signal-pairing defect. No WatchKeyboard or
  broad grab may compensate for missing per-shortcut ownership.

The builder now explicitly recognizes niri-global-shortcuts. Its complete Niri
patch remains relative to the original archive, with the validated Smithay patch
and licences referenced. Keep the stable prepared source path, nightly and
compositor Cargo cache. The original staged files in /tmp/canopa-global-shortcuts-23
are historical and must not be copied over the canonical prepared modules.

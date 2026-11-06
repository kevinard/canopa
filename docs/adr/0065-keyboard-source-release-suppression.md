# Keyboard source teardown retains ordinary-bind suppression

Status: canonical forwarded-press refinement; final build and 37 scoped native
groups pass, required workspace check completed with ten Files failures,
2026-11-01. The earlier callback candidate passes its initial scope but
fails the added accessibility-grab case. GlobalShortcuts registration, public
Rust sessions and Iced configuration remain separate work. Installed programs
and normal routing are unchanged.

The private RemoteInput lease owns a KeyboardSource and releases it on EOF,
protocol failure or revocation. Smithay's existing release_source updates the
combined seat state and directly forwards final releases. That path intentionally
does not run Niri's normal binding filter. A press consumed by an ordinary desktop
binding can therefore reach the focused Wayland client as a release alone.
Clearing suppressed_keys afterward cannot retract an event already delivered.

The retained native baseline reaches that actual boundary. An independent
receiver sees an unbound F7 press/release pair on source disconnect. A configured
F6 press and explicit release are both suppressed. Holding the same bound key
until source EOF instead delivers a lone release. The initial nine-case fixture
also reproduces the problem for a consumed modifier, overlapping sources,
typed-symbol input, mixed ordinary/consumed keys and a press/release binding.
Ordered F8 markers prove processing before and after EOF, without attributing
source teardown to an explicit release. Receiver logs distinguish key delivery
from focus or keymap changes.

The first candidate added a filtered release API and had Niri consult its normal
binding suppression set. Nine native cases passed, but an accessibility-consumed
press still leaked a release. That set was the wrong ownership boundary for a
shared seat operation.

Smithay already owns forwarded_pressed_keys, including the projected held state
used when focus changes. The refined release_source always removes the source's
seat state, then forwards a final key-up only when a corresponding press was
forwarded. Intercepted modifier state is still advertised. This removes the new
callback API and restores Niri's caller to its original code. Pending release
actions remain unexecuted during teardown; an explicit release still runs them.
The correction covers ordinary-client delivery, without changing the accessibility
monitor ABI or claiming its separate modifier-order signal-pairing gap is fixed.

The combined niri-input-capture recipe retains the official Niri/Smithay pins and
GPL/MIT attribution. Complete canonical patches remain relative to the original
archives; the Smithay patch is now owned by this recipe so other recorded recipes
keep their prior bytes. Exact old/new 5,963-file materializations prove that the final delta from the
original compositor is one source: Smithay's keyboard module. Niri's original
RemoteInput caller is restored. Only the
two validated stable Smithay dependency paths are normalized. The existing source
paths, incremental dev cache, one job and pinned nightly compiler are reused.
The original frozen compositor is retained as a failing control, and the candidate
is stored once per checksum.

The superseded callback candidate passes nine source-disconnect cases, thirteen existing raw
keyboard lease groups and twelve typed-symbol lease groups. Two actual public
portal keyboard regressions also pass, including concurrent Notify forwarding,
real monitor frames and the transition to EIS. Their three input workers are
terminal. No physical-input, complete capture/clipboard matrix, distribution or
GlobalShortcuts support follows from these checks. Its required workspace gate completes with the same ten Files failures; all other
Rust and post-Rust checks pass. The final forwarded-press refinement passes all
ten teardown cases, including accessibility-consumed source closure, plus the
same thirteen raw, twelve typed and two public keyboard/EIS groups. Its three
input workers are terminal and guarded binaries/sources are unchanged. The final
required workspace check completed separately as workspace-02: the same ten
Files integration failures remain (596 passed, 41 ignored), and all other Rust
and post-Rust checks pass with unchanged guarded sources. The callback gate
remains historical evidence rather than validation of the refined patch.

The separate staged shortcut core uses a registration/press boundary: replacing or
closing a generation retires its active recipients while preserving held-key
suppression until actual seat reconciliation. This is the kind of lifecycle
boundary that would have exposed the original teardown mismatch. Its initial 31 isolated
tests include bounded ownership, nonwrapping generations, honest layout-aware
conflicts, source-time milliseconds, current-layout/ASCII fallback resolution and
held release ownership. The later staged engine and actual Unix socket owner now
pass 51 combined tests, adding revalidation, stale-generation refusal and bounded
FD dispatch/backpressure. They are not yet integrated into the compositor or a portal.
The [lease ledger](../../.scratch/xdg-portals/global-shortcuts/LEASE-PLAN.md) and
[evidence](../../.scratch/xdg-portals/global-shortcuts/evidence.json) retain exact
pins, failed controls, staged artifacts and outstanding acceptance work.

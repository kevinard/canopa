# GlobalShortcuts compositor integration

2026-11-01. The private compositor module is now canonical under
vendor/source-overlays/niri-global-shortcuts. Public GlobalShortcuts v2 sessions
and Iced consent/configuration are now experimental under ADR0067;
[session-evidence.json](session-evidence.json) records scoped acceptance.
Normal advertisement remains pending. The full roadmap has eleven packaged and six experimental
interfaces; this private module does not change those counts.

Current inhibition checkpoint (ADR0068): recipe a85506a3… / Niri92294c79… permits
empty trusted leases while an editor inhibits shortcuts, retaining lock/UI
admission checks and suspending activation. This allows overlapping consent
dialogs while preserving reservations. All53 actual-source core tests and eleven
pilot public-frontend groups pass. The final shared build now passes 22 editor
groups, three native window-lifetime rounds and six shared desktop suites. The
original workspace retains the same ten Files failures with all other Rust/post
stages passing. Broader physical/sandbox and distribution acceptance remain. See
[inhibition-evidence.json](inhibition-evidence.json). Earlier recipe a750dae3… /
Niri9d8ccdd3… results below remain historical evidence. The production-unused
Registry::is_current helper mentioned below is now test-only.

## Current implementation

The narrow D-Bus interface opens one trusted-backend FD. The same Host owns its
nonblocking socket, logical definitions, generations and captured press cycles.
Its actual Niri input integration preserves normal/a11y precedence and observes
source edges even when Smithay absorbs a combined-seat non-transition. Physical
keyboards now have separate identities, while virtual and RemoteInput identities
remain distinct. Source withdrawal and actual seat-state reconciliation retire
only the correct recipients. See ADR0066 and the recipe README for protocol and
policy; the staged predecessor and original bugs remain in LEASE-PLAN.md.

The registration set changes atomically, using a generation observed by the
backend. Config/keymap/a11y changes rebuild the admitted subset before another
input dispatch. Previously approved logical choices remain separate from live
authority. Effective accessibility reservations have a semantic revision:
identical calls and watch-only ownership changes do not end an active action.
Bare modifier triggers match XKB's real post-press state. Backend-focus cleanup
retains suppression for keys still actually held.

## Source and build evidence

The validated input-capture recipe remains unchanged at
0c1f6cc49ef7e6ed186315c7e6d50cf215852304772979df5ec14cebcc084f22.
The new recipe reuses its complete Smithay patch and original official
Niri/Smithay archives and licences. The new complete Niri patch is relative to
the official archive, rather than stacked on a patched path.

target/portal-global-shortcuts-24/canonical-01 through -04.json reproduce both
complete old/new source trees from the archives, including only the two original
stable Smithay source bindings. The feature delta is thirteen Niri paths (five
existing modules and eight new files), with no Smithay delta. Source inventory
is 5,626 Niri plus 345 Smithay files. Stable source paths and the existing Niri
incremental dev cache/nightly/default feature selection are retained.

Build-01 fails on two adapter borrow-lifetime errors. Build-02 passes on recipe
2456901f… and binary ce8ecce7…, with compiler warnings later removed. Build-03
passes the semantic accessibility-reservation correction. Build-04 is the final candidate, with one unused Registry::is_current helper
warning. The actual-source core tests use that helper; production cleanup is
recorded for a subsequent source revision. Builds use one job, 3 GB growth cap and
8 GB free-space floor. No installed executable, live bus or portal routing changes.

The actual canonical core passes 53 tests, with denied compiler warnings, using
the same cached nightly xkbcommon/serde/serde_json dependencies. The source entry
points directly to the prepared canonical modules; there is no second Cargo graph.
core-tests-01.log and core-tests-02.log retain outcomes. The latter removes an
unused production inspection method and verifies permanent socket retirement
through real read/write operations instead.

## Native evidence and remaining checks

native-01 stopped at an invalid duplicate press in the RemoteInput probe. That
protocol rejects duplicate presses; the corrected fixture releases the existing
hold once. No production change was needed for this fixture failure.

native-02 passes nine groups on frozen ce8ecce7…: exact private FD/version and
compositor ownership, modifiers-first release, layout switching with a held
recipient, overlapping sources and EOF, rebind/generation cleanup, ordinary and
foreign conflicts, invalid/stale configuration, accessibility change/restoration,
malformed FD cleanup and finite admission recovery.

native-identical-red-03 uses the expanded regression on that same earlier
compositor. It reaches seven groups, then fails because submitting identical
empty a11y reservations ends the active Shift_L action. Source and binary pins
match and every owned process is terminal. This is the retained failing control
for the semantic revision correction.

The final native-09 probe passes thirteen groups on compositor
9d8ccdd3293b211180f29ccfe0762a3b498a7d33253443c460b06f0a2afeedaa,
recipe a750dae3c54819e1a55478f3a342552977b28afd86402dd44e7c6cd86f91269a.
The exact same script fails on earlier ce8ecce7… in native-identical-red-10 at
identical-reservation retirement. All pins match and owned processes are terminal.
The final two groups prove an actual binds-only config reload (keymap unchanged)
and a keymap replacement retiring an active source while the logical shortcut
lease survives and matches its new physical key.

Earlier slow-reader fixture attempts are retained: native-04 drained output while
commands were still processed, preventing backpressure; native-05 observed
hangup but treated the terminal connection reset as unexpected. The final probe
sends bounded valid commands, reads no output until POLLHUP/POLLRDHUP is observed,
then accepts only EOF or reset and proves another lease/input remains usable.
Its observation is hangup mask 8216 before read, 27,698 drained bytes and reset.
Native-07 passed the eleven groups before config/keymap coverage was added.

The same final compositor passes all related regressions: source-close-01 ten,
raw-keyboard-01 thirteen, symbol-keyboard-01 twelve and portal-keyboard-01 two
public Rust/Iced keyboard/EIS groups at 125% scale with actual monitor frames.
All three input workers are terminal and binary/source pins are unchanged.
Together with the thirteen shortcut groups this is fifty scoped native groups.

The required original-graph workspace-01 gate has completed: Files integration
has 596 passes, the same ten failures and 41 ignores. The cause of those failures
is not established. All other Rust targets pass, including Portal 179 library and
55 integration tests, and all four post-Rust stages pass. Guarded sources are
unchanged and net storage decreases by 139,603,968 bytes. It used root rustc 1.95,
the existing incremental target, one job, a 3 GB growth cap and 8 GB free floor.
See workspace-analysis-01.json in target/portal-global-shortcuts-24. The old tool
session 75902 is terminal; do not restart that check.

[Current machine-readable evidence](compositor-evidence.json) binds source,
recipe, core and native receipts. The exact source recipe is 5,971 files; compiled
harness core-tests-02 remains the current raw core executable. Obsolete evidence
is compressed with decompressed checksum verification, retaining all receipts.

Physical multi-device, broader layout/input-policy/lifetime, public Rust/Iced,
real sandbox and distribution acceptance remain open. Print still requires
implementation; GlobalShortcuts is now experimental. All native runs
reference one compositor per checksum; obsolete harnesses/binaries are archived
losslessly when no longer needed.

## Rust transport checkpoint

The in-process Rust endpoint now authenticates the original unique D-Bus owner,
version, compositor IPC credentials and returned Unix socket. Its sole lease
owner revokes the FD even when command handles survive. Framed reads, mutation
replies, admitted sets, generations and paired source-time events are bounded and
validated; malformed events, original-owner loss and stalled consumers close it.
Application action IDs remain distinct from private numeric binding IDs.

The default-feature package library harness passes fifteen model/transport tests
with host IPC access. The restricted run failed at the socket boundary; the same
compiled harness passes outside it. The guarded native test passes three groups
against the same canonical compositor: exact endpoint ownership and commit,
real shortcut press/release with source timestamps and no ordinary key leak, and
backend SIGKILL with F9 held followed by independent reacquisition. The old held
release stays suppressed and only a fresh press activates the replacement lease.
All owned processes are terminal and guarded inputs are unchanged.

This uses the existing root incremental cache and focused package selection;
there are no new dependencies or copied per-run application binaries. The Rust
native harness differs from the preceding full-workspace harness. Exact pins,
logs and scope are in [transport-evidence.json](transport-evidence.json).
The newer [public session checkpoint](session-evidence.json) implements saved
choices and Iced Bind/ConfigureShortcuts and passes eighteen public native groups.
Keyboard recording, editor inhibition and broader acceptance remain open.

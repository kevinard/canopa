# Legacy RemoteDesktop keyboard continuation

Status: compositor symbol owner, worker/parent transport and ordered public
keyboard Notify wiring verified, 2026-10-31. Eleven Cargo private-bus contracts
pass. Nine native public reply-barrier groups pass per appearance, including
original monitor streams after legacy-to-EIS transition. A pinned frontend
companion now fixes concurrent forwarding and passes fourteen native groups per
appearance and sanitizers. Its canonical recipe and outstanding release gates
are recorded below; the installed frontend remains unchanged.
The accepted production keyboard/media sessions remain unadvertised. The broader
eighteen-interface roadmap and its release/distribution gates remain active.

## Contract and source evidence

The [public RemoteDesktop contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html)
permits D-Bus Notify input or EIS. Notify must stop after EIS is established.
Implement caller/session/grant checks and an explicit input-mode transition in
the shared actor, rather than introducing another independent session registry.

[Mutter's native virtual keyboard](https://raw.githubusercontent.com/GNOME/mutter/main/src/backends/native/meta-virtual-input-device-native.c)
looks up symbols in the current group and adds level modifiers. It does not
provide arbitrary text insertion. [COSMIC's legacy adapter](https://raw.githubusercontent.com/pop-os/xdg-desktop-portal-cosmic/master/src/remote_desktop_ei.rs)
uses EI Text for keysyms, which Canopa's current keyboard-only EIS worker rejects.
Neither implementation establishes support in the current Canopa compositor.

The retained v1 Niri input lease accepts eight-byte evdev/state transitions
and emits modifier snapshots without a command sequence. Its keymap and source
ownership are authoritative in the compositor. A portal-side keysym resolver using that v1 stream
cannot infer that a received snapshot acknowledges its last Caps/group input.
Do not translate a keysym to a US keycode or silently treat it as a raw keycode.

## Current typed compositor evidence

The registered explicit niri-remote-input recipe now negotiates
OpenKeyboardSymbols through SymbolVersion1, preserving v1 OpenKeyboard unchanged.
Typed commands are16bytes (u32 sequence, kind, value, state); replies are24bytes
(sequence, status, authoritative modifiers). Sequence0 is unsolicited feedback;
positive commands start at1 without wrap. Status0 accepts;1 invalid logical
transition,2 unavailable symbol,3 conflicting base key,4 held-key limit. Refusal
leaves prior ownership intact; malformed frames or slow output readers retire.

The resolver borrows the actual keymap under Niri's input-thread XKB guard and
retains numeric chords only. It sees Caps/group changes synchronously with input,
shares8-lease admission with v1, expands a command to at most6transitions and keeps
64physical/logical keys and32commands per dispatch as finite limits.

Sixteen planner/wire tests pass0.41s. The incremental locked/offline nightly build
passes214.14s with all recorded inputs unchanged. Niri patch
 e1bd2127efba2a5503024ea295e162becfb635f2bda7b7c12bcf7b4ddb01ad99
and unchanged Smithay patch reproduce all15archive file entries; the registered
recipe also materializes canonically in6.14s. Vendor integrity, source-addition3
and source-binding4 regressions pass. No dependency/lock-graph change or install.

Frozen compositor
 e5e79252438ed2caf2d8fc0a9adeab45c543aaa0bd0792d7060feb38117d815f
passes12native symbol groups, all13v1 evdev groups, and6combined public-frontend/
media/EIS groups with the already frozen PortalD4/harness489. Summaries are
 target/remote-input-symbols-native-01/summary.json,
 target/remote-input-symbols-v1-regression-01/summary.json, and
 target/remote-input-symbols-media-dark-01/summary.json.
The typed fixture verifies French A/B modifier ownership, AltGr, immediate Caps,
live group switches, explicit logical refusal, independent v1-source overlap,
malformed-frame retirement, common admission, slow feedback and idle keymap loss.
Exact typed/media fixture sources are archived; compositor hashes are unchanged.
Build/source history is retained at
 target/niri-remote-input-source-01/keyboard-symbol-build.json and
 /tmp/canopa-legacy-keyboard-compositor-stage-01/.

The portal worker now has an optional legacy_keyboard bootstrap mode and a bounded
one-command bridge for this protocol. Default EIS bootstrap omits the new field
for compatibility with older deny-unknown-fields workers. The parent retains an
internal LegacyInput socket and the same reaped worker/control lease and8-slot
admission. Acknowledgement/domain errors are distinct from framing/lifetime loss.
Library check passes33.41s; initial bridge3 tests pass, then input-suite18pass/
1nativeignored1.27s. After adding source-EOF and invalid-input coverage, tests03
retain19pass/1fail/2ignored: a test-only try_read readiness assertion raced EOF.
Tests04 use a bounded async EOF read and pass20/2nativeignored0.90s. The real parent harness/driver pass all5groups with worker477fc400/harnessae400644
and compositorE5. Sources/binaries remain unchanged and all5workers are terminal.
All6existing EIS parent groups also pass on that build; their exact4fixture sources
are archived and all6workers are terminal. No Notify methods or
normal main/descriptor registration have been added.

## Verified prototype

`/tmp/canopa-legacy-keysym-stage-01/receipt.json` pins the prototype source/compiler,
test executable, build arguments, outcomes and retained red iterations. It uses
the existing root debug xkbcommon0.8 rlib with direct rustc; no Cargo dependency,
vendor source, supported target directory or portal behavior changed.

The final prototype passes ten tests in0.51s, including760 printable ASCII cases
across US, French, both groups of US/French, and Caps Lock on/off. Other cases cover
French uppercase and AltGr, shared modifiers and mixed raw-key ownership, delayed
feedback, unmappable symbols, duplicate/unpaired transitions, bounds, and symbols
that would reuse a still-held base key. The final source hash is in its receipt.

A genuine failed regression is retained: after real Shift feedback, a second
uppercase symbol borrowed the first symbol's modifier. Releasing the first then
released Shift too early. Independent chords and reference ownership fix it.
Two later red tests incorrectly assumed particular physical codes: Euro has a
direct key in the installed map, and XKB's internal level selector precedes RAlt.
The corrected AltGr test checks actual XKB symbols/state and exact owned release.

The planner bounds its source map to1MiB, logical and physical held keys to64,
evdev codes to1..0x2ff and selector searches to five named momentary modifiers and
finite mask subsets. It preserves locked modifiers and group, never toggles locks
to type, and refuses unavailable/conflicting symbols without changing prior held
ownership. This is model acceptance, not native input acceptance.

## Next concrete implementation

The compositor-owned borrowed-map planner and typed lease described above are
now adopted. Their map-ID retirement and source-aware cleanup remain required.
The following original continuation requirements remain until worker/session
integration and public native acceptance are complete.

The separately negotiated typed symbol/keycode lease is implemented. Keep
the accepted v1 evdev lease usable by existing EIS workers; never change its byte
format in place. The typed reader, finite per-dispatch budget, admission and held
state must cover chord expansion and invalid-transition retirement. A compositor
planner can resolve each symbol using actual current Caps/group state before
injecting its ordered chord, avoiding speculative portal-side feedback ordering.

The parent should retain the same worker control lease and shared8-worker
admission for legacy input. A private legacy worker can bridge bounded typed
commands to this lease without exposing its internal FD as an EIS descriptor.
The shared session actor must serialize Notify with ConnectToEIS, validate original
caller and approved keyboard scope, observe cancellation while starting, and
retire the whole session on transport loss. No normal main registration or
descriptor change belongs in this partial slice.

Required native proof includes real public NotifyKeycode/Keysym, US/French and
Caps/group changes, overlapping chord/raw/physical sources, mixed-mode refusal,
fresh consent and all existing stop/crash paths. Pointer/touch, supported compositor
distribution and remaining full-workspace/native reds are still separate gates.

Final scoped transport validation passes: library138pass4ignored5.97s and
integration30pass2ignored1.87s; Clippy104.28s, headless36.55s, formatting,
test-layout3groups, syntax and whitespace all pass. Receipt
 /tmp/canopa-legacy-keyboard-validation-01.json. The required full workspace
gate is next. An unadopted core Notify/actor draft at
 /tmp/canopa-legacy-notify-stage-01/ passes headless metadata11.01s only; its
mock-bus tests and ProductionControl integration still need work.

Required full workspace gate is now running as driver1183549/start28271509,
child1183556, tool session99657. Its receipt records1483inputs at
 /tmp/canopa-legacy-keyboard-workspace-01.json and output at
 /tmp/canopa-legacy-keyboard-workspace-01.log. Keep those root build inputs fixed
and continue Notify only in the unadopted draft until the existing gate is terminal.
The first wrapper attempt stopped before running the gate because a non-UTF8
symlink target required os.fsencode; byte-preserving link hashing and dangling-link
handling were corrected, with the failed wrapper retained separately. This is
not a product gate failure or a replacement for the still-pending gate outcome.

2026-10-31 typed transport full gate is terminal101/1547.65s. All1483recorded
inputs were unchanged. Files integration596pass10fail41ignored606.54s stops the
gate before Portal/later stages. Failure names exactly match the prior ten; no
added/removed names. Comparison /tmp/canopa-legacy-keyboard-workspace-comparison-01.json.
Dedicated Portal138/30, Clippy and headless passes remain scoped evidence.
The historical matrix and full gate remain red. Notify/production draft stays
unadopted while its mock-bus contracts are completed; see staging checkpoint.


## Notify adoption and focused contracts, 2026-10-31

After the previous full workspace gate became terminal, four guarded source
files were adopted from /tmp/canopa-legacy-notify-stage-01. The actual Cargo
library check passes25.70s, and the eight RemoteDesktop integration contracts
pass0.99s at /tmp/canopa-legacy-notify-contracts-01.log. They exercise original
caller/session/grant checks, recoverable domain refusal, the one-way EIS guard,
and pending acknowledgement cancellation on session/caller/source/backend loss.
A blocked input command cannot expand the actor's bounded queue; overflow revokes
the common session so a discarded release cannot leave held keys active. That
case was reproduced red against the earlier draft before the retirement fix.

Production starts one authenticated legacy worker lazily on the first accepted
Notify. ConnectToEIS stops and reaps that worker before creating EIS, deliberately
removing its lifetime observer so this transition does not close consent. The
same capture and ongoing controls remain owned by the shared session. These are
implementation details verified by focused contracts and compilation, not yet
public frontend/native acceptance. The new nine-case native fixture includes an
eight-event shortcut burst, real monitor frames and legacy-to-EIS continuity.
The main backend and shipped descriptor still do not register RemoteDesktop.


### Shortcut burst diagnosis

The first new native run passes input-only and view-only, then fails the ordinary
eight-event Ctrl/Alt/Shift/F13 burst. The isolated replay also fails, with empty
public replies, partial receiver events, Session.Closed and retired controls:
 /tmp/canopa-remote-desktop-notify-dark-01/summary.json and
 /tmp/canopa-remote-desktop-notify-burst-red-01/summary.json.
All owned fixture executables are confirmed terminal after both runs. These are
retained failures, not native acceptance. The new delayed-ack private-bus
regression reproduces explicit LimitsExceeded in0.27s at
 /tmp/canopa-legacy-notify-burst-contract-red-02.log.
The cause is the original four-command shared queue, appropriate for startup
commands but too small for asynchronously forwarded input. The fix permits64
pending commands per session (still16sessions) and retains session retirement on
overflow. A65-call blocked-input regression checks that retirement. Post-fix
Cargo and native validation are pending; no pointer/touch or distribution claim.


### Ordered backend admission and frontend forwarding, 2026-10-31

After the capacity fix, native burst-green03 delivers all events but swaps two
presses while the session and controls remain active. A32-event trace05 then
localizes one native ordering failure to the installed xdg-desktop-portal1.22.1-2:
public messages match the issued order; backend forwards do not; the actual
receiver matches those forwards after refusing duplicate/unpaired transitions.
 /tmp/canopa-legacy-notify-frontend-order-01.json pins the frontend executable and
comparison; trace05 archives its exact fixture sources. Trace04 passes once;
that does not disprove the retained races. The incomplete burst-green02 copy
failed filesystem quota before any native case; it is an environment failure.

Independently, the new direct four-thread regression fails in round0/0.36s before
an ordering fix, at /tmp/canopa-legacy-notify-order-contract-red-01.log. The new
RemoteDesktop Interface adapter admits keyboard commands in receive order before
spawning acknowledgement tasks, preserving asynchronous typed setup handlers.
Its library check passes33.21s. Eleven actual contracts pass1.55s at
 /tmp/canopa-legacy-notify-order-contracts-03.log, including1,024 ordered direct
transitions and another session's keyboard while consent is pending. An earlier
contracts02 compile failed only a new test's shadowed helper name; corrected in03.
Final library/full integration/Clippy/headless/native rebuild is running at
 /tmp/canopa-legacy-notify-order-validation-03.json.

The full public reply-barrier dark01 attempt returned Start2 in input-only;
 target/remote-desktop-notify-reply-barriers-dark-01/summary.json retains that
separate unresolved failure. The selected Iced consent screenshot was reviewed:
keyboard is checked and the action enabled. No cause is assigned yet. No native
acceptance or normal RemoteDesktop registration is claimed. Concurrent public
forwarding remains a frontend compatibility gate even after backend FIFO is fixed.


## Current ordered Notify acceptance, 2026-10-31

The final dispatcher passes library138/4ignored5.42s and integration36/2ignored2.88s,
including11RemoteDesktop contracts and1,024 ordered transitions across four threads.
Clippy all-targets with warnings denied passes88.64s; headless all-target check
passes49.39s; normal incremental default binary build passes4.09s. No new vendor,
profile, dependency graph or routing change accompanies this Portal-only slice.

Frozen ordered snapshot03 passes the isolated input-only native case, then all9
public reply-barrier groups in dark100 and light125 (58.10s and56.66s). Each full
run consumes8real monitor streams and records9terminal input workers. Legacy-to-EIS
releases the old held chord/worker, transfers the EI FD and keeps the original
approved stream usable. View-only denial, common Stop, explicit session close,
worker/caller/backend loss and Notify-after-EIS denial pass. Native screenshots
were reviewed in both appearances, including checked input-only consent and
unchecked view-only consent with one selected source. Exact7fixture sources per
run and99Portal/build inputs are archived unchanged; frozen binary hashes remain
unchanged. No live routing or RemoteDesktop main/descriptor registration.

Evidence: target/remote-desktop-notify-acceptance-03/receipt.json;
 target/native-test-snapshots/legacy-notify-ordered-bin-03/receipt.json;
 target/remote-desktop-notify-ordered-{dark,light}-03/summary.json.
The postbuild copies remove debug sections to fit storage; original Cargo debug
outputs/profile remain, and matching ELF build IDs/executable segment hashes are
recorded. Exact immutable test compositor duplicates are stored by reference to
the unchanged frozen compositor after each terminal run. Original tmp evidence
paths remain aliases to the stored successful artifacts.

This scope uses public reply barriers. The installed frontend1.22.1-2 concurrent
forwarding race remains independently reproduced and open, with persistent trace
and comparison at target/remote-desktop-notify-acceptance-03/frontend-ordering.json.
The older Start2 failure remains unexplained and retained; the current isolated
and full cases pass, without attributing its cause. The broader workspace and
historical full matrix remain red; focused Portal success does not clear them.
Pointer/touch, physical input, compatible distribution and the remaining interfaces
on the eighteen-interface roadmap remain active work.


## Canonical frontend companion acceptance, 2026-10-31

ADR0047 and the registered xdg-desktop-portal-input-order recipe address the
upstream worker-scheduling race. An exact official 1.22.1 build fails the same
concurrent burst; instrumentation shows reordering at worker/handler entry, while
proxy sends preserve handler order. Only three frontend files change. Cached
caller ownership and approved input are checked before a bounded per-session
queue; original handlers retain grant/option/EIS checks. Setup stays asynchronous.
The 64-pending/4-KiB/no-FD limits retire the owned session on overflow.

Canonical archive replay verifies 499 inputs and all three patched hashes.
Same-cache debug build/reuse succeeds; tampered/extra source, a symlinked root
and a nonempty generated lock are rejected. Frozen frontend b77492eb passes all
14 native groups in dark100 (86.95s, concurrent32events) and light125 (87.70s,
NoReplyExpected32events). Each has12real monitor consumers,14terminal workers,
unchanged7fixture sources and4binary pins. Explicit pre-consent denial and
rejected-FD closure pass. The separately frozen candidate also passes14groups
under ASan/UBSan with no memory-error report; global-cache leak scanning is
excluded. Original ordering/body/FD failing controls and interrupted-build logs
are retained, without assigning unrelated earlier failures a cause.

Persistent receipt: target/xdg-portal-input-order-acceptance-01/receipt.json.
The required post-adoption workspace gate finishes101in1667.42s at
 /tmp/canopa-frontend-order-workspace-01.json. Tooling/fmt/Clippy/all-target
compilation pass;2024inputs remain unchanged. Files integration finishes
596pass/10fail/41ignored648.07s, with exactly the same ten failure names and no
added/removed names. Remaining gate stages are tracked separately. Focused native
success does not clear this red gate or the historical107pass/81fail matrix. The installed frontend and live
routing remain unchanged. Pointer/touch, physical-input, compatible distribution
and the remaining eighteen-interface roadmap stay active; no RemoteDesktop
main registration or advertisement is added.


Canonical frontend Settings activation/read/reload also passes3.00s. FileChooser
passes12native groups per appearance (dark39.37s/light12541.66s) on the
portable root fixture after a diagnosed test-device lifetime correction. Both
original and candidate frontends fail the old one-shot Shift fixture identically;
a separate native probe observes mask0after return,1while the device lives and
0after retirement. The fixture now keeps that device alive across selection and
waits for its Wayland roundtrip. No application behavior changes. Root native
fixture sources and runtime binary pins remain unchanged during both corrected
runs. The full workspace gate above preceded this tooling-only correction.
Original failures and diagnostic/final fixture versions remain archived.


### Remaining shared checks and pointer continuation

The unreached stages finish with all remaining Rust tests passing1723.24s,
Settings activation1.10s, CLI build11.27s and vendor checks9.44s. Configuration
initially fails because the normal canopa-files executable is missing; the failed
control is retained. Its ordinary incremental debug build passes216.54s and the
configuration suite passes32.80s. The2024recorded source inputs remain unchanged.
This does not clear the required full gate's same ten Files integration failures.
Persistent continuation and recovery receipts are under
 target/xdg-portal-input-order-acceptance-01/.

Pointer continuation remains unadopted. Frozen E5 reproduces two virtual pointers
releasing each other's button state in8.13s, while its single-source control passes.
Five ownership tests and seven cycle/origin/routing model tests pass, each including
576four-source orders. A two-file getter/trace probe builds in345.28s in the same
nightly1.97 incremental dev cache. Its21.34s native diagnosis confirms teardown
releases already have a dead Wayland resource before Niri binding dispatch, while
ordinary releases have a live resource. The ownership contract remains red.
All owned probe processes are terminal; accepted E5 and source/runtime pins stay
unchanged. The temporary trace is removed afterward; only a read-only getter
remains in the unadopted prepared tree. Evidence and concrete binding/grab/mapping
continuation notes are at target/remote-input-pointer-investigation-01/.
This does not establish pointer/touch or physical-input support.

Completed inactive native snapshots were preserved on disk after process checks
and per-file hashes:11directories/1770596661bytes, with original/tmp paths retained
as aliases. The relocation receipt is
 target/native-test-snapshots/tmp-preserved-01/relocation.json. No active executable,
Cargo cache or root debug output was deleted. The full eighteen-interface goal
and distribution/full-matrix acceptance remain active; no live portal routing
or RemoteDesktop registration changed.

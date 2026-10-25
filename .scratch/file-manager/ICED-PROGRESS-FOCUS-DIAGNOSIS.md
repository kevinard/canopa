# Inspector focus return on Wayland

The full progress workflow exposes a real gap after otherwise successful work:
partial copy, cancellation, correct result sections, actual prefix Undo,25 new
copies, and an inspector still bound to the original report. Closing that
inspector leaves the newer Browser focused, so Alt+D cannot reopen Operations
inspection. The first paging probe51396 lacked a focus readiness check and is not
paging evidence. Controlled probe79611 waits for inspector focus before Escape,
then fails specifically waiting for Operations focus. No paging failure claimed.

Minimized feedback loop: native case files-iced-focus-dark, isolated private
HOME/Niri/bus. Reuses the real paused transfer but omits Undo and25-file copy.
Open Operations inspector, focus a new Browser, refocus inspector explicitly,
Escape, then require its parent Operations to regain focus within3 seconds.
Min-red18062 terminated1 at the same exact parent-focus assertion, confirming
the shorter deterministic loop. Wire-red97215 also terminated1 with protocol
tracing enabled only for that private command. No compiler/native remains live.

Ranked falsifiable hypotheses were shown before the minimized probe:

1. GainFocus is unsupported on the locked Wayland backend. Prediction: the call
   reaches an empty implementation; supported xdg activation restores focus.
2. Close/gain-focus ordering retires the input source before activation.
   Prediction: requesting focus while the inspector still exists changes behavior.
3. The transient parent is wrong. Prediction: a targeted Wayland set_parent trace
   names a different surface, rather than Operations.

Read-only source evidence: vendor/iced_winit/src/lib.rs GainFocus calls
window.raw.focus_window(); vendor/winit/src/platform_impl/linux/wayland/window/
mod.rs has an empty focus_window(). The Wayland backend separately exposes
request_activation_token(), but it currently sets only source surface, no input
serial, and Iced does not consume it for GainFocus. This points to hypothesis1;
source retirement and serial authority must still be investigated before fixing.
No debug instrumentation or vendor edits have been added.

Read docs/vendor-maintenance.md before shared changes (already read). Keep the
single root forks, upstream attribution, provenance and reviewable patches in
sync. Prefer a supported shared Wayland activation/focus mechanism over a Files
Niri-command workaround. Any focus request must retain correct user-input/source
context, have bounded pending lifetime, and release retired windows. Do not
steal focus from background operation updates. Full original repro and both themes
must pass after the minimized loop; separate other shared-stack/workspace gates.

Still pending: section pager, keyboard scrolling of long facts, Operations parent
closure without answering a pending question, explicit job switching, both-theme
full progress acceptance, controller/lint checks and independent review. The full
PRD goal remains active.

Wire probe97215 maps Operations to xdg_toplevel#58 and inspector to#94; #94
explicitly sets parent#58. xdg_activation_v1 is advertised and bound as#14, but
there is no get_activation_token or activate request. This confirms hypothesis1
and rules out the wrong-parent hypothesis3. Merely reordering an empty function
cannot request activation; ordering may still be required to preserve context
once supported activation exists. No source debug logs or vendor edits exist.
The unrelated25-file copy/Undo preconditions have already passed on the full loop.

Primary protocol sources: the exact installed wayland-protocols0.32.13 XML under
CARGO_HOME registry protocols/staging/xdg-activation/xdg-activation-v1.xml and
upstream https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/staging/xdg-activation/xdg-activation-v1.xml.
Online upstream access was denied by its bot protection; use the locked local
primary XML and native wire evidence. The source surface requests a token, with
seat/input serial when available, before another target surface is activated.
Compositor focus policy still applies; do not implement a Niri CLI workaround.

Current lifecycle source findings for the next implementation step:

- WinitState has a calloop LoopHandle and maps WindowId to Arc<Mutex<WindowState>>
  and WindowRequests. WindowState owns a clone of the SCTK window; has_focus()
  exposes actual seat focus. WindowRequests.closed is set immediately in native
  Window::drop. These provide lifetime/retirement guards.
- Native Window currently has xdg_activation, queue_handle and WindowRequests;
  its focus_window() is empty. Keyboard Enter and pressed Key currently discard
  the protocol serial. Pointer handling retains button serial per pointer.
- Existing xdg activation token data handles Attention(surface, weak flag) and
  Obtain(window ID, async serial), but not a user-input-bound focus request.
  A shared Wayland implementation must distinguish source surface/seat/serial
  from the target, reject retired targets and bound outstanding token/context
  ownership. Prefer encapsulating this in the native Winit backend so Iced's
  existing GainFocus API works, rather than teaching the file app Niri IPC.
- Inspector close currently batches native close before GainFocus. Once real
  activation exists, request/capture user input context while source is alive;
  source retirement ordering is a contributing issue to verify, not a proven
  standalone cause. No code for this fix has been written yet.

All native/compiler handles are terminal. Current application binary hash is
005e4f2d975f67b00faedf5a0b04ed3435cd854e8bd4fb1a3e21cdef919b21f9. Its production inputs match progress source02; scripts subsequently added
focus-only minimization and readiness checks. No vendor source has changed from
the preserved clipboard baseline. No instrumentation remains enabled globally;
WAYLAND_DEBUG was scoped to the one wire command only.

First implementation (root ADR0021) is now building in session10595,
iced-focus-build-01.log. Do not edit source or start another compiler/native
fixture until that exact handle is terminal. No green focus result yet.

The backend now records weak input context per connection for keyboard Enter/key,
pointer press and touch. Capture checks age <=2s, live source requests and actual
focus from the same seat. Native focus queues a source-role lease before close;
the backend submits it before retiring windows. Token/source/timer ownership is
coalesced per target, capped64 globally, expires1s, cancels when target retires and
ignores obsolete replies. The requesting and activated surfaces stay distinct.
Inspector closes by requesting focus before closing its own native window.

The exact failing005e4f2d binary was gzip-preserved and streamed back to verify its
hash (preservation56642 terminated0). See iced-focus-before-preservation.json.
Only ten intended Winit source paths changed, including new types/focus.rs.
The exact cached published archive hash and complete upstream inventory were
verified, then the reviewable patch and provenance were regenerated together.
python3 scripts/check-vendor.py passes. See refresh-iced-focus-provenance.py and
iced-focus-provenance-refresh-01.json. No versions/licenses/dependencies changed.
Source01 binds523 inputs and23 paths against clipboard source04.

Next: poll10595, fix any compiler errors only after terminal, then run the unchanged
files-iced-focus-dark minimized regression. Once green, rerun the original full
transfer scenario to reach a genuine paging red; do not force focus from the
fixture or claim the pager exists. Review shared focus eligibility/lifetimes and
run appropriate native/shared/controller checks before final scoped acceptance.

Compiler10595 terminated0. Native minimized green90899 is live in iced-focus-min-green-01; no source edits or other compiler/native until the exact handle is terminal. Production source remains iced-focus-source-01 (523 inputs). No focus green claimed yet.

Minimized native90899 terminated0: first shared-focus implementation passes. Before/after captures were inspected: inspector focused while a newer Home Browser exists, then inspector gone and Operations focused. No forced post-close focus in the fixture. Native original/light01 is now live, sequential light focus and full original dark transfers; no source edits or another compiler/native until its exact handle is terminal. Full scenario is expected to expose the still-missing pager, not be claimed as complete.

Final focused outcome: build64332 and native33784 terminate0; normal return and overdue-reply rejection pass both themes (four cases). Native16145 is the genuine deadline red: real done reply delayed1.2seconds, then old code activated the parent. The correction shares Timer::from_deadline and pending.at, rejects overdue replies directly, and retains cleanup. Both review axes accept it. Source02 has524 inputs, binary04a8478255e551ec64d7dc1d20fb5f39a27b15db5a7df9d3fe3445ac3318ee52; exact binary/source archives are recorded in iced-focus-preservation-02.json. No production diagnostic instrumentation remains. The later full original transfer workflow passes both themes on final paging source06 (native47958). Broad shared-stack and PRD gates remain open.

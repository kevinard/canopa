# Print authentication private reply lifetime — sealed

Previous goal turn PROGRESS: phase64 prompt/channel/file/admission evidence. This
turn PROGRESS: reproduced and fixed a postapproval deadline/cancellation gap.
ALL execution handles are terminal; no build, native run, question, approval,
agent, commit or PR is pending. The full18 goal stays active:11packaged,
sixexperimental and Printopt-in. Print-first/automaticQt-deferred remains in force.

## Reproduction and fix

Read [ADR0102](../../docs/adr/0102-print-authentication-reply-lifetime.md),
[issue09](issues/09-print-authentication-reply-lifetime.md) and
[sealed evidence](print/authentication-reply-lifetime-evidence.json).

The ordinary approval/helper-exit exchange had a90s timeout, followed by unbounded
private length/body/EOF reads outside that direct caller lifetime. Controlled
EOF-prefix baseline01 completes immediately. With an extra writer retaining the
same partial prefix after helper exit, baseline retained-prefix01 has no reply
after95.0119s. The helper is already terminal; releasing the writer immediately
completes error2. All baseline processes terminal/private data removed. Its failed
receipt is retained, not rewritten. Investigation01 records ranked hypotheses.

prompt.rs now wraps ordinary approval, helper exit and private length/body/EOF in
one90s timeout and withdrawal/caller-loss select. No timer reset after approval.
Frame limits, secret owner, private encoding, consent/cache policy and UI are
unchanged. Only prompt.rs and new prompt/tests.rs changed Rust. No vendor, manifest,
lockfile or dependency edit. Two regression tests cover six partial/unclosed
reply withdrawal/caller-loss variants and valid complete reply decoding. Socket
queue drain proves the parent passed ordinary approval and began its private read.

The same retained-prefix candidate02 passes at90.1966s while the controlled writer
is still live; EOF candidate02 still refuses immediately. These cases run the
real Rust controller/Production/Broker/Dialog/Client and snapshot worker with
controlled approval/worker/password stand-ins, no actual Iced/scheduler/document
submission or physical claim. Ordinary Iced is not claimed to fork or fail this way.

## Validation and native continuation

Strict workspace Clippy/format PASS (Clippy96s). Original workspace/default/
all-target/no-run compilation PASS477.82s, root incremental/debug/onejob/offline,
unchanged feature graph. Prompt2, allPortal437/20ignored, integration55/2ignored and
PPD1 PASS. Canonical current guard: target/portal-print-65/worker-01.json,1306inputs.
Current binary ef722d77bfc733b69ba35e1eee5032350625a9485fbf6a5b8ee556fdddf9ee33;
selected CUPS cliente0d80ef1ff4154878f72d00a1e45c65117c6420991eda568b8bb0b74422cb457 unchanged.

Six candidate native invocations pass: two controlled parent cases, actual public
Iced file transitions/prepared4pagePDF output in dark/light125%, and actual Iced
wrong-once TLS/PAM protected recovery in dark/light125%. Recovery uses another
approved actor with immutable owner, preserves the unrelated job and never sends
another document. File selection withdraws pending sign-in, clears an approved
login (fresh consent on return), then delivers files without printer requests.
Four actual Iced invocations,94 unique observed candidate PIDs terminal/private
cleanup. The private CUPS/PAM namespace overlays do not modify host auth files.

All sessions consumed:46086 EOFbaseline0,15840 baseline1,49507fmt0,64098Clippy0,
42512oldharnessarchive0,59817build0,77449read0,47455tests0,58239candidate90s0,
52712candidateEOF0,48507filedark0,58828recoverydark0,18607filelight0,
97809recoverylight0,84603binaryarchive0,60913seal0. No live child remains.

## Evidence, storage and continuation

Current scoped receipts: target65 acceptance01, validation-clippy/build/tests01,
worker01, investigation01, candidate01.patch and two source snapshots/hashes.
Source-only replay in target65/replay and /tmp/canopa-print-65. The old169,958,944byte
harness is losslessly compressed once as08455f8e... (29,212,256bytes). Current normal
binary is compressed once byef722d77... (38,838,620bytes); no per-evidence executable
copies. No cache purge, release profile or fresh target directory. Net at seal:
78,016,512 fewer bytes used,6,196,662,272free, within5GB reserve/900MB growth cap.

Full workspace execution was not repeated for the local Portal fix; earlier exact
ten Files integration failures remain open. No broad all-green/native-device/
Flatpak/package/release claim. NEXT [authentication-connection-next.md](print/authentication-connection-next.md):
outer60/300s supervision, credential/challenge/connection interoperability,
repeated authenticated FD retirement and optional runtime packaging/NOTICE,
then existing device/Flatpak/release/sixexperimental gates. The postapproval seam
is resolved; do not redo completed tests merely due another goal turn.

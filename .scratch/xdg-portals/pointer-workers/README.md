# Private capture-bound pointer workers

Status: Rust pointer worker and production capture handoff implemented with
scoped native acceptance, 2026-10-31. Public EI/Notify pointer handling, Iced
pointer approval and canonical compositor adoption remain outstanding.

The recording worker returns one complete private binding group after producers
start: original unique compositor owner, session/stream paths, selected live
source IDs, mapping IDs and logical sizes, associated with PipeWire nodes.
The production Capture facade retains this group separately from public stream
results. A missing mapping keeps legacy view-only capture available and cannot
authorize absolute input.

The host starts a separately owned pointer worker with that group. It validates
owner/PID, current source metadata, the returned Unix FD's peer and the atomically
pinned OpenMappedPointer descriptors. Host commands select approved PipeWire
nodes; the worker alone converts those selectors to compositor stream IDs.
An explicitly unbound relative lease has no absolute region. This internal
command FD is never presented to an application as an EI FD.

Both sides enforce finite framing, sequence and acknowledgement rules. The worker
checks button pairing, the 64-button limit, valid regions, coordinates and scroll
bounds before injection. Abandoning an in-flight host command revokes its worker
instead of reusing an ambiguous acknowledgement. Pointer and keyboard workers
share admission bounds while retaining separate typed transports.

Resize releases the old compositor source. The worker renews only after confirmed
EOF, the same live identities and a verified size change. It never replays the
interrupted command or restores old held buttons. Shutdown with unread command
bytes can produce ECONNRESET before EOF; the worker requires a subsequent bounded
EOF read. A reset alone, unexplained same-size EOF, source loss or failed identity
revalidation cannot authorize renewal. Recording ownership survives pointer Stop.

| Final scoped check | Result |
| --- | --- |
| Portal library | 151 passed, 5 ignored |
| Complete portal integration executable | 37 passed, 2 ignored |
| Formatting and all-target Clippy, warnings denied | Passed |
| Actual capture handoff, pointer delivery, video matching and retirement | 11 groups in each of two runs |
| In-flight resize stress | 192 transitions in each run |
| Identity-capable compositor, dark/125%, public video and Iced controls | 17 groups |
| Existing canonical compositor, light/100%, legacy view-only capture | 17 groups |
| Identity-capable compositor, light/125%, combined keyboard EIS/media | 6 groups |

The 62 native groups include actual client coordinates and four captured color
regions at 125%, paired buttons, wheel and axis stop, invalid/unrequested region
rejection, held-button release on idle resize and scale changes, repeated motion
during resize, pointer Stop/reopen while capture stays alive, backend SIGKILL and
independent worker retirement, and selected source destruction. Producer cleanup
uses PipeWire node plus serial identity so node-number reuse cannot fake failure
or success. Public capture and keyboard regression suites retain their normal
frontend/Iced consent and lifetime paths; they do not exercise a public pointer
grant.

Normal incremental debug/default-feature checks take 330.77 seconds, including a
direct regression run. Native suites take 361.44 seconds, including
startup and artifact preparation. All 1,308 recorded workspace/vendor inputs,
fixture sources and frozen executables stay unchanged. All owned jobs and native
process groups terminate. Evidence is at target/portal-pointer-workers-07/;
[evidence.json](evidence.json) identifies exact hashes and receipts.
[portal.patch](portal.patch) is the 16-file delta against the preserved starting
state. Its existing-file baselines and successful isolated replay are retained.
Native proof sources, receipts, logs and images are preserved in native-proof/.

Earlier failures remain evidence. One test expected a revocation channel value
instead of closure; a native fixture tried to serialize a public Stream without
Serialize. One integration run passes 36 tests and times out waiting for worker
startup; its cause is unresolved. The direct case on the same executable and the
unchanged full rerun pass. Two native runs pass seven groups then lose the pointer
worker during repeated resize. Early EOF handling alone does not fix the second
run's socket reset. Targeted diagnostic runs pass, including a 192-transition
control; their timing does not isolate the intermittent native failure. A real
Unix-socket test deterministically reproduces reset after shutdown with unread
command bytes, then passes with subsequent-EOF confirmation. Diagnostic tags are
absent from final code, and both final 192-transition native runs pass.

The mapped compositor remains the unadopted c56e0779… draft. Existing EI and
legacy keyboard transports remain keyboard-only. Fresh unchecked Iced pointer
permission, receive-ordered legacy Notify handling, EI devices/regions and their
geometry-change replacement, remaining window behavior and physical-input
acceptance still need implementation. Earlier full workspace and native matrix
failures remain open. Distribution, remaining portal interfaces and the full
18-interface goal are unchanged. See
[ADR0049](../../../docs/adr/0049-capture-bound-pointer-workers.md) and
[the mapping ledger](../POINTER-MAPPING.md).

The subsequent [pointer adapter foundation](../pointer-adapters/README.md) adds
public mapping metadata and typed receive-ordered Notify handling. Production
EI/Notify pointer transport and explicit Iced permission remain incomplete;
runtime pointer grants remain disabled.

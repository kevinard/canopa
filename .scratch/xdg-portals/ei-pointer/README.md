# Session-owned EI pointer devices

Status: private Rust transport implemented with scoped acceptance, 2026-10-31.
The broader portal goal remains active. Production pointer approval, public
pointer sessions and normal RemoteDesktop registration remain disabled.

One anonymous sender connection now supports independent keyboard and pointer
devices. Pointer-only input opens no keyboard. Captured streams become separate,
nonoverlapping logical regions carrying their exact mapping IDs, dimensions and
known physical scale. Absolute input resolves to the selected source's local
coordinates through the existing authenticated pointer actor. Relative motion,
paired buttons, smooth/discrete scrolling and selected-axis stop/cancel use that
same actor. Raw pending events and frame sends stay bounded per device.

Confirmed compositor retirement discards pending old input and releases held
state before a verified size/scale change can create a fresh EI pointer device.
The keyboard remains active across pointer replacement. Old object IDs cannot
route to a replacement, and interrupted commands are never replayed. Client
device release, request-device and stop/start emulation retain explicit source
cleanup. Both sources retire on client, caller, backend or capture loss.

| Final scoped check | Result |
| --- | --- |
| Normal incremental/default-feature portal build | Passed |
| Complete portal library | 154 passed, 5 ignored |
| Complete portal integration | 43 passed, 2 ignored |
| Portal Clippy and formatting | Passed |
| Real libei pointer/media acceptance | 18 groups, repeated successfully: 36 group executions |
| Existing real libei keyboard regressions | 12 groups |
| Actual captured frames | 10 frames, matching dimensions and four color regions |
| Changed-file patch replay | All 12 files matched |
| Owned native jobs and stage-specific workers | Terminal; none left running |

The pointer fixture verifies paired monitor/window regions at 125%, actual
absolute and fractional relative client coordinates, smooth/discrete scrolling,
zero deltas, axis stop/cancel, two size and two scale changes, held-button cleanup
and keyboard continuity, emulation restart, explicit device release/request,
parent cancellation, relative-only input, client EOF/death, worker death,
caller/backend loss, a rejected unframed flood and capture loss. It stays on a
private bus and nested Niri; it does not alter the installed session.

The first final pass uses the unchanged previous-stage capture fixture harness;
the second uses the freshly compiled/frozen current harness. Both use portal
47ca439354beac2168be023ce30131ae5cd3929e5af76887d23eb2278ae077da,
the same five fixture/helper source pins and prepared scale-capable compositor
84acceed40ace2be1362ecfd32239b0bc58b909f070801144a3e0962952168c9.
The final build/checks retain all 1,320 source inputs. The full check sequence takes
280.15 seconds; pointer passes take 13.01 and 21.65 seconds.

The retained baseline reproduces a real exit-classification bug: client EOF can
race queued keyboard modifier output. Held input is released, but the worker
returns failure with Broken pipe. The identical final fixture fails on the
pre-fix portal after 11 passing groups and passes all 18 with the fix. Only errors
on the client EI transport are classified as ordinary peer closure; compositor
cleanup acknowledgements remain required. Malformed requests, backpressure and
compositor errors remain failures.

Earlier execution evidence is retained: one native fixture supplied the capture
owner instead of the input owner and was rejected before readiness; two harness
runs treated intentionally successful child exits as unexpected failures. The
first default-sandbox library run passed 109 tests and failed 45 with denied
private socket/fixture operations; the same compiled executable passed all 154
when run with fixture access. An intermediate build caught an Errno-to-io-error
conversion missing from the peer-closure wrapper. The final build and checks
pass. These failures are not omitted or counted as acceptance.

[evidence.json](evidence.json) pins the binaries, checks and scope.
[portal.patch](portal.patch) contains the application/fixture delta, with preserved
baselines, current sources and [successful replay](replay.log). The keyboard
fixture's one-line baseline is explicitly reconstructed from its observed grant
change from 2 to 4; portal baselines were copied before edits. Native helper
sources, red/green receipts and final frame PNGs are retained here. Full execution
logs live at target/portal-ei-pointer-10/.

Next: connect production legacy pointer commands and retire them before EI
handoff, add fresh unchecked Iced pointer permission with runtime capability
checks, then validate public combined sessions and the broader input matrix.
Companion archive adoption/distribution and earlier shared-workspace/native reds
remain separate work. This slice changes no vendor source, installed service,
portal routing or advertised capabilities. See [ADR0052](../../../docs/adr/0052-session-owned-ei-pointer-devices.md).

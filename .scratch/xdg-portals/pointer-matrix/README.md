Follow-up: the historical backend-death red below is corrected by the canonical
frontend delta in [the session-owner ledger](../frontend-session/README.md). Its
original evidence remains unchanged. Broader shared validation remains open.

# Availability notifications and public mixed-source input

Status: availability fix verified with scoped passing checks; frontend session
closure after backend death remains a genuine red, 2026-10-31. The broader goal
remains active and RemoteDesktop stays unadvertised/unregistered normally.

The backend previously recomputed AvailableDeviceTypes on reads, but the standard
frontend cached its initial value. The identical headless private fixture shows a
backend transition from 3 to 1 while the real frontend remains at 3. A retained
registration guard now watches input owner/property notifications and publishes
authoritatively re-read masks. Owner changes invalidate the old value before
validating any replacement. Capability reads check the well-known owner before
and after querying the pinned unique owner. The watcher uses a bounded stream,
does no idle polling and opens no input lease. Stopping its guard aborts it.

The mixed-source fixture selects both monitor and window with explicit Iced
keyboard/pointer approval. It verifies distinct mapping IDs, synthetic EI region
offsets, actual monitor/window input coordinates, and public restricted video.
Two size and two scale changes replace the pointer device after held-button
release, preserve held keyboard input, and retain both source identities. Both
UI-stop and client death release held inputs and retire both producers/controls.

| Check | Result |
| --- | --- |
| Portal incremental/default-feature build | Passed using existing root target |
| Complete portal library | 161 passed, 5 ignored |
| Complete portal integration | 43 passed, 2 ignored |
| Portal Clippy and formatting | Passed |
| Actual frontend capability notifications | 6 groups passed; same-fixture pre-fix red retained |
| Final mixed-source UI-stop and client-loss runs | 9 groups each; 18 executions, 22 video frames |
| Earlier mixed-source explicit-close run | 9 groups, 11 video frames |
| Existing pointer consent/handoff regressions | 17 groups per appearance; 34 executions, 14 frames |
| Backend SIGKILL matrix | Failed after 8 passing groups: no public Session.Closed |
| Delta replay | All 8 files match copied baselines |
| Owned native processes | Terminal; no stage-specific workers remain |

The final Rust sequence takes 37.45 seconds with all 1,322 inputs unchanged. The
UI-stop mixed-source body takes 25.54 seconds; client-loss takes 23.96 seconds.
The default dark/light regression bodies take 56.19/55.39 seconds. The exact final
availability runner passes in 4.53 seconds, following a 13.62-second successful
check with the new harness. The pre-fix availability reproduction fails after its
initial group in 5.71 seconds. It uses the same fixture source as the final pass.

Portal 4a95f8549935cead0b370ec1e583fe9aa772de746b9ed2ebc941072ea4eb344b
and harness 1f55f71e816db7e6fcb305e55d798fe89176e2fac3f13dba4a66d57a1f99fa01
remain frozen. The compositor and frontend are unchanged snapshots 84acceed40ace2be1362ecfd32239b0bc58b909f070801144a3e0962952168c9
and b77492eb34753d19901f1d0825cbb226a61c2436474445718504e850995d33b3.
Compressed ELF archives live at target/portal-pointer-matrix-12/binary-archives/;
decompression hashes match the executed /tmp snapshots. Cargo outputs/caches stay
in root target/ and were not moved or cleaned.

The backend-death case reaches actual held-button and Shift release, then times
out waiting for the frontend's public Closed signal. Producer/control/worker
assertions later in that case are not reached and are not claimed as passing.
src/xdp-session.c in the pinned standard frontend subscribes to backend Closed,
but does not track its impl-session proxy's name owner. The retained failure and
logs are in evidence/. This requires a companion source correction and further
session/owner-replacement checks; it is not a reason to weaken the assertion.
No frontend source, canonical recipe, installed service or routing was changed
in this slice.

Earlier setup/compile evidence is retained: the first headless fixture lacked
the explicit GSettings schema path and its frontend aborted; later setup is
isolated and works. The initial Rust attempts caught a Cow signal argument and
test import/generated-stream type mismatches, corrected before final validation.
The matrix waits for the final desired region configuration because independent
width/height operations may legitimately expose intermediate replacements.

[evidence.json](evidence.json) records hashes, exact scope, successes and the open
red. [portal.patch](portal.patch), copied baselines/current sources and
[replay.log](replay.log) preserve the eight-file delta. Native sources and frame
PNGs are retained under native-proof/. Full execution is in
target/portal-pointer-matrix-12/. See [ADR0054](../../../docs/adr/0054-device-availability-and-mixed-source-input.md).

Next: fix backend owner loss/replacement in a prepared standard-frontend delta,
keeping the accepted canonical source and binaries unchanged until validation.
Relevant canonical source/build locations are recorded in the thread checkpoint.
Broader source-loss/cancellation/race cases, independent physical input and touch,
complete companion archive adoption/distribution, and existing shared/native reds
remain before normal advertisement and completion of the 18-interface roadmap.

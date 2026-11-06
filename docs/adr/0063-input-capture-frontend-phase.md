# InputCapture frontend phase follows confirmed backend transitions

Status: canonical companion and scoped acceptance verified,
2026-11-01. Both phase regressions reproduce on the unchanged frontend in dark and
light appearances. The canonical candidate passes seventeen InputCapture/
Clipboard groups per appearance and the existing frontend consumers: 146 native
GUI/input/media checks and 45 private-bus owner checks. The required original-graph
workspace run repeats the same ten Files integration failures; all other Rust
targets and post-Rust stages pass, with unchanged source guards. Normal
advertisement and installed routing are unchanged.

[ADR0062](0062-input-capture-clipboard-activation.md) makes Clipboard authority
follow actual capture activation in the Rust backend. The standard frontend also
checks its own InputCapture phase before forwarding Clipboard reads. Two upstream
state writes can make those views disagree: repeated Enable overwrites Active
with Enabled, and Release predicts deactivation before the backend validates a
supplied activation ID. A rejected stale Release therefore leaves real capture
active while the frontend denies an otherwise authorized Clipboard read.

The native regression reaches the actual public call site, using the canonical
frontend, Rust backend, Iced consent, pinned Niri and a passive libei receiver.
An ordinary local selection is readable before the control. After the no-op or
rejected control, EI motion still arrives, no phase signal occurs, and the same
Clipboard worker remains alive. The old frontend denies the next read without
forwarding it. Four independent baseline runs reproduce this exact failure;
worker lifetime and method-call-only tracing distinguish it from backend
revocation or an activation-startup race. No Clipboard bytes or FD replies are
recorded by the trace.

The companion adds a separate one-file patch to the existing pinned frontend
recipe. Repeated Enable preserves Active; an actual disabled session retains its
existing transition to Enabled. Release forwards the validated request and waits
for Deactivated/Disabled before changing the cached active phase. Existing
session ownership, option validation and invalid-phase checks remain in place.
No reply acknowledgement is treated as a compositor state transition.

The frontend can briefly forward a read while a valid Release is in flight.
The Rust backend independently checks the actual activation at admission and
execution and revokes old pipes through its producer gate. Waiting for a confirmed
frontend transition therefore preserves availability after rejected controls
without extending byte authority after actual deactivation. Valid Release,
Disable, topology invalidation and session/owner loss remain native regressions.

The original archive, licence and previous patches are retained. Provenance pins
the new patch and the upstream/patched hashes of src/input-capture.c. Complete
499-file source inventories are verified before and after the recipe transition,
including a fresh archive materialization. Only that C source changes in the
prepared tree. The existing source path and one-job debug Meson/Ninja cache are
reused. Baseline and candidate frontends remain separately pinned; the candidate
is stored once per checksum for native consumers.

The regression stays in scripts/test-input-capture-portal.py. Its optional
phase-case selector isolates either baseline failure; the ordinary Clipboard
suite exercises both controls. Successful candidates must prove continued real
EI motion and a newly forwarded, readable Clipboard transfer, then pass the
original revocation cases. This observable seam would have prevented the cached
phase mutations from silently diverging from the compositor.

The [evidence ledger](../../.scratch/xdg-portals/input-capture/frontend-phase-evidence.json)
records the four baseline failures, source/recipe transition and final checks.
The workspace configuration stage rebuilds the normal application, so the final
public capture runs are repeated on that exact executable; earlier public runs
are retained separately. All final native consumers share the same backend,
frontend and compositor checksums. Ten capture-worker handles are terminal.
The main executable and harness have losslessly verified archives, reusing the
existing identical harness archive. The observed checkpoint adds about 0.37 GB
and leaves about 14.9 GB free.

The [InputCapture plan](../../.scratch/xdg-portals/input-capture/PLAN.md) retains
keyboard/touch, broader capture/failure/physical testing, companion distribution
and the full eighteen-interface roadmap. The repeated Files failures remain
unresolved, with their cause unestablished; scoped frontend success does not clear them.

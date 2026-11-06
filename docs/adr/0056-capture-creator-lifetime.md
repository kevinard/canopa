# Recording lifetime follows its creator connection

Status: compositor correction, canonical source reproduction and Rust contract
admission pass scoped acceptance, 2026-11-01. Portal 163 library/43 integration,
Clippy and 89 production native groups pass. Full workspace remains red on ten
Files recovery/Undo tests; all remaining original gate stages pass. Broader
matrix, physical/touch and distribution acceptance remain separate. ScreenCast
and RemoteDesktop stay unadvertised.

A capture worker killed before it can send Stop leaves a real recording alive
on the previous compositor. Independent two-client native tests reproduce this:
the creator disappears and its Session and PipeWire producer survive, while the
other client's recording remains live. Explicit Stop removes both. The creator's
NameOwnerChanged event arrives, so the missing compositor ownership rule is the
cause. This closes the worker-crash limitation recorded in ADR0041.

The compositor subscribes to the creator's unique-name disappearance before
publishing a Session and rechecks that name after registration. Loss of that
creator or its notification stream stops only its recording. Normal Stop cancels
the watcher and removes the retired registry entry. A weak registry reference
avoids retaining the collection through its own Sessions. The public Mutter
method signatures stay unchanged.

CanopaCaptureOwnerVersion=1 declares this creator-cleanup contract. It is
independent of CanopaSourceVersion, which identifies live capture sources.
The Rust backend requires the cleanup contract when resolving its pinned
compositor, and the worker rechecks it before CreateSession. A missing or unknown
version fails before capture mutation. Cooperative Stop remains necessary for
normal completion; creator ownership also covers worker SIGKILL and queued
requests whose callers disappear before the compositor processes them.

The maintained private regression observes actual method replies for queued
CreateSession and RecordMonitor calls, then checks that no extra Session/Stream
objects remain and another owner's real producer stays live. It cannot pass just
because a queued request has not run yet. All three cases fail on the preserved
baseline; the candidate passes repeated runs. The original mixed-source public
worker-kill case passes in both themes, with actual frames and held-input release.
Source closure, client/backend death and closing sharing controls also pass.
The rebuilt Rust portal independently rejects the previous compositor before
showing consent or creating recording objects. The same maintained admission
fixture exposes consent on the preserved pre-fix portal, providing a failing
control. Two library tests also reject absent/unknown contract versions and
recheck the contract between owner resolution and worker creation.

The final rebuilt portal/harness pass six public mixed-source lifecycle cases
and standalone ScreenCast in both themes: 89 groups, 80 ordinary video captures and
12 cursor captures. These runs pin the canonical compositor and frontend and keep
all source/binary checksums unchanged. Earlier pre-admission Rust results remain
separately identified. The shared gate completes every Rust executable with
`--no-fail-fast`; only Files integration fails (596 pass/10 fail/41 ignored).
Settings, CLI/configuration and fork library/event/pipe checks then pass using the
same cache. The sole source-guard delta is an identified diagnostic JSON receipt;
compiled source inputs stay unchanged. Validation grows storage by about 2.58 GB
within its 3 GB cap, with no per-run binary copies.

The explicit niri-desktop-pointer recipe combines the previously prepared
capture/drag/keyboard/pointer/scale stack with this correction. Its two patches
declare 22 Niri and eight Smithay files against the original pinned archives,
retaining GPL-3.0-or-later and MIT attribution. All 5961 materialized files match
the compiled working source; only two absolute local Smithay paths are normalized.
Prepared-source reuse verifies the recipe, paths and complete source inventory
before retaining existing Cargo package identities. It neither installs nor
selects a compositor.

[The capture-owner ledger](../../.scratch/xdg-portals/capture-owner/README.md)
records exact binaries, red controls, source inputs, startup failures and current
shared/native results. The entire portal replacement goal remains open.

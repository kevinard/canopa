# Phase62: selected CUPS runtime and longer credentials

Status: scoped native verified; full Print and full18 goal active. All builds,
native probes and workspace checks terminal. No question/blocker/agent/commit/PR.
11packaged/sixexperimental/Printopt-in; user Print-first, automaticQt deferred.

## Implementation

Runtime selection in print/worker/runtime.rs freezes one sealed image per backend.
Optional bundle: PREFIX/lib/canopa/print/libcups.so.2 beside PREFIX/bin; operator
CANOPA_PRINT_CUPS_LIBRARY. Absent bundle keeps system CUPS; invalid present/explicit
selection fails. Parent copies at most16MiB, never dlopens CUPS. Metadata UI gets
the same snapshot; only CUPS workers load it. Credential helpers/file operations
inherit no CUPS image. Production, jobs review and recovery Client instances share
the backend choice. Image+Library stay alive until exit for CUPS TLS destructors.
No ambient LD_LIBRARY_PATH selection. Source replacement cannot change the image.

The six-file CUPS overlay exports canopaCupsAuthenticationVersion()=1, declaring
127-byte users/1023-byte passwords/1151combined and the verified Digest fixes.
Missing/unknown capability fails before authentication. System soname retains
64-byte users/189combined; oversized input is refused before truncation. Typed
private policy is part of exact server-scope equality and sealed prepared handoff.
Parent refuses extended challenges from a system-selected worker. Queue fields
retain their separate contract. No public Print option chooses code or raises limits.

All1301 CUPS sources verified before/after same-cache capability rebase. Only
cups.h/usersys.c changed from the sealed phase61 candidate; library-only rebuild13s.
Client SHA e0d80ef1ff4154878f72d00a1e45c65117c6420991eda568b8bb0b74422cb457.
Recipe7a275e0a029f70b22f935d747a7e20c997d62fced852db7c627897a0185c4058,
patch3ef0d7e68659d42014fce1cdd772b0789e213f9548f16386a4c83e58fe712769.
Private CUPS structures changed: never inject into installed cupsd/tools/filters.
Only public opaque APIs cross the Rust FFI seam; no GTK implementation/dependency.

## Validation and scope

worker03: strict workspace Clippy164s, original default/workspace/alltarget compile
769s PASS. Restricted test run failed111 socket/DBus refusals (324pass); retained.
Compiled continuation worker04 PASS435 serial Portal library/55integration/PPD1,
without another build. All source guards unchanged. Current Rust binary:
a45799db32759ba4857d5255eaf840e359089e35a2e5b5e98e96bb08a16561ee.
Frozen20 changed/new compiled inputs (19Rust plus provenance), source-only snapshot
in target/portal-print-62/candidate-01-sources and manifest candidate-01-sources.json.

Actual selected-runtime worker matrix:120 cases, selected SHA256/MD5/Basic31each,
systemSHA25627. Maximum ASCII/UTF8 values, legacy64/189 refusal, missing/unknown
capability, TLS/channel/ordered queue cases pass. Real private IPP scheduler and
independent exact-byte authentication gateway; no actual Digest/PAM/physical claim.

Actual Iced/public matrix02: four complete runs, Basic/SHA256 × dark/light,44groups.
Max127/1023 ASCII(dark)/UTF8(light), wrong-once, prepared private cache after UI exit,
direct printing, GTK/explicitQt six expected pages, server/device cancellation,
Request.Close/caller loss/Lockdown/shutdown pass.70 mapping/descriptor observations
across these and reduced captures keep CUPS out of backend/metadata/credential UI;
credential helpers have no library FD. Workers map the backend image's exact inode
and SHA. Parent-runtime01 separately proves actual Rust presentation handoff to an
owned Python child and source replacement; its scope is transport, not Iced.

Two reduced SHA256 capture runs add4groups and verify masked maximum fields after
asynchronous paste redraw. Both visually inspected, readable, bounded fields with
masked dots. Earlier full-run screenshots sometimes precede paste redraw; functional
byte equality passes there, mask proof comes from reduced captures. Total808 unique
observed identities terminal, private auth data removed. No native process remains.

Required bash scripts/check.sh --workspace ran once on final relevant source,
original graph/cache/alltargets/noFailFast. Only failing target is Files integration:
596passed/10failed/41ignored; exact same ten names as phase61, nested crash helper
separate. Other executables and independent settings/CLI/configuration/vendor
runtime stages pass. workspace-failures01 has exact names. No full workspace claim.
Prior parallel-handoff and early-TLS observability issues remain open.

Retained failures: worker01 publication-call type errors, worker02 collapsible-if
Clippy error, worker03 restricted socket run. Basicdark01's wtype5s timeout entered
no completed group (16observed terminal); original toolkit source retained. Owned
single-paste sources on private nested desktops handle long synthetic text and
exit after one read; no user clipboard involved. No product fix for that fixture
failure. Original phase61 negative/invalid receipts remain with historical sources.

## Evidence, storage and next work

Sealed [selected-authentication-runtime-evidence.json](print/selected-authentication-runtime-evidence.json)
and target/portal-print-62/acceptance-01.json. Replay source only in target62/replay.
Old/new Rust binaries each compressed once losslessly by checksum (38.8MB each),
old/new client images each retained once in shared checksum store. Unsupported
capability99 fixture is one15176-byte shared library. No per-evidence ELF copies,
new Rust graph, release profile or cache purge. Phase net growth434,184,192 bytes;
6,139,363,328 bytes free at seal. Compile reserve6GB; cached execution5.5GB reserve
and same1GB phase cap, including temporary test files. Reuse all stable paths.

Next read [the continuation map](print/runtime-integration-next.md): selected-runtime
protected Create-only/Send-only, explicit recovery and same-UUID held jobs, lost
acknowledgements/ownership regressions, then resource/timeouts/file-output lifetime
and broader connection/driver/device/Flatpak/runtime/package/NOTICE/release gates.
Raw --print-worker without Input.cups_runtime deliberately uses system CUPS even
if the operator environment variable is set; do not claim selected coverage from
that. Do not rerun completed matrices/broad gate absent relevant edits or a new
failure/concern. Full18 remains active, automaticQt deferred, no approval needed.

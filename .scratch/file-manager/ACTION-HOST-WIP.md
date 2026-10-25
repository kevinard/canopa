# Custom-action host checkpoint

Current goal turn: progress. ADR0082 is scoped-verified and preserved; the entire
file-manager PRD remains active. All compiler/native/archive/cache handles are
TERMINAL. Both code and final-document review axes report zero findings.

Final source08:567 inputs, archive SHA7e3ea358ecba68920dea2e993fa1d28d83f5023dc5a88ebf69c639413ce25fff.
Binary ab3caff0b861d50ea9560ecbf8024af423f23e4b7a1065082002dddb78ccd984,
119648664B; verified gzip canopa-files-ab3caff0.gz/28808558B. Preservation19848
exit0 verified every archive member, current input hash and decoded binary.
Source08 changes three docs only from07; production binary built06 is unchanged07.
See action-host-verification.json and action-host-preservation-08.json.

LocalActions now owns ActionJobs startup and completion-notification handoff
before Iced. Narrow Submit/Cancel/Shutdown/snapshot/notices prevent frontend
receipt stealing. Shared handoff/snapshot lock plus serialized Shutdown preserve
holds; delivery futures bounded17 and failures visible. Core ActionJobs unchanged.
Shared standard notification transport moved outside Iced; Notify retains policy.
Frontend snapshots, drafts/output/clipboard/window state remain presentation.

Evidence: compilation red62319 (new interface absent, not prior behavioral bug);
first green34961/1; narrowed focused34384/5 noGUI; private-bus headless35311/3,
91580/3 then stronger burst41425/4. Final burst20 exact completed commands,
17 actual delivered titles,3 explicit busy failures, held Notify refusal then
release/orderly shutdown. No display/compositor/Iced artifacts in headless build.
Default all-target Clippy25139/0/3 existing warnings; changed-test Clippy49992/0;
Iced/default-build/fmt35938/0/47. GUI38974/0:42 checks (12 operations-light,
15 action-dark,15 action-light). Operation app log empty; action logs only4
expected unavailable remoteGIO monitor messages each. Representative result,
background-work, narrow output and cancelled captures inspected. Both16-path
reviews07 and both3-doc reviews08 report0/all pinned hashes match.

Inherited source05 omitted the modified shared notification receiver; supplemental
manifest pinned it before/after native02 and both reviewers checked it in full.
06 merged it;07 added four unchanged runner/Session/pointer inputs omitted by the
inherited map. Final08 preserves all567 inputs. No claim those helpers were newly
implemented. Cache cleanup removed4 completed library-test ELF caches180071400B
under Cargo locks; application/libs/source/evidence preserved. Do not blindly
remove canopa_files-*; production aliases share that prefix.

Next: filesystem attention still UI-owned in iced_desktop/file_operations.rs;
move observation/generation/completion handoff and holds to the host, retaining
UI refresh/typed question presentation. Then choose optional resident process
layout/local primary activation and implement actual host/CLI/session integration.
No daemon flag exists. Windowless Iced is not display-free hosting. Keep queued
execution/original-media/source/journal/restart authority and safe Retry/Continue
as independent unfinished gates. Startup first-window timeout remains open.
Full workspace/native/release/packaging and the whole PRD remain unfinished.
Follow NEXT-EXECUTION-NOTES.md. Never use obsolete operation-startup finalizer.

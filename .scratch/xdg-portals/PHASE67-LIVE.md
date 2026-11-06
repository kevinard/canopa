# Optional Print runtime packaging — sealed

Final state: progress; full18 goal remains active, Print opt-in, Qt routing deferred.
All build/native/tooling handles are terminal. See ADR0104 and
print/packaged-runtime-evidence.json. No Rust/Cargo/client/provenance changes.

The full eight-binary archive and required-runtime smoke pass. Its duplicate
178794296-byte tarball is removed after SHA/manifest verification. Twelve actual
metadata-worker cases on the packaged image pass. A relocated normal backend
finds the image with no environment override; three public frontend/Iced file
groups, three isolation observations and30 observed terminal PIDs pass. Private
data is removed. Prefix executable was a hard link, not a data copy; temporary
prefix is removed after verifying the authoritative root executable/client pins.

Final packaging driver passes eight integrity/refusal groups, including directory
symlink refusal before writing. That last preflight refinement follows the native
and archive checks; final staging is compared against their archived manifest to
prove normal output is unchanged. Final Python/shell/workflow syntax, check-script
regressions and packaged documentation pass. The Ubuntu release workflow is
configured but not executed locally. Full workspace was not repeated for this
tooling/docs-only change; phase66 Files failures remain recorded.

Current guard target/portal-print-67/source-guard-final-01.json records packaging
tooling plus unchanged1310operational inputs. Continue existing outer60/300s
supervision and repeated-authenticated-FD checks, then driverless/device/Flatpak
and release/deployment gates. Packaging/NOTICE is locally verified; do not reopen
resolved work merely because another turn starts. Historical launch notes follow.

Previous goal turn: progress. Phase66 quoted authentication fix and its native,
workspace and source evidence were sealed. No pending build/native handle at start.
Current operational 1310-input guard is unchanged, Rust EF722d77... and CUPS
49cedda4... still current. No Rust, dependency, canonical patch or provenance edit.

New package-print-runtime.py verifies the successful current build receipt, all
source inputs, image bytes and static ELF metadata before staging the private
library. It packages original LICENSE/NOTICE, complete patch, modification summary
and a relocatable manifest. package-release.sh selects this with --cups-runtime;
smoke-release.sh checks extracted bytes and accepts --require-print-runtime.
The release workflow builds the pinned archive and selects the optional runtime.
Print stays opt-in; system daemon/tools stay separate.

New regression driver passes seven packaging/refusal groups. Initial standalone
stage failed because the manifest parent was absent; fixed and the complete final
driver passes. That failure is retained in package-checks01. Documentation staging
passes36files/74offline links. Python/shell/workflow syntax and invalid shell flags
pass. Next actual worker packaged-image checks, full archive/smoke and a normal
backend relocated-prefix discovery check without an environment override.

Reuse root incremental/default graph and existing executable/client checksum
stores. No Rust compilation, fresh graph or per-native executable copy. Full
archive testing uses temporary staging and removes its duplicate test archive
after checksum/receipt verification. Keep concise evidence and source-only replay.
Root reserve5GB, maximum retained growth100MB. Print-first/Qt-deferred; full18active.

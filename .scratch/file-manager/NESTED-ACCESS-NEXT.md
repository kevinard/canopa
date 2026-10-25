# Nested restricted-folder access

The complete five-area Files goal remains active. Do not mark it complete for this
slice. The previous goal turn made verified progress; this turn adds concrete fixes
and complete scoped checks. No agents, commits, vendor changes or main application/
Niri rebuilds were made. Preserve unrelated dirty work, especially Notify.

Current source is nested-access-inputs-09.json: 882 unchanged build inputs, schema49.
The predecessor restricted-access-inputs-09.json and its source archive SHA
1500532777440e82a872a0433e57703ed7b6e33d7c68d3517d416e38e2b9a933 were reverified
before editing. The new acceptance module is transfer/stage_boundary/access_checks/
nested_checks.rs. Root/Files instructions, contexts and ADR0140 were read.

Final verification passes 519 backend tests across 20 targets (47 ignored helpers)
and 285 normal isolated default-feature library tests (27 ignored), zero failures or
filters. Backend library02 passes 204 tests (26 ignored); driver02 batches 90..99
pass all 19 integrations, including both required active/stopped batch gates 92.
Default receipt: continuation-core-1792809594-receipt.json. Default all-target
Clippy02 with warnings denied, Files format03 and fixture-check03 pass. All test
handles are terminal. No complete native/workspace or release acceptance is claimed.
Exact proof: nested-access-verification-01.json. Final source/evidence archive:
nested-access-source-01-verification.json; verify current members after its archiver
is terminal. The previous app/Niri binaries and the rebuilt required gate remain
preserved. No final application binary was built by this slice.

Implemented behavior:

- Continue borrows each owned created ancestor shallow-first, requiring its recorded
  folder and parent Original generations on the actual opened objects before chmod.
- Startup restores abandoned read grants even when an already-restored ancestor
  hides the descendant. Each borrowed original ancestor has a durable grant before
  adding read/search. Opened ancestors unwind in reverse order after failures.
- Raw database path keys must match their grant bodies. Replacements, unavailable
  originals and refused/ignored saves preserve the original grant for retry.
- A writer reuses a reader's old original mode only while the current mode is its
  actual borrowed grant. External modes 0701 and 0444 survive Continue. Restoration
  extends only matching reader mode/object/parent contexts. Fresh Recovery can
  acquire newer context while an older non-searchable lease remains alive.
- Existing schema49 records suffice; no new journal representation or fresh transfer
  execution authority is created. Test hooks compile out of production and kill
  only separately launched private fixture processes.

Attempt/source relations:

- Inputs01 red01/red02: actual nested Continue stops with Permission denied.
- Inputs02 green01: continuation reaches its real partial-write SIGKILL and catalog.
- Inputs03 red03/red04: a real release-worker death leaves inner 0500 hidden behind
  already-restored outer 0400; a held descriptor proves the untouched original mode.
- Inputs04 green02: both parents pass, including expanded Copy/Keep/Continue/restart.
- Inputs05 green03: five parents pass, adding startup death, replacements, save faults.
- Inputs06 green04: five pass; replacement refusal is correct but wording assertion
  too narrow (replaced versus original). Inputs07 fixes that expectation only.
- Inputs07 full intermediate matrix: 517 backend tests, 47 ignored, 20 targets pass.
  No default run for that source. Later new cases justified changing/retesting it.
- Inputs08 mode-red01/mode-red02: both 0701 and 0444 are replaced with 0500 twice.
- Inputs09 green05: all eight nested parents pass, three ignored child helpers.
  Final backend/default/static checks above use this exact source.

The helper add-nested-live-mode-acceptance.py HAS been applied. Do not apply again;
it now guards against duplicate application. Live-mode test cases share the full
public Copy/Keep/Continue flow and verify fresh Recovery and another restart while
older selections remain alive. Other cases cover death between releases, another
startup death after granting ancestor access, refused/ignored grants with retry,
and replacement ancestors both before Continue and during startup restoration.

Failure evidence: archive01 preserves five roots/99 entries, SHA
8b1d5856697b9f0f999ee146e17304084a4f833524a33085da3487135933dcca.
Archive02 preserves four mode-regression roots/96 entries, SHA
11bab3c1a7e3d64558d70322ea187e50e4fbfba1347f7def722f8b4762808a1e.
Both have decoded and separate live verification. Bytes, modes, mtime, device and
inode match; temporary archive/inspection search borrowing restores exact modes
and explicitly records changed directory ctime. Do not mutate failed fixtures.

Use CARGO_INCREMENTAL=0 TMPDIR=/tmp, -j1 --locked --offline. Finish formatting before
capturing manifests; freeze hashed inputs during verification. Never overlap Cargo
mutations, or replace a test ELF while current_exe children use it. Reclaim only
exact audited completed test ELFs under both Cargo locks; preserve app, Niri, the
required operation_transfer_resume gate, source and evidence. No RUSTC_WRAPPER or
workspace wrapper was used. Keep fresh log/archive names and all failed attempts.

Next implementation priority: generic Forward/Undo abort cleanup. Current clean_stage
has legacy identity-only remove_file/remove_dir branches; some Undo paths clear the
journal first. Build actual public crash/data-preservation fixtures, then replace
these paths with durable task-bound limited disposal rights. Broader Undo/expiry,
queued lineage, large reports and Trash/Restore remain open.

Other acceptance ideas from inspection, not reproduced findings: stress multiple
queued lease releases and catalog replacement against raw-address token reuse;
test relocation/copy of ancestors with active permission contexts. Build real
public feedback loops before changing those protocols or claiming failures.

All backend navigation/search/preview separation, product/media/drag/device/
accessibility/physical acceptance, startup/focus/optimized measurements, complete
native/workspace/Shell/Notify checks, Niri distribution, release package/docs and
GTK-reference retirement remain open. The full goal is unchanged and active.

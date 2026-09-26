# Banner output-lifetime implementation review

Fixed point: `df88ecda9f2170ff9a1d1e3f62531882a786761e` (user-selected `df88ecd`). Starting tranche HEAD: `5951e3d`. Review command: `git diff df88ecd...HEAD`, supplemented by the pending working-tree diff and new surface/test/ADR/evidence files. Commit list: `8be0e99 9289e34 4eaa158 cf449ee 9d62fcf 622201a c170b6d a572593 2b8a21b 24f66bb 9f9f129 6260b01 aae365b 77d3aaf 60d733f 6397b9e 1bf5698 7ce2a0f c681dfe 559cdfa 2394849 8b752f3 5951e3d`.

The implementation skill required independent parallel Standards and Spec reviews. Sources: `AGENTS.md`, tracker/domain instructions, `CONTEXT.md`, relevant ADRs 0028/0033/0043–0047 and completion-plan W07/W10/W11. Standards included the complete Fowler smell baseline, with repo rules overriding heuristics. Scope is this continuation and its relevant cumulative interactions, not recertification of the entire plan.

## Standards

Initial review: zero documented-standard violations and zero actionable heuristic smells. The private surface owner serializes lifetimes, owns one cancellable deadline and rejects retired-window geometry, while acknowledgements retain captured ownership. The deferred-close resource issue was passed to Spec without double-counting it as a heuristic smell.

Final repair/evidence recheck: zero documented-standard violations and zero actionable heuristic smells. The reviewer verified the bounded close repair, appended tracker history, ownership/limitations in the domain and compliance docs, all six captures and relevant raw evidence. Standards: **0 outstanding findings; worst: none.**

## Spec

Initial P2 finding: a queued window close arriving after output removal could retain `RemoveWindow` forever in the adapter's unknown-ID retry queue. Repeated races violated W07's bounded post-completion work requirement. The new adapter action-consumer test reproduced 100 retained closes before the fix. Closed windows now purge queued actions; late closes are dropped when neither the renderer nor an existing backend binding owns the ID. Backend-owned windows awaiting their first refresh still retain their close request. No lifetime tombstones are introduced. All four standalone vendored adapter library tests pass after the correction.

The adapter is excluded from the workspace. Its separate test command is:

```sh
cargo test --offline --manifest-path vendor/iced_layershell/Cargo.toml \
  --target-dir target \
  --config 'patch.crates-io.layershellev.path="/home/kevinard/projects/canopa/notify/vendor/layershellev"' \
  --no-default-features --lib
```

`--no-default-features` matches Notify's adapter selection. An initial standalone build with the vendor's default theme-detection feature failed in its existing stream import; no unrelated feature repair was included. A generated standalone lockfile was removed after verification; the workspace lockfile and dependency declarations are unchanged.

Final applied-fix/evidence recheck clears the P2. The reviewer verified null-output requests, both action signals, one ID 3 expiry, pointer click-through and the distinction between graphical testing and the later adapter-only repair. Spec: **0 outstanding scoped findings; worst: none.** [Graphical evidence](OUTPUT-LIFETIME-EVIDENCE.md) leaves broader compositor coverage, exact-deadline proof and full W10/W11 acceptance open.

Final verification: **401 private-bus workspace tests and four separate adapter tests pass**, with zero failures or ignored tests. Workspace/all-targets typecheck and Clippy, workspace/vendor formatting, whitespace and protocol-snapshot checks pass with existing warnings. Summary: Standards — 0 outstanding findings, worst none; Spec — 0 outstanding scoped findings, worst none.

# Banner-image continuation review

User-selected baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Committed comparison: `git diff df88ecd...HEAD`; cumulative worktree comparison: `git diff df88ecd`, including initially untracked image preparation, shared local-file reads, renderer-consumer fixtures and ADR 0036. Commits since baseline are `8be0e99`, `9289e34`, `4eaa158`, `cf449ee`, `9d62fcf`, `622201a`, `c170b6d` and `a572593`. This tranche began at `a572593`.

The implementation skill required independent parallel Standards and Spec reviews. Both reviewers inspected production code, fixtures and scoped evidence without editing files or starting competing builds. W07/W09 remain in progress: upstream icon discovery/index replacement, broader alternative-resolver evaluation, provider streams, recovery and actual host/graphical acceptance are not claimed complete.

## Standards

No hard documented-standard violations. The initial review identified two low-priority heuristic concerns: duplicated security-sensitive file reading in image and sound modules, and the supersession fixture starting its timed executor blocker before acquiring the shared fixture lock.

Both were corrected and independently rechecked. `local_file` now owns bounded nonblocking opened-descriptor reads and content-free errors, including byte consumption before partial-read/overflow failure. Image aggregate accounting and sound/theme fallback remain with their respective owners. Both controlled-executor fixtures start their blocker after acquiring the serialization guard, and test through presenter/renderer output rather than private decoder or semaphore state.

Final Standards result: zero unresolved documented violations or actionable heuristic findings. Worst outstanding issue: none.

## Spec

The initial review identified one P2 regression: bypassing Iced's path loader also bypassed EXIF orientation. A new asymmetric four-quadrant fixture failed for a mirrored JPEG before the correction. All eight orientation cases now pass for JPEG and PNG, using literal metadata tags and independently expected dimensions/corner colors. Metadata is read from the same bounded decoder input; transformations preserve the pixel budget and publish resulting dimensions.

The independent recheck confirms the orientation correction, shared-reader accounting, actual-worker admission lifetime, supersession fallback and callback semantics. Discarded preparation cannot acknowledge display; already-delivered frames retain their prior semantics. No other selected-scope omission, incorrect behavior or scope expansion was found.

Final Spec result: one resolved finding, zero outstanding findings. Worst outstanding issue: none.

Summary: Standards — 0 outstanding findings (2 heuristic concerns resolved), worst none; Spec — 0 outstanding findings (1 P2 resolved), worst none. Final verification evidence is recorded in [IMPLEMENTATION.md](IMPLEMENTATION.md).

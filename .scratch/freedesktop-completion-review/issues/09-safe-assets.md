# W09: Safe image decoding and standards-aware icon/sound lookup.

Status: In progress

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W09. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-11: Continuing from `c170b6d` with bounded sound-theme resolution under ADR 0035: indexed profile directories, ordered roots and inheritance, event/locale fallback, disabled overrides and metadata refresh. Tests stay at the existing public SoundPresenter seam with offline mixer output. Image/icon work, broader resolver comparison and graphical acceptance remain open; this is not completion of W09.

2026-09-11: Selected sound-theme semantics implemented and independently reviewed against `df88ecd`, with zero scoped findings. All 35 focused sound tests and 304 workspace tests pass, with zero failures or ignored tests. Typecheck, Clippy, formatting, whitespace and Niribar protocol drift checks pass with no new warnings. Public-presenter fixtures verify indexed/profile/root/parent/name/locale/extension precedence, disabled and unsafe lookup suppression, metadata refresh and bounded traversal. ADR 0035 records the chosen specification interpretation, local safety policy and limited libcanberra interface assessment. See [SOUND-THEME-REVIEW.md](../SOUND-THEME-REVIEW.md). Image/icon work, broader alternative-resolver evaluation and graphical acceptance remain open; this issue is not complete.

2026-09-11: Continuing from `a572593` with renderer-side bounded local-image decoding through the existing Iced presenter/renderer-consumer seam. The first red regression reproduces a corrupt existing image suppressing its alternative text. This tranche will deliver stable pixels with bounded file/decode work outside notification mutation; icon-theme discovery semantics, source-index invalidation and graphical/host acceptance remain separate.

2026-09-11: Bounded banner image preparation implemented under ADR 0036 and independently reviewed against `df88ecd`, with zero outstanding scoped findings after correcting EXIF orientation, shared bounded-file reads and fixture synchronization. All 76 focused presentation tests and 35 sound tests pass; the final private-bus workspace run passes 318 tests, with zero failures or ignored tests. Initial control-spine startup timeouts and unchanged successful reruns are recorded in [IMPLEMENTATION.md](../IMPLEMENTATION.md). Typecheck, Clippy, formatting, whitespace and Niribar protocol drift checks pass with no new warnings. See [BANNER-IMAGE-REVIEW.md](../BANNER-IMAGE-REVIEW.md). Icon-theme discovery/index invalidation, broader alternative-resolver evaluation and graphical/host acceptance remain open; this issue is not complete and no live desktop service or installed configuration was changed.

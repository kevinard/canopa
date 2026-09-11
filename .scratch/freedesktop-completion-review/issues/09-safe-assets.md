# W09: Safe image decoding and standards-aware icon/sound lookup.

Status: In progress

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W09. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-11: Continuing from `c170b6d` with bounded sound-theme resolution under ADR 0035: indexed profile directories, ordered roots and inheritance, event/locale fallback, disabled overrides and metadata refresh. Tests stay at the existing public SoundPresenter seam with offline mixer output. Image/icon work, broader resolver comparison and graphical acceptance remain open; this is not completion of W09.

2026-09-11: Selected sound-theme semantics implemented and independently reviewed against `df88ecd`, with zero scoped findings. All 35 focused sound tests and 304 workspace tests pass, with zero failures or ignored tests. Typecheck, Clippy, formatting, whitespace and Niribar protocol drift checks pass with no new warnings. Public-presenter fixtures verify indexed/profile/root/parent/name/locale/extension precedence, disabled and unsafe lookup suppression, metadata refresh and bounded traversal. ADR 0035 records the chosen specification interpretation, local safety policy and limited libcanberra interface assessment. See [SOUND-THEME-REVIEW.md](../SOUND-THEME-REVIEW.md). Image/icon work, broader alternative-resolver evaluation and graphical acceptance remain open; this issue is not complete.

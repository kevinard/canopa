# Notification Center keyboard review

Notify fixed point: user-selected `df88ecd` (`df88ecda9f2170ff9a1d1e3f62531882a786761e`). Starting continuation HEAD: `2284945`. Review command: `git diff df88ecd...HEAD`, supplemented by the pending documentation/evidence files. Host fixed point: `d7870c2d20cff5d0f84da03296e15bf65115cc90`; starting HEAD: `b5d1b54`. Host command: `git diff d7870c2d20cff5d0f84da03296e15bf65115cc90...HEAD`, supplemented by the current working-tree diff and new action-widget/renderer tests. Prior reviewed changes are context; findings below concern this bounded W11 continuation.

Independent Standards and Spec agents reviewed the approved W11 plan, issue 35, the Center verification checklist, both domain contexts, Niribar ADRs 0008/0011 and Notify ADRs 0041/0042. The implementation remains generic host presentation; no notification-specific action interpretation or capability advertisement was added.

## Standards

Zero documented-standard violations and zero actionable heuristic findings. The shared action widget applies opaque-interaction-ID state ownership to both ordinary controls and the selected preview. Its Iced trait forwarding supports a concrete state-reset responsibility, not an unnecessary pass-through abstraction. Centralizing button construction/reset avoids duplicate logic without expanding the provider adapter's public interface.

The renderer regression tests observable press/replacement/release behavior for both button forms. Evidence distinguishes visible keyboard guidance from platform accessibility and leaves broader acceptance open.

Standards: **0 outstanding findings; worst: none.**

## Spec

Initial **P1**: ordinary buttons preserved Iced pointer-press state across replacement while rebuilding their message with the new interaction ID. Releasing an old press could invoke replacement content, contradicting “an old click cannot invoke the new snapshot.” Serialized stale-ID checks could not reject an event already constructed with the new identity.

The real-widget regression reproduced the violation before correction. All content actions, permission options, item actions and selected-preview controls now use the identity-owned state reset. The regression passes for old-release suppression and fresh-gesture activation. The independent applied-source recheck clears the P1.

No remaining scoped omissions, incorrect behavior or scope creep. Action-first contextual labels retain exact opaque wire targets and revision values. Full screen-reader integration, complete keyboard reading/navigation of overflowing context, the broader rich-content/permission/scale matrix and host recovery acceptance remain explicit W11 work, not completed claims.

Spec: **0 outstanding findings; worst: none.**

Final evidence recheck: both independent agents confirmed the 1,145-test total, captured signal outcomes and bounded graphical claims, with zero additional findings. The reviewed host implementation is committed as `bdda852`.

Summary: Standards 0 / Spec 0 outstanding scoped findings. Test and graphical outcomes are recorded separately in [CENTER-KEYBOARD-EVIDENCE.md](CENTER-KEYBOARD-EVIDENCE.md).

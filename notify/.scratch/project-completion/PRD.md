# Complete the remaining project release work

Status: local implementation and current-session rollout verified; remaining gates tracked in issues/

The selected static Notifications 1.3 feature work is complete. This work follows
the user's four remaining items, without adding optional portal-v2 product scope:

1. Install the paired Notify/Niribar builds, configure `canopa-v4`, and verify
   ordinary application notifications, desktop portal links, and session startup.
2. Exercise interrupted SQLite migrations/imports and real busy errors; investigate
   configuration-test failures under load and correct defects established by tests.
3. Run the configured hosted workflows and provision a dedicated graphical runner.
   The user confirmed on 2026-09-21 that no GitHub repository exists yet. Prepare
   runnable checks and setup instructions; leave hosted execution explicitly open.
4. Reconcile older tickets with current action reachability and future-request-only
   policy/reload semantics, preserving historical evidence and stable ticket paths.

Review baseline: Notify `80f2134cb981fe68e618bfcf60bb45873bb86c3a`;
Niribar `e4280a1`. Results belong in `docs/project-completion-verification.md`.

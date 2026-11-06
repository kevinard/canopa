# Domain documentation

Read root `CONTEXT-MAP.md` before exploring the codebase. It maps the application
contexts and shared infrastructure:

- Shell language and decisions: `docs/shell/CONTEXT.md` and `docs/shell/adr/`.
- Notification language and decisions: `docs/notify/CONTEXT.md` and `docs/notify/adr/`.
- File manager language and decisions: `docs/files/CONTEXT.md` and `docs/files/adr/`.
- Wallpaper/lock ownership and setup: `docs/session-tools.md` and integration ADRs
  0119/0120. The compositor owns lock security; PAM owns authentication policy.
  `canopa-widgets` shares passive data/command readers while consumers own UI,
  privacy choices, actions and lifetime.
- Login/greeter ownership: `docs/greeter.md` and ADR 0121. Greetd owns PAM,
  privilege changes and login-session lifetime; Canopa owns presentation.
- Polkit ownership/setup: `docs/polkit.md` and ADR 0122. Native polkit owns action
  policy, permitted identities, credentials and receipts; Canopa owns the dialog.
- Pinentry ownership/setup: `docs/pinentry.md` and ADR 0123. GnuPG owns keys,
  verification, policy and agent caching; Canopa owns the Assuan dialog.
- Integration decisions: `docs/adr/`.

Use each context's glossary terms when describing its concepts. Shared crates do
not transfer notification policy or persistence into the shell. Surface conflicts
with accepted ADRs explicitly instead of silently overriding them.

Application instructions live in `crates/canopa-shell/AGENTS.md`,
`crates/canopa-notify/AGENTS.md`, and `crates/canopa-files/AGENTS.md`. Repository
commands run from the workspace root.

# Domain documentation

Read root `CONTEXT-MAP.md` before exploring the codebase. It maps each context to
its glossary, architecture page and decision records:

- Shell: `docs/shell/CONTEXT.md`, `docs/architecture/shell.md`, `docs/shell/adr/`.
- Notifications: `docs/notify/CONTEXT.md`, `docs/architecture/notify.md`,
  `docs/notify/adr/`.
- Files: `docs/files/CONTEXT.md`, `docs/architecture/files.md`.
- Portals, desktop sharing and Print: `docs/architecture/portals.md`,
  `docs/architecture/desktop-sharing.md`, `docs/architecture/printing.md`.
- Wallpaper, lock, greeter, polkit, pinentry and idle:
  `docs/architecture/session.md`. The compositor owns lock security and PAM owns
  authentication policy; greetd owns login authentication, privilege changes and
  session lifetime; native polkit owns action policy and credentials; GnuPG owns
  keys and agent caching. Canopa owns the presentation.
- Settings: `docs/architecture/settings.md`. Settings presents preferences; their
  existing owners keep policy and lifetime.
- Shared infrastructure (configuration, appearance, assets, i18n,
  accessibility, service clients): `docs/architecture/foundations.md` and
  `docs/architecture/panel-protocol.md`.
- New decisions: `docs/adr/README.md`.

Use each context's glossary terms when describing its concepts. Shared crates do
not move notification policy or persistence into the Shell. Surface conflicts with
documented decisions explicitly instead of silently overriding them.

Application instructions live in `crates/canopa-shell/AGENTS.md`,
`crates/canopa-notify/AGENTS.md` and `crates/canopa-files/AGENTS.md`. Repository
commands run from the workspace root.

# ADR0029: Explicit Iced permission decisions

Status: accepted; final native/shared validation in progress, 2026-10-29.

The Access backend presents a direct grant/deny question. The XDG frontend keeps
permission persistence and resource policy. Reuse the portal request host,
application-owned helper process, shared appearance and exported Wayland parent
support. Keep one common parser/result validator for FileChooser and Access
choices, one bounded icon preparer and one bounded one-shot helper exchange.
AppChooser retains its framed live-update transport.

Grant must require an explicit action. An unhandled initial Enter, Escape,
window close, Request.Close, caller loss, invalid control/result or helper failure
must never produce a grant. Render title/subtitle/body as plain text. Keep long
body/choice content scrollable with the two decision actions reachable. Optional
symbolic icons are prepared off the UI thread with the existing bounded PNG/SVG
loader; missing icons leave the question usable. Compositor policy still governs
optional modal behavior.

Focused model/wire tests, eight native checks per theme, including the public
Camera consent handoff, pass. The shipped descriptor now advertises Access; final
fixture/regression and shared-gate results are recorded in the evidence ledger.
Wire checks cover complete argument ordering, bool/combo choices, malformed options, invalid returned selections, owner-authenticated Close
and caller loss with a non-cooperative presenter. Native checks must cover both
explicit decisions, no grant on initial Enter, Escape/Close/caller loss,
concurrency, long text and both shared appearances. Backend-native success does
not establish a real frontend consumer's permission persistence or camera/media
access; record those scopes separately.

See [the upstream contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.Access.html)
and [the evidence ledger](../../.scratch/xdg-portals/IMPLEMENTATION.md).

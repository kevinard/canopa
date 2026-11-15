# Canopa polkit agent

`canopa-polkit` is the polkit authentication agent for the user's graphical logind
session. It shows a dialog only while a request is active and uses polkit's native
agent library (`polkit-agent-1`) to check credentials, so polkit keeps action policy,
identities and authorization receipts. It starts with the Canopa session as
`canopa-polkit.service`.

- [Polkit agent guide](../../docs/polkit.md)
- [Session architecture](../../docs/architecture/session.md)

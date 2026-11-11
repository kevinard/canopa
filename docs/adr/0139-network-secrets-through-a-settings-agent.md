# 0139 Network secrets through NetworkManager and a Settings agent

Status: accepted. Builds on [ADR 0137](0137-shared-system-service-clients.md).

The Settings Network page joins WPA-Enterprise networks, adds WireGuard and
plugin VPNs and edits saved connections. These need passwords, private keys
and one-time codes. NetworkManager owns connection profiles and their secrets;
Canopa must not keep a second store.

## Decision

Canopa never stores network secrets. A secret reaches NetworkManager in one
of two ways:

1. **In the profile**, when the person types it while creating the profile
   (Wi-Fi password, WPA-Enterprise password or key password, WireGuard keys).
   The secret flags say who keeps it. "Remember the password" (the default)
   uses flags 0: NetworkManager stores the secret with the profile under its own
   permissions, so the Shell, the login screen and autoconnect keep working
   without any agent. "Ask every time" uses flags 2 (not saved): the profile
   holds no secret.
2. **Through the Settings secret agent**, when NetworkManager asks later: a
   not-saved secret, a rejected password it wants again (`REQUEST_NEW`), or a
   plugin VPN's password. `canopa_services::network::secret_agent` serves
   `org.freedesktop.NetworkManager.SecretAgent` on Settings' system bus
   connection and registers as `org.canopa.Settings` while the window runs
   (from the first time the Network page opens). Each `GetSecrets` becomes a
   question on the page; the answer goes back to NetworkManager and is
   dropped. Requests that do not allow interaction get `NoSecrets` at once,
   because the agent has nothing stored. `SaveSecrets` and `DeleteSecrets`
   succeed and keep nothing. Only calls from NetworkManager's current unique
   name are answered; one question waits at a time and expires after two
   minutes.

The Shell's existing iwd agent (per connection attempt, ADR 0137) is
unchanged. It remains scoped to one attempt and one daemon owner, which is the
same rule this agent follows for its owner check. The Shell's Wi-Fi panel does
not register a NetworkManager agent; for WPA-Enterprise it points to Settings.

Certificates are referenced by path (NetworkManager's `file://` scheme), so
NetworkManager reads them; Settings does not copy them.

## Consequences

- A connection marked "ask every time" asks only while Settings runs. When
  NetworkManager needs it otherwise (for example at login), the connection
  fails as it would without any agent. A resident session agent is not added;
  that would be a Canopa daemon between NetworkManager and the person.
- Plugin VPNs that need their own authentication dialog (for example
  OpenConnect SSO) get the plain secret fields their hints name; richer
  dialogs are left to the plugin's editor.
- Importing an OpenVPN or other plugin file uses `nmcli connection import`,
  NetworkManager's own tool, because the D-Bus API has no import call; the
  installed plugin parses the file. WireGuard `.conf` files are parsed by
  Canopa and added with `AddConnection`, as NetworkManager itself does.

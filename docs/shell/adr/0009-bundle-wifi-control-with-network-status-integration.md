# Bundle Wi-Fi Control with Network Status Integration

Niribar keeps network status and provider-owned Wi-Fi control aligned to one backend bundle. When NetworkManager is available, the network status provider and **Wi-Fi Control Integration** use NetworkManager; otherwise systemd-networkd may provide status while iwd provides Wi-Fi control, and without iwd the network module remains status-only. Niribar does not mix NetworkManager status with iwd Wi-Fi control, because two network managers can otherwise race over the same Wi-Fi device.

ADR-0010 moves rich Wi-Fi interaction into an **External Panel Provider**. This decision still governs which backend bundle that provider should use when it is launched from Niribar's network module.


## Live ownership and provider boundaries (2026-10-01)

The headless network bundle (`canopa_services::network::bundle` since
[ADR 0137](../../adr/0137-shared-system-service-clients.md)) is the authority for bundle selection
in both Shell status and the shipped Wi-Fi provider. A unique NetworkManager
owner supplies both status and control. Otherwise networkd may supply status
and iwd may supply control. A standalone iwd installation still supports direct
Wi-Fi provider use with unavailable overall network status. Without a control
owner the panel displays unavailable state and keeps observing ownership.

Both consumers subscribe before discovery and switch when relevant D-Bus owners
change, including daemon appearance after startup. Proxies address the selected
unique owner. No-control panels recover without relaunching, and open credential
prompts and in-flight work are retired when their selected control owner changes.
The provider's optional backend environment override restricts the shared choice;
forcing iwd cannot pair iwd controls with NetworkManager status. Disabling controls
leaves status observation intact. This policy is shared in code and does not add
backend-specific fields to the provider wire protocol.

A Wi-Fi selection is an opaque device/network handle bound to the bus, daemon
owner, device and network fingerprint. NetworkManager uses raw SSID bytes and
security flags; iwd uses its network Name and Type on the selected station.
Object paths are only routing information. Same-identity access-point roaming
preserves selection; a reused path with changed identity requires reselection.
Adapters revalidate before acting. Credential prompts are cleared when their
selection disappears; an iwd agent revalidates owner, station, name and type on
every secret request. Profile and agent cleanup targets the original unique
owner, even after the well-known service name changes hands.

Private bus fixtures cover owner changes, old and newly opened panels, pending
activation cancellation, path reuse, roaming, prompt invalidation and agent
callbacks. Status fixtures cover networkd/NetworkManager appearance, fallback,
complete absence and recovery on the same subscription.

# Bundle Wi-Fi Control with Network Status Integration

Niribar keeps network status and provider-owned Wi-Fi control aligned to one backend bundle. When NetworkManager is available, the network status provider and **Wi-Fi Control Integration** use NetworkManager; otherwise systemd-networkd may provide status while iwd provides Wi-Fi control, and without iwd the network module remains status-only. Niribar does not mix NetworkManager status with iwd Wi-Fi control, because two network managers can otherwise race over the same Wi-Fi device.

ADR-0010 moves rich Wi-Fi interaction into an **External Panel Provider**. This decision still governs which backend bundle that provider should use when it is launched from Niribar's network module.

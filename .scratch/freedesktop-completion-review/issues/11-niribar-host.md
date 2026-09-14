# W11: Port Canopa protocol v3 to the confirmed Niribar host; preserve existing framed-v1 providers.

Status: In progress

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W11. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-14: Opened the unchanged Canopa-v3 Niribar host (`b5d1b54`) on a nested Niri compositor inside headless Sway with private state and a non-activating D-Bus session. The provider panel displays Canopa content; screenshots and environment details are in [BANNER-LAYOUT-EVIDENCE.md](../BANNER-LAYOUT-EVIDENCE.md). Complete Center rich-content/action scrolling, reliable graphical keyboard selection and full-session accessibility remain unverified. This smoke check does not enable `body-images` or close W11.

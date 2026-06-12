# External Panel Providers Use Declarative Content

Niribar supports rich domain-specific panel interactions through out-of-process **External Panel Providers** rather than in-process plugins or module-owned control code in core. Providers own domain logic and send constrained declarative **External Panel Content** to Niribar; Niribar owns rendering, styling, placement, focus, sizing, and user-interaction routing for the active **Panel**.

This keeps crash isolation, dependency ownership, and backend permissions outside the bar while preserving a consistent panel/menu experience. It also avoids arbitrary provider-owned widgets or markup, which would make theme integration, layout, security, and Wayland focus behavior harder to control.

For the first provider protocol, Niribar launches providers on demand and communicates with them using framed JSON over standard input and standard output. That avoids provider registration, socket discovery, and separate authentication while leaving room for a later persistent-provider transport if the need becomes real.

The provider platform should be implemented before integrating domain-specific providers such as Wi-Fi or audio, so those providers validate the shared contract instead of shaping core panel code around one domain.

# Model Interactive Overlays as Foreground

Niribar models the currently active interactive overlay as **Foreground**: either no overlay, a **Menu**, or a **Panel**. The **Foreground** owns its physical overlay window identity together with its interaction state, so stale close handling, replacement, keyboard focus, and selectable menu behavior are local to one module instead of being coordinated between **Panel Runtime**, **Menu** popup helpers, overlay lifecycle helpers, and the **Bar Surface Registry**.

This supersedes ADR-0007's split between logical **Panel Lifecycle** and overlay-window lifecycle for future architecture work. That split was useful while only **Panels** needed rich interaction state, but it preserved the architectural loop once **SNI Menus** and **Panel Menus** needed shared **Selectable Menu Session** behavior. The **Bar Surface Registry** should narrow back to live **Bar Surface** records and passive **Tooltip** metadata; **Tooltip** is not **Foreground** because it is passive, non-interactive, and owned as a **Bar Surface** affordance.

Pending Menu replacements and installed-application discovery occupy one pending
opening slot. A newer opening or explicit dismissal retires that slot; delayed
completion or closure cannot reopen an obsolete overlay. Duplicate and unknown
Menu closures leave an active Panel intact. Panel Runtime owns the Panel tree and
menu render state, including an inert retiring presentation during animation;
Foreground owns the window and adapts lifecycle effects. Panel interactions use
one shared intent type rather than mirrored forwarding variants. Replacing a
dmenu Panel completes its responder exactly once before retiring its presentation.

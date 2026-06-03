# Separate Panel Lifecycle From Overlay Window Lifecycle

Niribar has repeatedly moved responsibility between **Panel Runtime**, a short-lived panel coordinator, overlay lifecycle helpers, and the **Bar Surface Registry**. That history showed that treating logical **Panel Lifecycle** and overlay window cleanup as one ownership problem creates an architectural loop: the next refactor naturally wants to move the same behavior back to the other side.

The **Panel Runtime** owns the logical **Panel Lifecycle**, including the current **Panel** render identity, **Panel Menu Session**, **Module Detail Session** state, pending search focus, and **Dmenu Selection Request** responder state. The **Bar Surface Registry** owns live **Bar Surface** records plus overlay window metadata, role classification, cleanup, and stale-window tracking for **Panel**, **Menu**, and **Tooltip** windows. The two are connected by an explicit **Panel Attachment**: an overlay window event can clear **Panel Runtime** state only when it still matches the current logical **Panel**.

`App` remains the Adapter for layer-shell and iced task execution, process launch, installed desktop application discovery, and physical IPC response sending. Future refactors should not resolve Panel/overlay friction by moving all **Panel** semantics into the **Bar Surface Registry** or by moving all overlay cleanup into **Panel Runtime**; they should deepen the **Panel Attachment** Seam instead.

The required regression tests are replacement-focused: replacing a **Panel** makes the old logical **Panel** gone immediately, a stale close from the old overlay window must not clear the new logical **Panel**, and an external close from the currently attached overlay window must clear the current **Panel Runtime** state.

# Use Named Bars for Output-Specific Behavior

Niribar models a **Bar** as a named configuration and a **Bar Surface** as the live surface created for a specific **Bar** on a specific **Output**. Output-specific behavior is expressed by defining another named **Bar** with an **Output Target**, rather than mutating a **Bar** through per-output overrides, because overrides blur the identity of the **Bar** that produced a **Bar Surface** and create two competing ways to express the same layout.

# Align iced/layer-shell dependencies with Niribar

Notify aligns its iced and layer-shell dependency source and version with Niribar where practical. Both projects depend on the same Wayland/layer-shell behavior, so avoiding version drift reduces rendering and event-loop surprises across the Canopa desktop tools.

# 0153. Keep SSD titlebars in the compositor

Status: accepted

## Context

Canopa already delegates window placement, corners, clipping, shadows and borders
to Niri. Its server-side decoration negotiation previously removed application
frames without providing titlebars. A shell-drawn overlay would need to duplicate
window geometry, animation, output scaling and input ownership, and would lose
basic window controls when the Shell restarts.

## Decision

The Niri fork owns titlebar layout, rendering and input for windows that negotiate
server-side decorations. Canopa enables the optional titlebars in its compositor
defaults and supplies their colors and font there. Applications that negotiate
client-side decorations keep their headerbars.

Titlebars reserve space above client content. They provide title text, dragging,
close and maximize/restore controls, including touch input. Maximized windows keep
the frame; fullscreen windows omit it. Basic controls operate without the Shell.

Do not add an unused generic titlebar extension protocol. A shell-hosted window
menu needs an explicit input handoff: a compositor click does not provide the
Shell with an input serial valid for grabbing one of its existing bar popups.
Add that interface with its first concrete consumer.

## Consequences

Frame geometry and hit testing stay together in Niri, at the cost of maintaining
a compositor patch. Colors follow compositor configuration; the first version
does not synchronize titlebar styling with live Canopa appearance changes.
Consistency applies to cooperating SSD windows, while integrated application
headerbars retain their own appearance and controls.

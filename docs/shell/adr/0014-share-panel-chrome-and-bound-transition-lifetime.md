# Share Panel Chrome and Bound Transition Lifetime

Drawer and automatic Panel presentation share titles, close controls, body spacing,
typography and menu-row treatment. `canopa-ui::panel` owns the visual roles and
metrics. Niribar owns content, placement, input and lifetime. Exact dimensions,
dimension caps, edge-fill sizing and drawer overlay/push-bar behavior are retained;
content-preferred sizes include the common chrome.

The per-Bar animation configuration drives an 8-pixel directional slide and fade
(180 ms entry, 160 ms exit by default). Entry decelerates with a cubic ease-out;
exit uses smoothstep so both movement and opacity taper before window removal.
The original 12-pixel, 120 ms accelerating exit looked abrupt: it still had 19%
opacity in its final tenth. The shorter travel and gentler exit apply equally to
drawers and popovers. A zero duration disables that transition. This is a per-Bar
configuration, not automatic detection of a desktop reduced-motion preference.

Extending ADR 0008, Foreground can retain one inert outgoing presentation after
logical close. IPC responses, provider cleanup and keyboard ownership follow the
logical close immediately. The outgoing window has an empty pointer-input region,
no keyboard interactivity, no widget operations or accessibility actions. A redraw
completion or a one-shot timeout removes it. Both identify the window; stale
completion cannot remove new content. Replacement and origin removal discard it.

Iced's renderer has no group-opacity primitive. A root widget therefore captures
the panel once per transition at the window's scale and presents that image with
translation and opacity. The live widget tree is retained for focus and scrolling;
resting panels render directly and release the capture. Captures are capped at
8,388,608 pixels (32 MiB of RGBA); larger surfaces render directly without motion. GPU
working allocations are additional. This avoids a new renderer fork and per-frame
surface resizing, at the cost of a synchronous capture on transition start.

Mouse/touch hit testing uses the inverse transform of the last displayed frame.
Typing and scrolling settle entry immediately, and pointer release settles it
after dispatching to the visible control. In-place provider updates also settle
entry so new controls cannot sit behind an obsolete bitmap. Frames are requested
only while transitioning; no permanent animation subscription is introduced.

The private desktop harness in `scripts/verify-panel-motion.py` warms the software renderer, then samples deliberately
slow transitions on every edge, disabled motion, rapid reopening, immediate IPC
cancellation and idle CPU. The existing drawer and scrolling harnesses cover normal
timings, placement, click-through space and fixed-pitch virtualized lists. Software
compositor captures establish native behavior, not hardware frame-rate guarantees.

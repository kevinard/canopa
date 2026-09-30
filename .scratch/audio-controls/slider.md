# Volume sliders

Status: ready-for-agent

Replace Sound's volume decrease/increase buttons with a 0–100% slider for every
output and microphone. Keep mute/unmute and default-device selection. Show the
loudest channel's percentage consistently with the control, preserve channel
proportions and mute, and support external updates and unavailable devices.

The shell renders a generic, optional rich-item slider with bounded integer values
and uses existing opaque interactions. Providers retain domain ownership. Missing
slider fields preserve existing wire output; v1 and existing v4/v5 clients remain
supported. Invalid ranges, unadvertised values, stale generations/revisions and
replaced devices must not execute changes.

Dragging previews locally and commits the exact final value on release. Keyboard
arrows, Home/End and native accessibility value/increment/decrement actions work.
Content replacement cancels an unfinished gesture instead of applying it to a new
contract. Focus and visual styling follow existing host behavior. No vendor changes.

Validate protocol bounds/compatibility, actual Pulse volume/mute/channel balance,
provider transport, host pointer/keyboard replacement behavior and native slider
accessibility. Run workspace checks and an isolated graphical acceptance fixture.

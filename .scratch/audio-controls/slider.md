# Volume sliders

Status: done

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

## Delivered

Optional rich-item sliders now replace Sound's volume buttons for outputs and
microphones. Pointer/key gestures preserve generation identity; quick consecutive
commits preserve the latest submitted value. The provider queues validated commands
and coalesces adjacent absolute volume changes without changing mute/default order.
Logical focus survives replacements without scrolling back to an earlier selection.

## Validation

- `bash scripts/check.sh` passed, including workspace tests, clippy, compatibility,
  updater, configuration, portal and vendor checks.
- Final shell `cargo test --locked -p canopa-shell --all-targets -- --test-threads=2`
  passed; the initial unrestricted rerun hit an existing installed-app 50 ms timing
  assertion under build load. Slider, stale-gesture, scrolling and real Pulse
  rapid-command regressions pass.
- Final shell clippy, formatting and `git diff --check` passed.
- Standards and spec reviews have no remaining findings after corrections.
- Private Sway/AT-SPI/PipeWire acceptance passed: native value setting, keyboard
  arrows/endpoints, pointer preview without audio writes, final release, mute,
  microphone control, focus reveal and Escape. Screenshots and the result are in
  `/tmp/canopa-slider-desktop/`; harness in `/tmp/canopa-slider-desktop.py`.
- Physical hardware and native PulseAudio hardware behavior remain desktop
  acceptance work. The normal updater installs both host and provider.

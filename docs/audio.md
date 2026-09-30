# Sound controls

Click the volume module, or activate it from the keyboard, to open **Sound**.
The panel lists speakers/headphones and microphones with their volume, mute state
and current default marker. Output monitor sources are excluded from microphone
choices.

- **Volume slider** selects 0–100% in one-point steps. The thumb and percentage
  preview your choice while dragging; releasing applies it. Tab to the slider and
  use arrow keys or Home/End, or use your screen reader's value controls.
  The level represents the loudest channel. Changes preserve existing nonzero
  channel proportions and do not automatically unmute. Externally amplified
  devices appear at the slider's 100% limit and remain untouched until adjusted.
- **Mute / Unmute** explicitly changes that device's mute state.
- **Use this device** requests the default output or input. Existing application
  streams follow the audio session manager's routing policy; Canopa does not move
  them individually.

The panel follows external changes and device removal. It shows an unavailable
message while reconnecting after audio-server loss. Failed operations display an
error. A provider update during an unfinished drag or held key cancels that gesture.
Keyboard focus and scroll position survive live updates; Tab and the panel's
existing keyboard navigation can reach each action, and Escape closes.

Both the bar and panel work with PulseAudio or PipeWire's PulseAudio compatibility
service. Install `canopa-audio-provider` beside `canopa-shell` or on PATH. No `pactl`
or `wpctl` executable is required for production controls. Microphone discovery
reads device information; it does not record audio.

Existing volume `actions { click { open-panel; } }` bindings open Sound. Explicit
custom commands or other actions still take precedence. To open Sound from another
module or a named menu, use `open-provider "audio"`; no provider declaration is
required. A configured provider named `audio` overrides the built-in.

This milestone covers devices. Application mixing, audio-card profiles, connector
selection remain future work.

## Validation

The library's `audio` tests use private PipeWire/Pulse servers with synthetic
outputs and remapped inputs. They cover speaker and microphone volume/mute,
absolute slider levels, channel proportions, rapid consecutive changes, default
requests, external changes, monitor exclusion, removal, reconnection and stale
actions through the v5 provider transport.
The fixture supplies active-default metadata because it has no session manager.
An isolated Sway/AT-SPI acceptance run also verified opening Sound from the volume
module, native slider value setting, arrow/Home/End input, pointer preview and
release, mute, microphone adjustment, focus revealing a clipped slider, and Escape
dismissal with synthetic speakers and a microphone.
Physical devices and native PulseAudio hardware behavior require desktop acceptance.

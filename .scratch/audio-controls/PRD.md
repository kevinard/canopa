# Audio device controls

Status: ready-for-agent

## Outcome

Opening Sound from the volume module lets the user inspect speakers/headphones and
microphones, adjust volume, mute/unmute, and choose the default output or input.
The user selected device controls first; application mixing is outside this milestone.

## Acceptance

- A built-in `audio` panel provider uses the existing `canopa-v5` rich contract and
  the shell's accessible buttons. Each device shows its name, volume, mute state,
  default status, volume decrease/increase controls, and an explicit default action.
- Volume adjustments preserve channel balance, stop at 0/100%, and never unmute
  implicitly. Microphone monitor sources are excluded from the input list.
- The volume module opens Sound by default; explicit user actions take precedence.
  Existing `open-panel` volume bindings open Sound as well. Scroll adjustment of
  the current default output continues to work.
- Status and controls use libpulse with the same server selection, supporting
  PulseAudio and PipeWire's PulseAudio compatibility service without CLI helpers.
- The panel reflects external changes and device removal, reports failed operations,
  and recovers after audio-server loss. A stale action cannot redirect to a new
  default or a device from a restarted server.
- Closing the panel retires its provider and audio observation. Audio volume/mute
  changes are deliberate user actions; discovery never records microphone audio.
- Release archives and the local updater include the provider. Documentation
  describes controls, configuration, default-device semantics and validation limits.

## Validation

Extend the existing private Pulse-protocol fixture with synthetic outputs and inputs
to cover control, external updates, default selection, missing devices and restart.
Exercise provider publication, stale action rejection and close through the actual
v5 transport. Check shell routing and existing volume behavior. Run the workspace
validation entry point and review the implementation against this specification.

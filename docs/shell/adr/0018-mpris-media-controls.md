# MPRIS media selection shared by the bar and panel

The `media` Module observes running MPRIS 2 players on the session bus. One Module
Data Source owns discovery, current-player selection and control dispatch for all
Media Module Instances. It exists only while the module is configured. Existing
players own media playback, decoding, streams, queues and libraries.

The `canopa-media-provider` process owns the Media panel's declarative v5 content
and interaction revisions, following [ADR 0010](0010-external-panel-providers-use-declarative-content.md).
It uses existing rich items, actions and a player selection control. No media-specific
host widget or panel protocol extension is added.

Unlike audio device controls, selecting a media player must update both the bar and
panel. The Module Data Source therefore exports the private session-bus interface
`org.canopa.Shell.Media` at `/org/canopa/Shell/Media`. `Snapshot`, `Changed` and
`Apply` exchange a bounded JSON snapshot or request using types shared inside the
shell library. The panel connects to the shell's exact bus owner and reconnects
after ownership changes. This introduces no separate daemon or persisted selection.

Automatic selection prefers Playing, then Paused, then Stopped, retaining the
current player among ties. Initial ties use the bus name. Manual selection pins a
live identity and falls back to Automatic when it disappears. Discovery installs
property and ownership signal matches before listing players; updates are signal
driven, with no periodic playback polling. Reconnection clears the old snapshot
and selection. Multiple module instances share this one selection.

Control requests capture the player bus name, unique owner, bus-server GUID and
track identifier. The worker re-reads that player's state and capabilities before
dispatch and calls the unique owner, never a replacement owner. The panel retires
old interaction revisions on content changes. Missing optional metadata has a
player-name fallback. Metadata is plain text and bounded; discovery admits at most
64 players, with bounded concurrent reads, command queues and request deadlines.

The source and provider futures own their connections and signal subscriptions;
canceling them releases that work. The provider preserves partially read host
frames across metadata updates. Errors are shown in the panel, and controls appear
only when the player advertises the relevant MPRIS capability.

The first version omits artwork, seeking, queue management, shuffle/repeat and
per-player volume. These can be added when needed without making the shell a
media player. See [the user guide](../../media.md) and the
[MPRIS player interface](https://specifications.freedesktop.org/mpris/latest/Player_Interface.html).

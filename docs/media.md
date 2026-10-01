# Media controls

Canopa shows and controls existing players through [MPRIS 2](https://specifications.freedesktop.org/mpris/latest/).
The player remains responsible for playback. No media library, decoder or audio
stream runs in the shell.

Add `"media"` to a bar's module placement to enable it. It works without an explicit
module declaration. For example, inside the `shell` block:

```kdl
bar "main" {
    edge "top"
    modules {
        start "workspaces"
        center "media"
        end "notifications" "volume" "clock"
    }
}
```

The horizontal bar shows a playback icon and a compact artist/title label, limited
to 48 characters. The tooltip shows the full label and player. A collapsed vertical
bar shows the icon; its expanded view shows the label. Click or activate the module
from the keyboard to open **Media**, with the title, artist, player name, playback
state and supported Previous, Play/Pause and Next controls.

**Automatic** prefers playing players, then paused players, then stopped players.
It keeps the current choice among equally active players; the initial tie is resolved
by MPRIS bus name. Selecting a player in the panel pins that live player in both the
bar and panel. The pin lasts until you choose Automatic, the player disappears, or
the shell's media connection restarts. Paused and stopped players remain accessible;
with no players the bar module hides. Only running MPRIS players are discovered.

The panel follows metadata and capability changes. Unsupported controls are omitted.
Failed operations show an error. If a player exits or the track changes before a
control is handled, the operation is rejected rather than sent to a replacement
player. The panel reconnects when the shell's media connection returns. Closing the
panel leaves playback and the bar's selection intact.

Optional formatting uses the standard module templates and content rotation:

```kdl
modules {
    module "media" {
        type "media"
        horizontal-format "{icon}" "{value}"
        expanded-format "{title}" "{artist}"
        tooltip "{label}" "{player}" "{state}"
    }
}
```

Placeholders are `icon`, `value` (compact label), `label` (full label), `title`,
`artist`, `player`, and `state` (`playing`, `paused`, `stopped`). Those state names
also work in conditional format segments. Metadata is displayed as plain text.

Install `canopa-media-provider` beside `canopa-shell` or on PATH. It is included in
workspace builds, release bundles and the updater. The built-in `media` provider
uses the shared panel protocol. Explicit module actions remain authoritative;
`open-panel` on a media module opens Media. A custom provider named `media`
overrides the built-in. `open-provider "media"` can also open it from another
module or menu while a media module is configured.

The initial controls cover previous, play/pause and next. Artwork, seeking,
playlists, shuffle/repeat and per-player volume are outside this version.

## Validation

The media tests use a private D-Bus daemon and synthetic MPRIS players. They cover
discovery, metadata updates, shared selection, supported controls, player replacement,
stale track actions, panel revisions and clean provider shutdown. Configuration
tests cover named instances, templates and default panel activation.

Desktop acceptance on 2026-10-01 used a private Sway session and a synthetic player.
Pointer activation opened the panel; Previous, Pause, Play and Next reached the
player and updated the bar and panel. Manual selection, return to Automatic and
panel dismissal were also checked.

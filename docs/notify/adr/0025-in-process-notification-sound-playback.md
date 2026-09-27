# In-process notification sound playback

Canopa Notify plays notification sounds through an in-process Rust audio backend and a Canopa-owned freedesktop sound theme resolver rather than shelling out to tools such as `canberra-gtk-play`. This keeps sound playback inside the daemon's adapter model and avoids process-spawn dependencies, while preserving sender-facing `sound-file`, `sound-name`, `suppress-sound`, and default sound behavior.

**Consequences**

Canopa Notify owns XDG sound theme lookup, theme inheritance, local audio file resolution, and best-effort playback diagnostics. The audio backend remains replaceable behind the sound adapter, but the daemon must not depend on external command-line players for normal notification sound behavior.

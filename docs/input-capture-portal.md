# InputCapture portal

Canopa implements experimental InputCapture v2 with Rust session ownership and
Iced approval. It is exported by the normal backend and can be selected through
the [optional companion package](desktop-sharing.md) with `--extra-portals`.
The keyboard-capable companion advertises keyboard and pointer capture. The
preceding pointer companion remains supported; touch capture is unavailable.

Applications request a session and explicit approval before enabling capture.
The separate Iced keyboard and pointer choices start unchecked. Create alone opens no capture lease.
The compatible Niri compositor owns capture barriers and activation; the
application receives a session-owned EI connection. Releasing or disabling
capture clears held input before returning local control. Topology changes
invalidate old zones and barriers, requiring the application to obtain current
zones before enabling again. The session's Iced Stop control retires its authority.

Clipboard is an independent, initially unchecked permission. When granted to an
InputCapture session, it is usable only while capture is actually active.
Deactivation, Disable, stale topology, Stop, caller loss and backend loss revoke
active clipboard transfers. Repeated Enable or a rejected stale Release preserve
the real capture phase rather than fabricating an inactive state.

The companion frontend supplies the v2 public interface and legacy create
translation, ownership tracking and signal-confirmed phase updates. It must be
selected together with the compatible Niri recipe. See the
[session decision](adr/0061-input-capture-sessions-and-consent.md),
[active clipboard decision](adr/0062-input-capture-clipboard-activation.md) and
[frontend phase decision](adr/0063-input-capture-frontend-phase.md).

The maintained native check uses a private bus, nested compositor, real Iced
approval and synthetic pointer input:

```sh
python3 scripts/test-input-capture-portal.py --clipboard \
  --binary BACKEND --binary-sha256 BACKEND_SHA256 \
  --niri NIRI --niri-sha256 NIRI_SHA256 \
  --frontend FRONTEND --frontend-sha256 FRONTEND_SHA256 \
  --portal-descriptor BUNDLE/share/xdg-desktop-portal/portals/canopa.portal \
  --output target/input-capture-check
```

This verifies activation, release, topology changes, clipboard transfer and
revocation on the selected stack. Physical input, real sandbox clients and wider
distribution acceptance remain separate from this scoped verification.

The `niri-input-capture-keyboard` companion and normal backend also pass the
public keyboard check in both appearances. Use the command above with
`--keyboard` instead of `--clipboard`. It verifies independent keyboard/pointer
approval, immutable keymap FDs, ordered key/modifier events, held-key cleanup,
keymap device replacement and source withdrawal. Captured release tails and late
modifier packets remain owned by the retired activation. A new activation cannot
adopt them. The ordinary client is checked independently, including after source
withdrawal. See the [keyboard decision](adr/0111-input-capture-keyboard-ownership.md).

These are scoped synthetic-input checks. Development packages can include this
keyboard-capable companion through the explicit extra-portals option. Building,
packaging and verification do not change the installed compositor or portal selection.

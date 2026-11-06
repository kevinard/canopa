# Canopa Account portal

The Rust/Iced Account backend presents a profile-sharing draft for each request.
The user can edit or exclude their username, name and image before choosing Share.
Opening the dialog does not return profile information to the requesting app.

## Build and select

```sh
cargo build --locked -p canopa-portal
```

Use the default desktop features. Follow the [installation guide](settings-portal.md#installation-and-backend-selection)
for the binary, D-Bus activation service, user unit and portal descriptor. Add the
following entry to the existing `[preferred]` group in the session's selected
portal configuration:

```ini
org.freedesktop.impl.portal.Account=canopa
```

For Niri, this is normally `~/.config/xdg-desktop-portal/niri-portals.conf`. Portal
configuration files are selected as a whole. Preserve the session's existing
entries for other interfaces when adding Account. These build and test commands
do not install services or change the live desktop's portal routing.

## Sharing behavior

The helper reads the current UID's AccountsService profile on a worker, with a
two-second service timeout and an NSS fallback for missing identity fields.
When AccountsService is unavailable, it also checks the user's `.face` and
`.face.icon`. Profile reads do not modify the system account. Each helper uses
Canopa's shared appearance as it was when the dialog opened.

- Share returns the reviewed fields. An excluded field becomes an empty string;
  a successful result always contains `id`, `name` and `image` strings.
- Choose image opens the existing Files-based picker inside the same window.
  Cancel or Escape in the picker returns to the retained sharing draft.
- Clear image changes the draft. It does not remove the original profile image.
- Initial Enter does not share. Cancel, Escape in the profile, window close,
  Request.Close and caller loss return no shared information.
- Concurrent requests have independent drafts and share the backend's bounded
  admission pool with the other dialog interfaces.

Local PNG, JPEG, GIF, WebP and SVG images are prepared as bounded, owned pixels.
The preview and exported PNG use those same pixels, with source metadata and
external-resource dependencies removed. Animated formats use their decoded
static frame. Selecting or previewing an image does not publish it.

Only Share publishes an image into the private
`$XDG_CACHE_HOME/canopa/portal/account/` directory, with the usual `~/.cache`
fallback. The returned URI stays readable after the helper exits or the original
source image is removed. Files have private permissions and content-addressed
names. The cache holds at most 64 generated images and 16 MiB; recently published
or reused images are protected for at least 60 seconds before pressure eviction.
This is a bounded cache rather than permanent image storage. If publication
fails, the draft remains open for retry or sharing without its image.

The standard XDG frontend retains application identification and sandbox document
export. Backend native checks cover host frontend requests; actual Flatpak/FUSE
image export requires separate acceptance in a sandbox-capable environment.
Foreign Wayland parenting is shared with FileChooser. Optional compositor modal
hints have the same availability limits described in the [chooser guide](file-chooser-portal.md).

## Validation

```sh
cargo test --locked -p canopa-portal --lib --test integration
cargo check --locked -p canopa-portal --no-default-features --all-targets
python3 scripts/test-account-portal.py --binary target/debug/xdg-desktop-portal-canopa \
  --output /tmp/canopa-account-dark --shipped-descriptor
python3 scripts/test-account-portal.py --binary target/debug/xdg-desktop-portal-canopa \
  --output /tmp/canopa-account-light --color-scheme light --scale 1.25 --shipped-descriptor
```

The native fixture supplies its own AccountsService identity, profile images,
private bus, public portal frontend and nested Niri compositor. It uses no host
account service. Dependencies match the [FileChooser native fixture](file-chooser-portal.md#validation).
Results, captures and traces remain in the new output directory. See
[ADR0033](adr/0033-account-sharing-draft.md) and the
[implementation ledger](../.scratch/xdg-portals/IMPLEMENTATION.md) for scoped
acceptance and broader validation results.

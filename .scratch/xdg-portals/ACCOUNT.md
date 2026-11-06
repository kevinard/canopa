# Account backend preparation

Status: implemented; focused/native/shipped-activation acceptance passes,
2026-10-29. Actual sandbox export remains unverified.

Account is the sixth implementation slice. The initial contract and design
investigation below is retained as its source evidence. The implementation uses
the request host, worker profile lookup, embedded picker and private PNG cache
described in [ADR0033](../../docs/adr/0033-account-sharing-draft.md). No installed
routing or live services have been changed.

## Contract

The [backend contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.Account.html)
has one `GetUserInformation(o handle, s app_id, s window, a{sv} options)` method,
returning a response code and results dictionary. `reason` is optional explanatory
text. Successful results contain string `id`, `name` and `image` (an avatar file
URI). The [public interface](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Account.html)
is version 1 and uses the usual asynchronous Request response. The installed XMLs
under `/usr/share/dbus-1/interfaces/` agree with that shape. No backend version
property is specified by its XML.

The [GNOME backend](https://github.com/GNOME/xdg-desktop-portal-gnome/blob/main/src/account.c)
reads the current UID's AccountsService user properties. Its
[dialog](https://github.com/GNOME/xdg-desktop-portal-gnome/blob/main/src/accountdialog.c)
lets the user edit the shared login/real names, replace or clear the avatar, and
accept or cancel. This is reference behavior, not permission to copy GTK code or
add a GTK dependency. Personal-data sharing needs an explicit decision.

## Canopa seams to resolve

- Reuse `Requests` for bounded admission, owner-authorized Close and caller-loss
  retirement, and the ordinary Iced helper for parenting/appearance. Keep system
  profile discovery and asset preparation outside the UI thread and main bus
  handler. Reuse the application catalog for readable requester attribution.
- Account data belongs to this request's editable sharing draft. Do not change
  the user's OS account, persist disclosure permissions, or transmit fields before
  acceptance. Cancellation/error results must contain no personal information.
  Initial Enter must not silently disclose a profile.
- Investigate a bounded AccountsService lookup with a deliberate fallback when
  it is absent. Do not infer account identity from spoofable USER/LOGNAME alone.
  NSS-backed discovery is available through the already locked `whoami` crate;
  its locked Linux source uses `getpwuid_r(geteuid())`, returns errors for absent
  or empty records and bounds its NSS buffer to 16 KiB. The fallible API still
  needs explicit field defaults and an admitted worker. No AccountsService
  interface XML was found locally in the initial static inventory.
- Avatar selection should reuse local FileChooser browsing rather than another
  platform chooser. Preview decoding uses shared caller-bounded raster/SVG
  mechanisms. Decide how the accepted image retains the exact reviewed bytes
  long enough for the public frontend to export/open it; helper shutdown must not
  delete it prematurely. Do not claim Flatpak document export without an actual
  sandbox fixture.
- Share no avatar when cleared or unusable. Render caller reason as plain text,
  keep cancellation reachable with long content and narrow/scaled windows, and
  bound edited fields and helper output.

## Acceptance

Wire/model tests must cover the complete shape, malformed options/results,
cancel/Close/owner loss, request capacity and independent concurrent drafts.
Profile tests need deterministic providers, absent/failed AccountsService, edited
names, avatar removal and bounded invalid/oversized media. A private public-
frontend native fixture should prove explicit accept/cancel, initial Enter,
keyboard edits, avatar selection/clear, parent association and image readability
after the helper exits in both appearances. Use only a fixture-owned profile and
image; do not assert or record the human user's account data in screenshots.


## Frontend avatar handoff verified

The current [Account frontend source](https://github.com/flatpak/xdg-desktop-portal/blob/main/desktop-portal/account.c)
is under `desktop-portal/`, after the repository layout change. It was read from
the official raw source after the web reader returned a cache miss. Host callers
receive the backend's image URI unchanged; sandbox callers go through document
registration before response. The frontend requires string id/name values when
accepting a successful result. Public reasons are limited to 256 Unicode
characters. Source SHA-256 checked on 2026-10-29: `6e7dc03d1659f2b3deaf8f1bc2092222d08b681960c78b0c3d279e3d435a7b02`.

This means helper-owned temporary avatar files cannot be removed on dialog exit.
The Canopa draft should retain its reviewed pixels, normalize an accepted avatar
to a bounded standalone raster without external resources/metadata, and publish
through a persistent caller-owned cache before returning its URI. Define cache
reuse, ownership and bounds deliberately; changing the user's original avatar is
outside this portal. Test both the public host URI after helper exit and actual
sandbox document export separately. The latter remains an environment-dependent
acceptance gap until a real Flatpak/FUSE fixture is available.


## Local avatar-picker integration

Prefer an embedded picker view using the existing FileChooser state/view within
the Account helper's own Iced window. Extract its completion boundary so the
standalone FileChooser adapter prints/exits, while an embedded caller receives
Selected/Cancelled and retains its sharing draft. Keep browsing and eligibility
logic in the same component. This avoids a nested request consuming another
shared slot, a second parenting/export protocol, or invoking whichever external
FileChooser the user's current routing happens to select.

The Account caller must tag each picker instance and ignore delayed messages from
an earlier instance; the picker's own directory revision alone cannot distinguish
close/reopen. Route keyboard subscriptions through the active view so Return in
the picker cannot also disclose the profile. Escape returns to the draft, while
closing the owning window cancels the Account request. Require current FileChooser
model/native regression after this local extraction, plus deterministic instance-
replacement and accepted-avatar lifetime tests. No shared vendor change is
expected from this seam. This seam is now implemented; see the implementation
ledger for its focused and native checks, including standalone chooser regression.

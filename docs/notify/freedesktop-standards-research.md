# Freedesktop notification standards

Reference notes on the Desktop Notifications Specification 1.3 and related freedesktop standards, with the reasoning behind ambiguous points. The outcome for each requirement is recorded in [the compliance matrix](freedesktop-notification-compliance.md). Researched in September 2026 against the published specifications linked below.

The current published **Desktop Notifications Specification is 1.3**, published 18 August 2024. Use its versioned pages for acceptance criteria; the moving `latest` alias currently resolves to the same version. The official index lists 1.3 and 1.2. [Version 1.3](https://specifications.freedesktop.org/notification/1.3/), [official specifications index](https://specifications.freedesktop.org/).

The specification uses ordinary “must”, “should”, “may”, and explicit optionality. Preserve those distinctions. A complete product can implement an ambitious optional feature profile, but “conforming” does not mean every hint or capability must exist. Define three independent acceptance tracks:

1. Desktop Notifications 1.3 core and the project's selected capability profile.
2. XDG Notification portal interoperability and, separately, a native portal backend if desired.
3. Product quality: maintainability, accessibility, resource limits, privacy, packaging, persistence reliability, and compatibility extensions.

The latter two tracks are planning recommendations. They must not silently change the meaning of the first.

Recommended target: a **complete static Desktop Notifications 1.3 implementation** with actions, action icons, body text/styles/hyperlinks/images, sound, and a deliberately specified persistence model. Select `icon-static`; do not promise `icon-multi` simultaneously. Build a complete standard-hint matrix, including explicitly ignored optional hints. Treat robust icon/sound resource resolution as part of delivering those supported capabilities. Portal interoperability is a companion release criterion; owning a native portal v2 backend is a separate scope decision.

## Core contract and checklist

The service is session-scoped: name/interface `org.freedesktop.Notifications`, object `/org/freedesktop/Notifications`. Bus activation is optional. IDs are session-unique and not recycled during daemon execution before uint32 exhaustion. Summary/body are UTF-8; summary length 40 is guidance, not a limit. Actions arrive as alternating identifier/label strings; the conventional default key is `default`. Expiration is measured from display: milliseconds, `-1` means server policy, `0` means no expiration. [Basic design, §2](https://specifications.freedesktop.org/notification/1.3/basic-design.html).

| Check | Required observable contract |
| --- | --- |
| Discovery | `GetCapabilities() → as`; `GetServerInformation() → ssss`. |
| Receipt | `Notify(susssasa{sv}i) → u`; returned ID nonzero. |
| Replacement | Existing notification changes atomically; nonzero replacement ID is returned unchanged. |
| Closure | `CloseNotification(u)` closes and signals; absent ID returns an empty error. |
| Closure event | `NotificationClosed(uu)`: reasons 1 expiration, 2 dismissal, 3 sender close, 4 other. Invalidate ID before emission. |
| Interaction | `ActionInvoked(us)` carries the supplied action key. Optional `ActivationToken(us)` precedes the action. |
| Capabilities | Advertise implemented behavior; `icon-static` and `icon-multi` are mutually exclusive. |
| Persistence | Retain until acknowledgement, user removal, or sender recall. |
| Sound | Advertising `sound` requires `sound-file` and `suppress-sound`. |

These protocol facts come from [D-Bus protocol, §9](https://specifications.freedesktop.org/notification/1.3/protocol.html).

The capability vocabulary also includes `actions`, `action-icons`, `body`, `body-markup`, `body-hyperlinks`, and `body-images`; animation is optional. [Capability table](https://specifications.freedesktop.org/notification/1.3/protocol.html).

Review implications: make each row a public-boundary test. Keep the complete D-Bus signature under introspection tests, and check the actual D-Bus error reply and empty error message rather than comparing only a Rust error enum. The notification specification does not unambiguously distinguish an absent error body from an empty string argument. Preserve notification content when optional metadata is unsupported. Use observed UI output to substantiate visual capabilities.

## Hints, urgency, and content

Hints are optional; unknown hints should be ignored. Standard types include boolean `action-icons`, `resident`, `transient`, `suppress-sound`; string `category`, `desktop-entry`, `image-path`, `sound-file`, `sound-name`; byte `urgency`; paired int32 `x`/`y`; and structured `image-data`. Deprecated spellings are `image_data`, `image_path`, and `icon_data`. `resident` prevents action invocation from removing the notification; `transient` bypasses persistence. Action icons use the action identifier as an icon name and retain the localized label for accessibility. The sound-name/file precedence is unspecified. [Hints, §8](https://specifications.freedesktop.org/notification/1.3/hints.html).

Urgency values are low `0`, normal `1`, critical `2`. Critical notifications **should** remain until dismissed; this is a recommendation, not an unconditional MUST. The urgency section does not resolve its interaction with an explicit positive timeout. [Urgency, §7](https://specifications.freedesktop.org/notification/1.3/urgency-levels.html).

Recommended default: critical notifications do not expire automatically, with any user-selected override documented as an intentional policy. This prioritizes the urgency recommendation; the tradeoff is that a sender's positive timeout may be ignored. If the product instead honors an explicit positive sender timeout, document that exception and retain a non-expiring server default for critical urgency. Test both configured and sender-provided timeout precedence.

Markup is an optional XML-based subset: bold, italic, underline, hyperlinks, and images. Unsupported tags should be filtered. Hyperlinks address standard URIs and are separate from notification actions; blue underlining is recommended. Body images reference local files only, never embedded payloads; alternative text is recommended, and 200×100 is the recommended maximum. Full HTML, CSS, and browser layout are outside this feature. [Markup, §4](https://specifications.freedesktop.org/notification/1.3/markup.html).

Recommended content acceptance fixtures: interleaved styles and images, XML entities, Unicode, unknown elements, malformed attributes, missing files, oversized images, nested links, and plain-text fallback. URI launching should require deliberate user interaction and use a maintained desktop opener without interpolating sender strings into shell commands. These robustness choices are engineering recommendations, not extra quoted specification requirements.

## Image selection and byte layout

For a single visual slot, the required preference is `image-data`, then `image-path`, then `app_icon`, then deprecated `icon_data`. When displaying image and icon separately, show `app_icon` as the icon and choose the image from the same order with `app_icon` omitted. The image structure is `(iiibiiay)`: width, height, rowstride, alpha flag, sample depth, channels, bytes. Sample depth is 8; channels are 4 with alpha and 3 otherwise; byte order is RGB(A). `app_icon` and `image-path` accept local file URIs or themed names. [Images, §5](https://specifications.freedesktop.org/notification/1.3/icons-and-images.html).

The specification explicitly derives this representation from GdkPixbuf. Its final row may omit stride padding. Therefore a valid minimum byte length at 8 bits is `(height - 1) * rowstride + width * channels`, with overflow-checked arithmetic and nonzero valid dimensions. Requiring `height * rowstride` rejects valid images. [GdkPixbuf image data documentation](https://docs.gtk.org/gdk-pixbuf/class.Pixbuf.html#image-data).

Recommended fixtures: padded RGB and RGBA, a one-pixel image, final-row padding absent/present, conflicting hints, invalid highest-priority images, legacy aliases, unusual rowstride, huge/negative dimensions, and overflowing products. The specification does not define whether invalid preferred input should block fallback; choose a consistent best-effort policy and encode it once in Notification Intake.

## Related theme specifications

For named icons, **Icon Theme 0.13** supplies inheritance, size/scale selection, and the required `hicolor` fallback. PNG and legacy XPM are supported formats; SVG is optional. Within the selected theme, a less exact size still wins over an inherited theme's closer size. Cache implementations must notice top-level directory changes, allowing a five-second check interval. [Icon Theme 0.13](https://specifications.freedesktop.org/icon-theme/0.13/).

For named sounds, **Sound Theme 0.2** defines theme inheritance and final `freedesktop` fallback, locale fallback, successive removal of hyphen suffixes, and output-profile fallback through `stereo`. File preference is `.disabled`, `.oga`, legacy `.ogg`, then `.wav`. [Sound lookup](https://specifications.freedesktop.org/sound-theme/0.2/sound_lookup.html).

A `.disabled` marker terminates lookup; it is not an ordinary missing asset that should fall through to another sound. Sound themes can span XDG data directories. [Sound directory layout](https://specifications.freedesktop.org/sound-theme/0.2/directory_layout.html).

Engineering recommendation: inspect the project's current resolvers against these related standards before deciding whether to deepen or replace them. Test small temporary theme trees rather than relying on the developer's installed desktop theme. Reuse a library only if its effective behavior satisfies the contract. Sound playback must remain in-process under [ADR 0025](adr/0025-in-process-notification-sound-playback.md).

## Portal: a separate interface family

The current public Notification portal documents **interface version 2**. Notifications can outlive their sending process. Exported `app.` actions activate the application through `org.freedesktop.Application`; other actions use portal signals. Version 2 introduces markup-body, FD-backed images/sounds, display hints, categories, and button purposes, with supported categories/purposes advertised separately. Its `persistent` display hint prevents ordinary user dismissal and is limited to applications with windows; it is different from Desktop Notifications `persistence`. [Public Notification portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Notification.html).

The backend interface is also version 2. It receives `(app_id, id, notification)`, removes by `(app_id, id)`, and emits action metadata with application identity. Its input shares the public notification format; version 2 backend icons never use the older byte-array form. `SupportedOptions` and `version` are properties. [Notification backend](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.Notification.html), [first-party backend XML](https://raw.githubusercontent.com/flatpak/xdg-desktop-portal/main/data/org.freedesktop.impl.portal.Notification.xml).

An existing GTK portal backend demonstrates an architectural option: it translates to the classic freedesktop notification daemon and handles `app.` activation itself. This demonstrates interoperability through a bridge, not proof that every v2 feature survives translation. [GTK backend source](https://github.com/flatpak/xdg-desktop-portal-gtk/blob/main/src/notification.c).

Recommended scope: require an isolated real-portal delivery/replace/remove/action test for the currently selected backend. Make direct implementation of the v2 backend a separately estimated milestone. Avoid claiming v2 completion merely because sending a title/body notification through an installed portal succeeds. The portal documentation URLs are moving references; pin the deployed frontend/backend package versions in test evidence.

## Ambiguities and deliberate policy choices

These points require explicit decisions rather than an invented normative answer:

- **Unknown/stale replacement IDs and sender ownership.** The nonzero replacement wording is broad, while closed IDs cannot be reused; unknown replacement behavior and cross-client authorization are not resolved. Preserve sender isolation and record the chosen compatibility behavior. [Protocol](https://specifications.freedesktop.org/notification/1.3/protocol.html).
- **Signal destination and immediate completion.** Notification 1.3 does not specify destination headers or Notify reply ordering. D-Bus supports unicast signals, which avoid the need for a recipient match rule and can limit dissemination. Recommend sender-directed events and a receipt-before-immediate-close contract, tested on a private bus. [D-Bus message routing](https://dbus.freedesktop.org/doc/dbus-specification.html#message-bus-routing).
- **Animated icons.** The current capability describes frames, but the image data contract provides no frame-array field or timing model. Keep static support as the documented profile; do not invent a portable animation protocol. [Capabilities](https://specifications.freedesktop.org/notification/1.3/protocol.html), [image structure](https://specifications.freedesktop.org/notification/1.3/icons-and-images.html).
- **Body markup fallback.** The markup section recommends server filtering; the capability description also tells clients to strip markup when unsupported. A tolerant server parser and preserved raw input are reasonable interoperability choices. [Markup](https://specifications.freedesktop.org/notification/1.3/markup.html), [capabilities](https://specifications.freedesktop.org/notification/1.3/protocol.html).
- **Policy and retention.** Decide how Do Not Disturb, drop rules, active Notification Center records, finite expiry, and `resident` interact with the chosen persistence claim. Archival bytes alone are inadequate evidence of actionable notification retention. [Persistence capability](https://specifications.freedesktop.org/notification/1.3/protocol.html).
- **Protocol omissions.** Values below `-1` for expiry, malformed action pairs, malformed known hints, and rejected visual resources lack a complete error/fallback policy. Prefer documented, bounded behavior with compatibility fixtures.

Repository alignment: [ADR 0028](adr/0028-notification-timeouts-begin-with-banner-display.md) already chooses display-based timers and keeps never-displayed records active. [ADR 0004](adr/0004-persistent-local-notification-history.md) makes transient retention sticky across replacements; that is a product decision requiring explicit fixtures, not a rule stated in the standard. Any change should amend the ADR. [ADR 0026](adr/0026-ordered-body-segments-for-body-images.md) already requires end-to-end rendering before advertising body images. [ADR 0009](adr/0009-freedesktop-notification-api-first.md) keeps Dunst-specific compatibility separate.

## Recommended definition of complete

Completion should be a checked release artifact, containing:

1. A versioned conformance matrix linking each core rule, selected capability, and supported hint to implementation ownership and verification evidence; exclusions explicitly explain optionality or ambiguity.
2. A private-session-bus contract suite that uses the built daemon, real wire types, replacement and lifetime sequences, actions, errors, capabilities, and concurrency. Tests must fail when the daemon cannot start, not silently pass after skipping.
3. A bounded, replayable visual interaction suite for Notification Banner and Notification Center content, including keyboard activation, hyperlinks, images, and activation-token ordering.
4. Theme fixtures, malformed-input coverage, resource budgets, and error-injection cases for unavailable sound/display/storage resources. Complete Notification Intake should remain responsive under those failures.
5. Explicit state invariants: external IDs are separated from durable thread identity; a completed sender lifecycle cannot reactivate accidentally; retained records and transient records obey their documented storage rules; obsolete revisions cannot expire or invoke current content.
6. A documented portal interoperability envelope with the actual tested frontend/backend versions and feature limitations.
7. Reproducible build/install/startup checks and a maintainable ownership map for adapters, Notification Runtime, Notification Lifecycle, asset resolution, and presentation.

This completion definition is an engineering proposal based on the contracts above. It does not require implementing every vendor hint, adding X11 support, or claiming undocumented animation semantics.

# 0010: Optional source attribution for rich panel items

Status: accepted (2026-09-29).

Center cards should identify their sender like notification banners: a leading
icon, message content and a quiet source label. Body images cannot serve as source
icons because they have different meaning, placement and accessibility semantics.

Rich `PanelItem` gains optional `attribution`: a nonempty label and an optional
`PanelIcon`. `PanelSection` gains an optional `icon` beside its title, so settings
can identify apps even when no message cards are displayed. Icons carry an absolute local path or up to two ordered icon names
with a theme and source scale. These are generic presentation fields. Notification
policy, application identity fallback and action meaning remain owned by Notify.

This is an additive exception to the frozen rich primitives described in ADRs
0006/0007. v4 and v5 retain their envelopes, versions, byte limits and interaction
rules. Existing item and section decoders ignore unknown fields; new decoders accept
absence of attribution/section icons and omit them when serializing old content. Older hosts keep their
existing text layout. Rust constructors add `attribution: None` / `icon: None` when unused.
Compatibility fixtures cover both directions and v5 validation.

Notify projects the stored app name/icon plus its current icon theme and scale.
It shares the banner identity fallback candidates, preserving an explicit icon
instead of replacing it with a guessed app icon. Settings group IDs remain canonical app
identities; human-readable names are presentation labels only. Source icons need no store or notification lifecycle change.

Niribar prepares icons on the existing admitted blocking worker using
`canopa-assets`. Icons use the banner PNG/JPEG lookup policy at nominal size 34,
with at most 16 distinct icon requests per snapshot, independent of the existing
16 body images. Icon batches retain the Center's 8 MiB file, 2,048 dimension and
32 MiB decoder-allocation limits, with output thumbnails bounded to 128 pixels.
Lookup metadata is shared per theme/scale within the snapshot; neither file work
nor decoding occurs during widget rendering. Missing, undecodable or unadmitted assets
fall back to an initial badge and visible source label. Replacement snapshots
prepare fresh pixels and cannot retain a previous item's source icon.

The host renders attribution without notification-specific keys. A section title
already repeated by every card's source label is omitted visually when the section
has no controls. The source label remains a measured native accessibility node;
its icon/badge is decorative. Section icons use the same asset budget and fallback;
the section title supplies their accessible identity. Empty section titles add no
visual heading, allowing a single chronological list. Body images, unread indicators, keyboard reading and
revision-bound actions retain their own semantics.

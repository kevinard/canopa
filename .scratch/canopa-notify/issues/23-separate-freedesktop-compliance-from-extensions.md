# Separate freedesktop 1.3 compliance from notification extensions

Status: implemented
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Make the compliance ledger and capability tests distinguish normative Desktop Notifications Specification 1.3 surfaces from widely implemented compatibility extensions and Canopa-specific extensions. Preserve deliberate interoperability behavior while avoiding claims that extension capabilities or hints are standard freedesktop 1.3 features.

## Acceptance criteria

- [ ] The compliance ledger has separate sections for specification-defined capabilities and hints, compatibility extensions, and Canopa extensions.
- [ ] `ActivationToken` is documented as a standard signal while any `activation-token` capability advertisement is explicitly classified as an extension.
- [ ] `inline-reply` is not presented as a freedesktop 1.3 capability.
- [ ] The `value` progress hint is documented as a de-facto extension rather than a standard 1.3 hint.
- [ ] Any retained unnamespaced extension has a documented compatibility reason; new Canopa-only names use an `x-canopa` namespace.
- [ ] Capability tests assert the exact normative and extension sets in headless, graphical, sound-enabled, and activation-token-capable modes.
- [ ] Manual compatibility documentation tells client authors which behavior is portable and which depends on extensions.

## Blocked by

None - can start immediately.

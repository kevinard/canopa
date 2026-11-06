# ADR0035: Iced launcher review and frontend publication ownership

Status: implemented and advertised; scoped wire/native acceptance passes, 2026-10-29.

## Context

DynamicLauncher prepares an installation by asking the user to approve its name
and serialized bytes icon. The standard frontend already owns the resulting
token and desktop-entry lifecycle. Copying that lifecycle into the backend would
create two competing publication and authorization owners.

## Decision

Implement the version-1 backend in Rust, using the existing request registry,
helper transport and foreign-parent Iced windows. Support application and web-app
review, editable or fixed names, explicit Install, and the standard cancellation
paths. Show the full target URI for web apps as plain text. Return the original
icon in the exact nested variant required by the frontend. Validate the helper's
chosen name again in the backend before approving it.

Keep icon editing read-only for this slice. It is optional in the contract, and
frontend 1.22.1 retains input format and size for its eventual icon writer. A
normalized replacement could contradict that metadata. Preserve original bytes
and prepare only small preview pixels using the existing Rust asset mechanisms.
Admit requests before extraction/decoding, and hold independent preparation
permits inside workers so cancelled requests cannot create unbounded decoders.

The frontend retains sender-scoped expiring tokens, desktop-entry and icon
publication, launch and uninstall. The backend creates no launcher files or token
store. Deny all non-interactive RequestInstallToken policy queries until Canopa
has an explicit trusted-installer policy. Do not inherit another desktop's app-ID
allowlist. An unidentified host process can bypass the backend policy query in
the installed frontend; document that boundary explicitly.

## Validation and limits

Model and wire tests cover options, complete serialized icon shapes, bounded
content, fixed-name enforcement, exact nested results and request ownership.
Native acceptance uses disposable registered host identities, a real frontend,
private nested Niri and only a recording launcher. It must verify frontend token
scope/replay, original image bytes and actual install/read/launch/uninstall in
both appearances before advertising the interface. Eight native groups now pass
with the shipped descriptor and activation service in dark/100% and light/125%.
The portal suite passes 56 unit and seven integration tests; the deliberately
separate native observer case remains excluded from that command.

The [user guide](../dynamic-launcher-portal.md) records resource limits, optional
compositor modality and the separate real-Flatpak acceptance environment. Actual
results, including failed attempts, belong in
`.scratch/xdg-portals/IMPLEMENTATION.md`; focused success is not a workspace gate.

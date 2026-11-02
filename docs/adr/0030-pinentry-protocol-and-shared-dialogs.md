# 0030: Pinentry protocol and shared dialogs

Status: implemented; validation is recorded in `.scratch/pinentry/PRD.md`.

Canopa supplies an on-demand pinentry executable for GnuPG. The public Assuan
pipe protocol is implemented in Rust from its documented interface; no GnuPG
frontend implementation is copied or linked. GnuPG owns key material, credential
verification, policy and agent caching. There is no pinentry daemon, PAM stack,
external password manager or systemd service.

`canopa-pinentry` reuses `canopa-session-ui` appearance, layout, presentation and
secret input. Shared presentation adds an optional hidden response field for
confirmation/message controls, and fixed-capacity secrets can be allocated at
2048-character UTF-8 capacity without reallocating sensitive bytes. Existing
consumers retain their default capacity and visible input behavior. Application
surface roles remain consumer-owned, as with the greeter and polkit agent.

The serial bounded protocol admits a single dialog at a time. Wayland connects
only for a prompt and disconnects on completion. A toplevel is an ordinary
compositor-managed window, identifiable as Canopa Pinentry even with caller titles.
The event loop redraws only on changes; timers and pipe disconnect readiness end
pending requests. Optional protocol features are rejected unless implemented;
cosmetic quality/repeat hints are accepted without claiming a meter, generated
passwords, external cache or constraint checking. Repeated entries must match
before any secret data is emitted. Full request details are paged, and opening
those details discards pending input and any stored first entry.

The independent protocol writer uses erased fixed frame storage and unbuffered
stdout. Assuan line bounds, percent escaping, cancellation/error codes and reset
semantics are tested separately from rendering. Diagnostics contain no submitted
secrets. Dump protection and erased owned buffers reduce exposure, but input is
not mlocked and toolkit/compositor allocations have their own lifetimes. Shared
presentation's accessibility/text-shaping limits also apply here.

The shared configuration envelope gains a pinentry namespace, the forwarding CLI
adds `canopa pinentry`, and release/updater inventory includes the executable and
setup documentation. Host GnuPG configuration is never modified by installation
or acceptance. Native validation uses a private compositor and GNUPGHOME, including
a real GnuPG cryptographic round trip. See [pinentry setup](../pinentry.md).

Follow-on quality, constraint and generation inquiries, protected mappings and
shared accessibility are specified in [ADR 0035](0035-authentication-recovery-and-agent-inquiries.md)
and [ADR 0032](0032-shared-authentication-input-and-accessibility.md). They supersede
the initial limitations above.

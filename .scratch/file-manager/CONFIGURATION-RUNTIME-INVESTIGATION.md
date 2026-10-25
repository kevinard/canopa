# Configuration ownership extraction

Read-only preparation while full-core13990 is live on clipboard-capacity source04.
No configuration queue implementation or new product input has been written yet.
The tracer at /tmp/canopa-files-configuration-jobs-tracer.rs is a draft only.
Previous goal turn was progress: action runtime checkpoint plus clipboard capacity
reproduction/fix and core/Iced/native evidence. Current13990 was confirmed live.

The next actual host-related extraction is the ordered configuration worker in
iced_desktop/configuration_ui.rs. Domain ConfigurationRuntime is already core;
its worker loop, request admission, publication and Drop policy still belong to
Iced. No new dependency is needed (futures-channel already exists). Follow
ADR0074/0076 and pre-agreed configuration/reload seams; record ADR0080 before
writing product tests/implementation after the frozen checkpoint is settled.

Proposed public ConfigurationJobs owns the bounded serialized Save/SaveSettings/
Reload/poll loop, last valid ConfigurationSnapshot, monotonic latest publication,
pending request count and one coalesced notice stream. Arc handle clones share
ownership; dropping a frontend clone or its reply/notice receiver cannot stop an
accepted save while the owning runtime holds a handle. Final-owner Drop requests
stop, with no UI join. Source/version checks in existing snapshot writers remain
the authority. Do not invent durable draft replay or filesystem Undo.

A snapshot retains current valid configuration, latest Updated/Rejected event and
pending count under one lock. Unlike the old consuming mailbox, reads need not
steal another frontend's last publication. A failed explicit write also publishes
a rejection while retaining the current valid configuration, so a detached reply
cannot silently hide its failure. Exact per-request one-shot acknowledgements
remain independent of coalesced notices. Ordinary external-reload rejection keeps
the existing last-valid policy. Explicit requests stay bounded, polling250ms stays
serialized with saves as before; measure resident idle performance separately.

The first tracer uses real private config files: submit Save then drop frontend,
reply and notices; owner still observes the written action. Drop a stale Save
reply; snapshot must retain both failure and prior valid actions. Then a real
external view edit must reload without any frontend. Correct KDL syntax is
`files { view column-width=390; }`, from existing configuration_reload tests.
Compile red before implementation. After first green, migrate the three existing
worker tests (source preservation/stale saves, settings+actions coexistence,
undrained notices) to the public core seam; preserve their behavioral assertions.

Integrate without a new forwarding-only runtime facade. Start ConfigurationJobs
in desktop::run before iced::daemon, keep the owning handle there and pass a clone
to Desktop::boot. There are only two boot callers (run and iced_desktop/tests.rs
helper). Preserve graceful startup failure as Result<ConfigurationJobs,String>.
Desktop stores the core handle and a separate presentation Notice key; the Iced
adapter only maps publications/streams and retains editor/widget state. Add core
pending requests to exit holds. Remove the old UI-owned request loop and Drop.
Existing action/preferences editor call sites can keep their async core methods.

This extraction is part of actual host ownership; it does not implement a daemon,
choose the final process layout or satisfy activation. NEXT-HOST-SLICE.md remains
an unimplemented layout/transport proposal. After this, own volume-monitor startup
and notification lifetime outside Iced before resident/headless CLI acceptance.

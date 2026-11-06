# Phase76: real Flatpak prepared-token ownership and unused-approval lifetime

Full eighteen-interface goal ACTIVE: eleven packaged, six experimental, Print
opt-in. The user chose Print features first; automatic Qt integration remains
deferred. This phase adds an application-side C test consumer and documentation.
No Rust, Cargo, vendor, installed routing, system service, queue or physical
printer changed. All native handles are terminal. No pending question or approval.

Read [the evidence index](print/flatpak-token-evidence.json) and
[source-only replay notes](print/flatpak-token-replay/README.md).

## Native results

Four runs pass, totaling 24 groups: token-dark-01 and token-light-02 each cover
nine groups; restart-dark-03 and restart-light-04 each cover three. Dark uses
100% scale, light 125%. All use actual Flatpak identities, the filtered public
portal bus, standard frontend277568 and unchanged normal470 Rust/Iced backend.
The host-only sentinel stays inaccessible. No broad filesystem/network/bus grant.

An original owner's prepared approval survives foreign attempts from another
connection in the same process, another process with the same app ID, and a
different app ID. Those attempts each open fresh Iced consent; actual cancellation
returns response1. The original owner then prints with the token, response0,
without renewed consent. Reusing the consumed token requires fresh consent.

Caller loss proactively releases the retained private output-directory FD:
pins `[14]` become `[]` before another Print request. A replacement process with
the same app ID cannot revive that token. For overlapping same-owner requests,
the first sends a real pipe FD and withholds its bytes. The controller observes
the exact pipe inode in the production snapshot worker, after token consumption.
A second request using that token requires fresh consent while the first remains
pending. After canceling the second, the first pipe is released and prints exactly.
No production hook, artificial clock or token expiry shortcut is used.

Separate restart profiles keep the same real application and original bus
connection alive. Stopping/restarting the private frontend drops the backend's
unused approval directory pins before the next request. Its old token requires
fresh consent. A new approval followed by backend SIGKILL and restart with the
same private recovery store also cannot revive its token. The still-connected
app can obtain a fresh approval and print after both restarts. This concerns
unused approvals and volatile tokens; crash during publication, power loss,
cross-restart resource quotas and physical job acceptance remain separate.

Six delivered PDFs are byte-identical to the known 2435-byte application source
`c63c47f47d16c4bd7d5de1269df4e2d8fd4f889c89a9d3214204e3fd37b01589`.
Explicit system qpdf/Poppler verify all 24 pages, PAGE-1 through PAGE-4 labels and
precise A4 boxes. One representative identical raster has the readable first-page
label and full stamp. Both Iced appearances and that page were visually inspected.
Native host audit confirms all 141 recorded PID/start identities absent and all
four private desktop directories removed.

## Bounded reusable fixture

Reuse the Phase74 private Flatpak tools/repository/installation and unchanged
Phase75 136-file runtime, commit
`394e7596f1d132f35bccd35338c60360f3f598b056e022a681fd932c6dff1e31`.
Two additional app refs, `org.canopa.PortalPrintOwner` and
`org.canopa.PortalPrintOther`, use the same controlled GIO client; existing GIO
and GTK app refs remain. There is no second SDK or platform download.

Canonical source: `scripts/desktop-fixture/print-flatpak-tokens.c`. Current client
`a81be8a14f9e9213eba71ba1aada110fc2336783b2693f6205f520ce75e33a91` is
28560 bytes, stored once by checksum and shared with staging. Its unused 5373a4
pre-stream prototype was never deployed or executed; its 28384-byte executable
is losslessly archived once in 6744 bytes, with the exact original source retained.
It is not a failing product baseline.

Only unused extracted private SDK headers/manuals were retired: 167 files,
509216 logical bytes, each verified against the retained signed original package
archive. System SDK files and runtime libraries/notices remain. Shared unique-file
allocation across Phase74/75/76 stays below 128 MiB with a 3 GiB free-space floor.
No Cargo command, new build graph, target tree, cache purge or per-case ELF copy.

The final source guard binds 1358 root inputs against Phase75's 1357: only the
new C consumer and two intended documentation changes. Normal backend, Portal
unit/integration executables and CUPS client/receiver/backend remain unchanged.
Tooling/native validation is appropriate; no workspace gate was repeated.
Phase73's exact ten Files failures still prevent full workspace acceptance.

Next: wider Print integration and failure/resource gates, including Documents
exports, standard-runtime applications, crash during publication and stalled
filesystems, device/driverless/custom precision, installed deployment and release
routing. Six experimental interfaces and final workspace acceptance remain open.
Use this same bounded store. Do not rerun one-time deployment, completed token/
restart profiles or workspace checks solely because a new goal turn starts.

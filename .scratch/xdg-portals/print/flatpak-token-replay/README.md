# Real Flatpak prepared-token acceptance

Run from the workspace root using the existing Phase74 private Flatpak store and
Phase75 SDK. See [the checkpoint](../../PHASE76-LIVE.md) and
[evidence index](../flatpak-token-evidence.json). These are application-side test
tools; Canopa's normal backend and consent helper remain Rust/Iced.

`deploy-clients.py` is completed one-time setup, not a command to rerun. It adds
Owner/Other app refs using one checksum-addressed client image and the existing
document/runtime. Neither app has broad filesystem, network or bus permissions.
All repositories retain a 3 GiB free-space reserve. The private tools come from
the retained signature-verified packages, with no system installation. The
retirement receipt binds all 167 unused extracted SDK headers/manuals to those
archives; it changes no system SDK files. The runtime inventory remains 136 files.

Compile canonical `scripts/desktop-fixture/print-flatpak-tokens.c` using the exact
command in `target/portal-print-76/client-build-02.json`. Its final A81BE image is
28560 bytes. `token-client-before-stream.c` matches the unused 5373 prototype's
source; its executable is losslessly XZ archived by checksum and was never run or
deployed. Keep one raw current image, not one per native case.

`native-tokens.py --profile ownership --scheme dark --out NEW_DIRECTORY` covers
the nine ownership/revocation/one-use groups. Use light for 125% scale.
`--profile restarts` covers the three unused-approval restart groups while the
same app stays connected. The old `ownership-native.py` is the exact original
ownership-only runner; the extended runner retains that branch. Final ownership
runs used the original runner; restart runs used the extended runner. Every
native run retains its exact probe-source copy and input checksums.

The runners import unchanged Phase75 `target/portal-print-75/native-gtk.py` assets
and `scripts/validate-desktop.py`. They select the pinned Niri and standard
frontend, normal backend with `--experimental-print`, a private recovery store,
private session bus, and a nonexistent private scheduler socket. No printer I/O
or live desktop routing. Native host execution is necessary; sandbox namespaces
can hide process identity and alter converter behavior. Core dumps are disabled.

The GIO client accepts the stdin protocol documented in
[the fixture guide](../../../../scripts/desktop-fixture/README.md#flatpak-prepared-token-ownership).
Actual request handles and frontend-authenticated app IDs distinguish caller bus
connections. Foreign/consumed/stale tokens open fresh Iced consent, which is then
canceled; do not describe these as automatic rejection. Original approvals remain
usable after foreign attempts. The held-pipe overlap waits for the actual worker
to hold the exact source pipe before issuing the second request. No production
hook or token clock change is involved.

Directory FD revocation is observed before another Print call. Frontend/backend
restart coverage concerns unused approvals only. It does not establish interrupted
publication, power-loss durability, cross-restart quotas or physical jobs.

`check-pdf.py` uses explicit `/usr/bin/qpdf`, `/usr/bin/pdftotext` and
`/usr/bin/pdftoppm`, a private cache and system Fontconfig. It checks all six PDFs
against the exact known source, all 24 page boxes/labels, and saves only one
representative identical raster. `host-audit.py` checks the four native reports,
all 141 recorded PID/start identities and four removed private directories.

Final runs are token-dark-01, token-light-02, restart-dark-03 and restart-light-04:
24 groups pass, no failed native run in this phase. Keep unique-file allocation
across Phase74/75/76 below 128 MiB and free space above 3 GiB. No Cargo rebuild is
needed for these application-fixture/docs changes. Earlier workspace Files
failures, wider Print gates and the eighteen-interface objective remain open.

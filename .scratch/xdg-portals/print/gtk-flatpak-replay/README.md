# Automatic GTK Print in the owned Flatpak runtime

Reuse the shared Phase74 store; do not create a second SDK or download a platform.
`target/portal-print-75/xkb-deployment-01.json` binds the current136-file runtime
inventory and commit. `gtk-app-update-01.json` and `client-build-02.json` select
the final0AF application. At the Phase75 seal the store contained GIO and GTK test
app refs plus one shared runtime. Phase76 adds the Owner and Other token probes
to that same store; see [the current checkpoint](../../PHASE76-LIVE.md).
Canopa's normal backend and Iced helper do not gain a GTK dependency.

`prepare-gtk-first.py`, `prepare-gtk.py` and `extend-xkb.py` are completed one-time
setup/continuation evidence, not commands to rerun. The first successful export
was reused when correcting its OSTree payload verification path. Full payload
hashes were checked before large raw staging duplicates were removed; after
deployment those stable paths were restored as hardlinks. Base GIO libraries were
also deduplicated against the existing immutable deployment. CLI translations are
recoverable byte-for-byte from the five retained, signature-verified Arch packages.
No Cargo cache or installed software was changed.

The runtime adds the installed GTK3.24.52 ELF closure, file print backend,
DejaVuSans font, a small Fontconfig file, GLib schemas and default-US keyboard data.
The explicit `xkbcli` compile uses only the staged include path; its final receipt
is `xkb-closure-03.json`. The two preceding failed probes remain. Locale is C and
only the Wayland socket is exposed to the app. Broader keyboard/localization and
standard-runtime/application acceptance remain separate.

Compile the canonical root `scripts/desktop-fixture/print-flatpak-gtk.c` with the
exact command in the final build receipt. The original A402 program/source belong
to the failed timing fixture. The old program is losslessly XZ archived by checksum;
restore it only when reproducing that failure. `gtk-before-spool-completion.c`
matches its original source SHA256 exactly. The final0AF program remains raw once
in the shared checksum store and in the necessary real Flatpak deployment.

`native-gtk.py` runs from the workspace root, in the native host environment,
with a new `--out` directory. Its modes are basic, layout, cancel and withdraw;
`--scheme light` selects125% scale. It uses the standard portal frontend and normal
opt-in backend. The app checks `/.flatpak-info`, host-only sentinel denial and an
absent GTK_USE_PORTAL override. Actual app image/mappings, implementation calls,
exported parent and public response codes are independently observed. The normal
Iced helper's mappings contain no GTK. Automatic accessibility/Documents setup
is disabled; the app retains the actual filtered portal bus and Wayland socket.

The application invokes GtkPrintOperation and renders through Cairo. GTK::done
can precede asynchronous spool completion; the controller waits for the public
Print response and host publication, then sends `quit` over owned stdin. The
app then hashes its completed private PDF. Successful host output must have
exactly those bytes. This pipe supplies lifecycle observation, not filesystem or
unrestricted bus access. Cancellation sends no document; caller termination must
withdraw Iced consent before any Print call.

The layout case selects pages2–4, two-up, reverse sheets and75% scale. Its sides
are `[4]`, `[2,3]`; reversal follows imposition as documented in ADR0076 and GTK's
`increment_page_sequence` implementation. Native04's `[4,3,2]` oracle was wrong;
its failed receipt remains. The corrected predicate retains exact-byte checks.

Final native coverage is dark-basic03, dark-layout-lifetime05 and light06: eight
cases. `check-gtk-pdf.py` uses explicit system Poppler/qpdf and verifies all twelve
pages, expected per-side labels, precise A4 boxes, stamp presence/absence and RMS0
across appearances. It saves only representative page renders. `host-audit.py`
checks all116 recorded identities and all six temporary desktop paths, including
failed runs. All final handles are terminal.

Keep shared fixture storage below128MiB and filesystem free space above3GiB.
Original/default Cargo outputs and profile selection stay intact. These tooling/
native-only changes do not require a new shared Rust workspace run; the earlier
ten Files failures remain open. Full Print/default routing and the full eighteen
portal objective remain active. Phase76 records the subsequent sandbox token/
lifetime checks; do not repeat completed Phase75 setup on continuation.

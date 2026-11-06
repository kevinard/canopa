# Real Flatpak Print fixture

The shared installation is under `target/portal-print-74/installation`. At the
Phase74 seal its refs were `org.canopa.PortalPrintProbe/x86_64/1` and
`org.canopa.PortalPrintRuntime/x86_64/1`. Phase75 extends that runtime and adds
`org.canopa.PortalGtkPrintProbe`; read the later GTK/Flatpak checkpoint before
continuing. Reuse the store; neither the full platform SDK nor
a Flathub download is needed for this scoped GIO caller. The app/runtime are test
fixtures, separate from standard-runtime/toolkit acceptance.

The packages, detached signatures, private extracted tools and repository are
retained once under that stable target directory. `flatpak-packages-plan.json`
records URLs, repository SHA256 values and valid Arch signatures. OSTree came
from the official package archive after the live mirror returned404. The fixture
does not modify host/user installations, remotes, system services or desktop routes.

The client source is `scripts/desktop-fixture/print-flatpak-client.c`. The actual
compile used `cc -std=c11 -D_GNU_SOURCE -Wall -Wextra -Werror -O1`, with flags from
`pkg-config --cflags --libs gio-unix-2.0`. Publish one executable by checksum and
link it into the app as `files/bin/probe`. `prepare-runtime.py` stages the recorded
ELF library closure and existing four-page stamped PDF fixture. The build-input
receipt binds source/program/library hashes. Runtime metadata names
`org.canopa.PortalPrintRuntime/x86_64/1`; app metadata uses that runtime and command
`probe`, without filesystem, network or CUPS-socket permissions.

`deploy-runtime.py` is the completed one-time continuation, NOT a fresh-install
builder. It intentionally consumes the prior successful runtime export receipt.
Do not rerun it. A fresh equivalent setup must initialize its own repository,
set `core.min-free-space-size=3GB`, export the runtime with `--runtime
--files=files`, run `build-finish --command=probe` on the app, then export it.
Add only that local repository to an explicitly private `FLATPAK_USER_DIR`, set
the private installation repository's reserve before pulling, and install only
the two local refs with `--no-deps --no-related`. Keep system/config/cache/home
paths private as recorded in the deployment receipt. Enforce the128MB fixture cap
and3GB free floor before mutations. No host pacman transaction is needed.

`native-flatpak.py` drives the real filtered session bus, standard frontend,
normal backend and Iced file picker on a private nested compositor. It selects
the existing checksum-addressed native executables and private Flatpak tools.
Run from the workspace root, in the native host environment, with a new `--out`
directory; `--scheme light` selects125% scale. The default three cases are
prepared, direct and cancellation. `--manual` is a retained diagnostic option,
unused in the final runs. All services retire before temporary-directory teardown.

The fixture disables automatic accessibility/Documents-portal setup. It grants
neither broad session-bus access nor host filesystem access. Public Print itself
uses the real portal proxy and FD transfer. Prepared and direct callers open and
unlink a private PDF before sending its FD. Method-call receipts independently
show the frontend-authenticated app ID reaching the implementation.

`direct-control.py` is the independent normal-worker direct-layout control.
`check-pdf.py` verifies exact boxes, text, stamp ink and all-page pixels. It must
run natively and explicitly use `/usr/bin/pdftoppm`, `/usr/bin/pdftotext` and
`/usr/bin/qpdf`; the Codex PATH override's bundled Poppler displayed missing glyphs.
Prepared output compares with the original input; direct output compares with
the actual worker's expected Ghostscript normalization. Initial failed oracle,
sandbox converter and bundled-renderer receipts are retained.

`host-audit.py` checks recorded host PIDs with their start times. The sandbox
control's namespace PID is deliberately excluded. The final audit verifies all102
recorded native identities absent and every desktop private path removed. Its
initial failure exposed two cache files from the first failed fixture only;
the exact cleanup receipt records their checksums and removal.

All source-only replays are preserved here; they use the stable target location
and require its recorded inputs. The canonical root fixture and development guide
contain the maintained behavior/scope. No Rust/Cargo/vendor/runtime source change
occurred, so the prior workspace result (ten Files failures) remains separate.

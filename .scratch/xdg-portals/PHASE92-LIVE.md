# Phase92: Automatic GTK4 file-dialog routing

Goal ACTIVE, Print deferred. Previous turn made concrete progress by correcting
the generator's none veto and verifying ordinary Qt. This turn adds proof that
an ordinary GTK4 application reaches Iced through toolkit-owned routing, rather
than the explicit GIO portal calls used by the existing real sandbox probe.

New scripts/desktop-fixture/file-chooser-gtk.c uses public GtkFileDialog/GFile
APIs only, without explicit portal/D-Bus calls. Installed GTK4.24.1 Open/read,
Save/write and Cancel pass through the actual Iced helper. Offered text filters,
Unicode/space filenames, original preservation, public+backend method traces and
GTK-free helper mappings pass. GTK_USE_PORTAL/GDK_DEBUG remain unset. The caller
itself links GTK, as an ordinary GTK app; no Canopa GTK dependency is added.
Screenshot inspected. This is a host application check, not new Flatpak proof.

Qt and GTK wrappers now share scripts/file_chooser_toolkit.py for private bus,
configuration generation, frontend/compositor, input, tracing, cache and cleanup.
The existing Qt C++ caller/recipe/executable is reused byte-for-byte, with all3
cases passing again. The one new GTK executable is23872bytes; no per-case caller
or backend copies, Cargo build, SDK/runtime download or archive staging.

## Failed control and correction

native-gtk-dark-01 remains FAILED, handle29117terminal1. The private bus advertised
only the backend, so GTK's environment_has_portals() returned false despite the
running frontend and healthy public FileChooser v4. Pinned GTK4.24.1 gdk/gdk.c
checks ListActivatableNames for org.freedesktop.portal.Desktop. Adding its standard
activation metadata fixes the fixture without forcing portal use or changing
Canopa runtime code. The harness now asserts frontend activatability before
starting either caller. The failed source and pre-extraction Qt harness are
losslessly preserved and match their recorded native hashes.

## Terminal acceptance

- native-gtk-dark-02/result.json PASS3groups, handle4832terminal0.
- native-qt-regression-01/result.json PASS3groups, handle39485terminal0.
- Python AST and both CLI help checks PASS. The only post-native module edit is
  corrected help text; help-text-check-01.json proves its functional AST identical
  and matches both native-tested source hashes. No unnecessary native rerun.
- Source guard PASS1357unchanged Rust/Cargo/vendor runtime inputs. Normal backend
  remains dfdf49c5, frontend277568 and installed Niri f09e0b1c identities pinned.
- Host audit PASS34recorded PIDs absent, all3private paths absent and no process
  argv/environment references. All current handles terminal, no live validation.

Combined receipt: target/portal-filechooser-gtk-92/result-01.json.
Guide and ADR0116 updated in the checkout. No runtime artifact changed, so the
accepted development archive remains phase91, verified byte-for-byte unchanged:
target/portal-release-package-check/canopa-x86_64-unknown-linux-gnu.tar.gz
Bytes209602464; SHA256bd3cb68ddd2943e37cad1141c0a2a3fed2e27aba927f9cd4d30864a343aa4739.
The new GTK notes are checkout-only until the next runtime delivery. Do not claim
this turn refreshed the bundle or established new whole-workspace/native matrix
acceptance. Latest full workspace gate86, full Portal library89 remain scoped.
Root remains above3GiB free; no cache cleanup or user disk request.

This cohort is DONE. Do not replay toolkit matrices/package on continuation or
turn the Qt startup limit into a hardening detour. Next inspect the concrete
remaining RemoteDesktop touch capability in current code and the compositor
contract. The map records absent legacy touch methods and an unadvertised device
class; implement a real input path before changing advertised masks, not stubs.
Read relevant domain/vendor instructions before edits, reuse stable sources and
keep build storage bounded. Print remains deferred. No external blocker or
missing user answer; full goal ACTIVE and live routing unchanged.

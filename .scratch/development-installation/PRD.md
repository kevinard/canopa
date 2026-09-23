# Automatic local development installation

The owner wants their desktop to use current Niribar changes without manually
checking executable versions. They selected automatic updates after local
commits, keeping the last working version if a build fails. Nix stays deferred.

- Follow this repository's local `main` branch once per minute during the
  graphical session. Do not fetch or pull remote changes.
- Build committed files in a separate checkout. Preserve uncommitted work in
  the author's checkout and do not run partially edited source.
- Build and smoke-test both executables in a dedicated target directory.
  Install the bar and Wi-Fi provider together through one atomic current link.
- Restart an already-running user service after successful activation. Check
  that it remains active with the intended executable; restore the previous
  binary if those initial checks fail. Leave an intentionally stopped bar stopped.
- Preserve the existing service's session/dependency configuration via a drop-in.
  Back up the original binaries and retain the previous successful release.
- A commit arriving during a build supersedes that build. Failed commits have
  a retry cooldown; new commits can try immediately. Concurrent runs must not
  interfere with each other.
- Provide status, update/retry and disable instructions, including the expected
  compilation delay. Verify actual installation, automatic commit detection,
  paired executable identity, service health and timer activation on this host.

The updater tests run at its CLI boundary using real temporary Git repositories,
with Cargo and the user service manager represented by external test programs.

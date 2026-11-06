# Initial application print preferences

Status: scoped worker, public Iced and prepared-client validation complete.

An application can retain its last print settings, but queue defaults used to
replace those settings when Canopa installed printer capabilities. The initial
printer, paper and device choices now resolve against the selected queue before
the dialog installs its draft. This does not grant approval.

Only enumerated queue/instance identities can restore a printer. A missing
remembered printer leaves the destination unselected with an explanatory notice.
Print-to-file may restore an offered output format, but still requires a fresh
Files picker selection. Application output paths never enter destination
approval or the worker's device preferences.

The worker receives a bounded whitelist for source, paper type, quality,
resolution, duplex, color and paper. Current IPP values and selected-driver PPD
aliases resolve to advertised choices. Conflicting or unavailable spellings
retain the actual queue default for that field and produce a notice that survives
a successful printer check. Human labels are not option aliases. Unknown fields
are not forwarded to CUPS.

Resolution equality preserves both axes and original DPI/DPCM units. Equivalent
600dpi and 600x600dpi spellings match the same choice. Rounded integer DPI hints
cannot choose a DPCM setting. Replies retain the exact selected resolution under
`canopa-printer-resolution`, alongside the ordinary client rendering hints;
restoration checks that those representations agree. Physical device options
retain their original units.

Paper names resolve through advertised PWG identities and the public CUPS PPD
name mapping. Supplied dimensions must agree. Ambiguous variants are not guessed.
Custom dimensions enter the draft without printable geometry and require the
same full device check as a user edit. Application margins cannot become printer
printable margins. File paper remains the application's editable file geometry.

Initial hints are consumed after the current capability response is installed.
Manual destination selection and refresh discard them, so a later response cannot
replay them over user choices. Destination/generation checks reject stale work.
The complete option check and fresh pre-Create check from
[ADR0092](0092-printer-controls-and-driver-constraints.md) remain authoritative.
Prepared physical-copy and page-layout ownership are unchanged.

Print stays opt-in. Authenticated devices, broader driverless/physical acceptance,
remaining failure gates, runtime packaging and default routing remain open.
Automatic Qt client integration stays deferred by the user's choice.

Both appearances pass public preference restoration, unavailable/missing hints,
conflict correction and changed-driver rejection. Selected source/type/quality
reach the accepted IPP request and all four raster pages retain600x1200DPI.
Actual GTK and explicit Qt callers retain their A4 preference over a saved custom
queue default, with the prepared three-page sequence copied twice. Explicit
Printer default restores125x225mm; explicit application custom paper is rechecked
before Create. The Qt caller remains an explicit portal probe. These observations
are scoped private-device evidence; physical and general driverless gates remain.

See [preference evidence](../../.scratch/xdg-portals/print/preferences-evidence.json)
and [the checkpoint](../../.scratch/xdg-portals/CONTINUE.md).

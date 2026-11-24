# Printing

Canopa has its own print dialog, served by the portal backend
`xdg-desktop-portal-canopa` as the XDG Print portal. It is experimental and off
by default. Unless you turn it on, the Canopa session routes printing to
`xdg-desktop-portal-gtk`.

Applications reach this dialog only through the Print portal: sandboxed
applications (such as Flatpak apps) and toolkits configured to use portals.
Unsandboxed GTK and Qt applications normally show their own print dialog.

To set up printers, use the Printers page in [Canopa Settings](settings.md).

## Turn on the Canopa print dialog

Two changes are needed. Make both, or neither: if the descriptor lists Print but
the backend runs without `--experimental-print`, applications cannot print.

1. Start the backend with `--experimental-print`. On Arch, add a systemd drop-in:

   ```sh
   systemctl --user edit xdg-desktop-portal-canopa.service
   ```

   with this content:

   ```ini
   [Service]
   ExecStart=
   ExecStart=/usr/bin/xdg-desktop-portal-canopa --experimental-print
   ```

   For a `~/.local` install, use `%h/.local/bin/xdg-desktop-portal-canopa`.
2. List Print in a per-user portal descriptor. The portal frontend reads
   `~/.local/share/xdg-desktop-portal/portals/` before the system directory, and
   a user `canopa.portal` replaces the packaged one:

   ```sh
   mkdir -p ~/.local/share/xdg-desktop-portal/portals
   sed 's/^Interfaces=/Interfaces=org.freedesktop.impl.portal.Print;/' \
     /usr/share/xdg-desktop-portal/portals/canopa.portal \
     > ~/.local/share/xdg-desktop-portal/portals/canopa.portal
   ```

   The packaged routing (`default=canopa;gtk;`) then sends Print to Canopa. For a
   portable install, add `org.freedesktop.impl.portal.Print=canopa` to the
   `[preferred]` group of your portal configuration as well.

Log out and back in to apply both. To undo, remove the drop-in and the user
descriptor. While the user descriptor exists it hides later package changes to
the descriptor, so recreate it after upgrading Canopa.

### Requirements

The dialog talks to the CUPS scheduler through the CUPS client library. It lays
out and converts documents with external tools, each run in an isolated
bubblewrap sandbox. Install them before turning Print on; the Arch package does
not pull them in:

| Program or library | Arch package | Used for |
| --- | --- | --- |
| `libcups.so.2` | `libcups` | Printers, options and job submission (unless the private runtime is bundled) |
| `/usr/bin/bwrap` | `bubblewrap` | Sandbox for every converter |
| `/usr/bin/gs` | `ghostscript` | Normalizing PDF and PostScript, and PostScript output |
| `libqpdf` | `qpdf` | Page layout |
| `/usr/bin/rsvg-convert` | `librsvg` | SVG documents |
| `/usr/bin/pdftocairo` | `poppler` | SVG file output |

## The print dialog

Choose a destination: one of your CUPS printers or Print to file. Refresh reloads
the printer list. A ready printer shows **Ready** beside **Destination**.
Busy or stopped printers and printers that cannot accept jobs show an explanation
below the destination.

**Document** groups pages, copies and layout.
**Paper and printer options** groups media and settings from the printer. For
file output, the second tab is **Paper and margins**.

For a printer, the dialog offers:

- **Paper**: the sizes the printer reports, or Printer default.
- **Custom paper size**: when the printer supports a custom range. Enter width
  and height in millimetres with up to two decimal places, then choose Check
  paper size. The dialog shows the confirmed printable margins. Changing the size
  or the sides needs another check.
- **Sides**: one-sided, or two-sided on the long or short edge.
- **Color** or **Monochrome**.
- **Copies**, within the range the printer reports, with Collate copies.
- **Paper source**, **Paper type**, **Print quality** and **Resolution**, when the
  queue advertises them. Choices the driver cannot combine stay editable but keep
  Print disabled; Check printer settings confirms them together.

If printer discovery fails, use Retry or choose the file option from the error
card. That option is named Save as PDF when PDF is the only available format.

For Print to file, choose the file format (PDF, PostScript or SVG), a paper size
(A4, Letter, Legal or custom) with margins, and the file with Choose file…, which
opens the Canopa Files save dialog. Replacing an existing file needs your
confirmation. A file is published only once it is complete; an interrupted save
never leaves a partly written file in place of the old one.

When Canopa lays out the document itself, the dialog also offers:

- **Pages**: All pages, Page ranges (for example `1-3, 5`), and Current page or
  Selection when the application provides them. Ranges print in the order you
  type them.
- **Scale (%)**, **Orientation**, **Page set** (all, even or odd pages),
  **Reverse order**, **Pages per sheet** and **Page arrangement**.

Applications that prepare their own pages keep their layout; Canopa adds only
copies, sides and the printer options.

The dialog starts from the application's saved print settings when they still
match the selected printer. A setting the printer no longer offers shows a notice
and uses the printer default. A remembered file destination always needs a fresh
choice.

Print sends nothing until you choose Print. Cancel, Escape, closing the window or
the application exiting cancels the request. If printing is disabled by the
[Lockdown policy](portals.md), the dialog refuses to print.

## Signing in to a printer

When a print server asks for credentials, Canopa opens a Printer authentication
dialog showing the server address, the printer or realm when available, and
whether the connection is local, on this computer or encrypted. Enter the
username (and domain, if asked) and password, then choose Authenticate. If the
server rejects them, you can try again or cancel.

Credentials are used only for that print request and are not saved by Canopa.
Canopa refuses to send credentials to another computer over an unencrypted
connection.

Credential length depends on the CUPS client:

| CUPS client | Username | Password |
| --- | --- | --- |
| Private runtime | 127 bytes | 1023 bytes |
| System `libcups` | 64 bytes | 189 bytes for `username:password` combined |

## Jobs waiting for sign-in

A printer can hold a job until you sign in, for example when a device asks for
credentials after receiving the document. To review such jobs, run:

```sh
xdg-desktop-portal-canopa --printer-jobs
```

This asks the running backend to open the Printer jobs window, so the backend must
run with `--experimental-print`. Choose a printer, select a job marked "Sign-in
needed", and choose Sign in…. The job continues with the document the printer
service already holds. The review never sends the document again, creates a job
or cancels one. It lists only jobs owned by the account you are signed in as.
If there are no printers, **Open printer settings** takes you to the Printers
page in Canopa Settings. Use **Refresh** after adding a printer.

## Recovery

Canopa keeps a private record of each print job until its outcome is known. If
the backend or your session stops while a document is being saved or sent, the
record stays behind. Records are kept in
`$XDG_STATE_HOME/canopa/portal/print-recovery` (default
`~/.local/state/canopa/portal/print-recovery`).

At startup, the backend cancels unfinished printer jobs that it can identify as
its own and that never received their document. A job whose document the printer
received is always kept, and no document is ever sent again. If anything remains, the Print recovery window
opens. Open it yourself with:

```sh
xdg-desktop-portal-canopa --recover-print
```

This works without `--experimental-print`. Use `--print-recovery-store DIR` to
review another location.

- **Files** lists interrupted saves. Save document copy… writes the retained
  document to a new destination you choose, and Save file moved aside… does the
  same for a file that was being replaced. Remove from list… forgets the entry
  and leaves the files in place; Open folder shows them in your file manager.
- **Printers** lists jobs whose outcome is unconfirmed. Check and clean up checks
  every record, signing in if needed, and cancels only jobs it can identify that
  have not been accepted. Remove record… forgets a record without touching the
  printer job. Check your printer before printing another copy.

The recovery lists have a fixed size (16 printer records). When the printer list
is full, new print jobs are refused until records are resolved or removed.

## Private CUPS client runtime

Packagers can bundle a private CUPS client built from Canopa's CUPS fork. It
allows longer credentials, handles quoted and Digest authentication fields
correctly, and rejects incomplete server replies. Only Canopa's isolated print
workers load it; the CUPS scheduler, filters and other programs keep the system
library. Bundling it does not turn Print on.

The backend looks for `lib/canopa/print/libcups.so.2` in its own installation
prefix (for `/usr/bin/xdg-desktop-portal-canopa`, that is
`/usr/lib/canopa/print/libcups.so.2`). If the file is missing, it uses the system
`libcups.so.2`. To test another build, set `CANOPA_PRINT_CUPS_LIBRARY` to its
absolute path when starting the backend. If the selected file is present but
invalid, printing to printers is unavailable rather than falling back.

The Arch packages do not include the private runtime. The release workflow
builds it and adds it to the release archive. To build and package it yourself:

```sh
python3 scripts/build-cups-authentication.py --archives target/fork-archives --download \
  --work-dir target/cups-authentication-compat
bash scripts/package-release.sh BUILD_DIR OUTPUT_DIR \
  --cups-runtime target/cups-authentication-compat/canopa-build.json
bash scripts/smoke-release.sh OUTPUT_DIR/canopa-x86_64-unknown-linux-gnu.tar.gz \
  --require-print-runtime
```

Building needs Python 3, GNU `readelf`, a C compiler, Make, pkg-config and the
GnuTLS development files. Without `--cups-runtime`, the archive uses the system
CUPS client.

| Path | Contents |
| --- | --- |
| `lib/canopa/print/libcups.so.2` | Private CUPS client |
| `share/canopa/print-runtime.json` | Fork commit, archive checksum, library checksum and required shared libraries |
| `share/licenses/canopa-print-cups/` | Original CUPS `LICENSE` and `NOTICE`, and a summary of the changes |

Keep these paths together when moving an install. The library still needs the
system libraries listed in `print-runtime.json`, including GnuTLS.

### Optional IPP backend

The fork also provides an IPP backend for the CUPS scheduler that keeps an
explicitly chosen resolution, which the stock backend drops. It is compiled
against the private client:

```sh
python3 scripts/build-print-ipp-backend.py \
  --client-build target/cups-authentication-compat/canopa-build.json \
  --work-dir target/print-ipp-backend-compat/maintained
bash scripts/package-release.sh BUILD_DIR OUTPUT_DIR \
  --cups-runtime target/cups-authentication-compat/canopa-build.json \
  --cups-ipp-backend target/print-ipp-backend-compat/maintained/canopa-build.json
bash scripts/smoke-release.sh ARCHIVE --require-print-runtime --require-print-backend
```

`--cups-ipp-backend` requires `--cups-runtime`. The archive then contains
`lib/canopa/print/ipp`, `share/canopa/print-backend.json` and
`share/licenses/canopa-print-ipp-backend/`. Packaging does not install it into
CUPS or change any queue; it takes effect only for a queue that a CUPS
administrator explicitly configures to use it.

## Known limitations

- Print is opt-in: start the backend with `--experimental-print` and list Print in the descriptor.
- The Arch packages do not include the private CUPS client or declare the print tools as dependencies.
- Qt applications that use `QPrintDialog` show Qt's own dialog and print through CUPS directly.
- Driverless and PDF-only queues offer only the device's default resolution.
- With the stock CUPS IPP backend, an explicitly chosen resolution does not reach the device.
- With the system `libcups`, long credentials are refused and some Digest servers fail.
- The printer-job review lists only jobs of the account you are signed in as, so jobs created under another CUPS account can be hidden.
- Prepared print approvals are kept in memory only and do not survive a backend restart.
- The print, authentication, recovery and printer-job dialogs do not yet publish an accessibility tree.

For the design, see [Printing architecture](architecture/printing.md).

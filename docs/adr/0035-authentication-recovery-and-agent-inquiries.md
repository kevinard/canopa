# Authentication recovery and agent inquiries

Status: accepted; focused and private native checks are recorded in the follow-up PRD.

Authentication policy remains with PAM, polkit and GnuPG. Shared software controls
and protected input do not transfer policy or credential verification to the shell.
Owned response storage uses dedicated best-effort mlocked, non-dumpable mappings;
editing and request retirement erase unused bytes. This does not guarantee that
native libraries, explicitly revealed text, kernel transport or compositor buffers
never hold another copy. See ADR 0032 for shared editing, shaping and accessibility.

After successful locker authentication, PAM_NEW_AUTHTOK_REQD from account checks
creates a distinct consent step. Enter/Change password accepts; Cancel drops the
worker and leaves the session locked. Only then does the same PAM transaction call
pam_chauthtok(PAM_CHANGE_EXPIRED_AUTHTOK). Each conversation prompt receives a fresh
response. A successful change, fresh account check and pam_end are all required
before unlock. The example distribution policies include their password stack.
No host policy is installed by development tests.

Polkit display metadata is resolved only for the selected UID from the authority's
permitted identities. AccountsService FindUserById and individual public properties
provide the label/icon; NSS is a fallback. Labels cannot add identities or replace
numeric authorization. NSS and bounded raster decoding run in a cancellable,
two-second process-group-owned helper. Request/selection generations reject stale
results. No directory of users, email, home or password hints is requested.

Pinentry uses the calling agent's QUALITY, CHECKPIN and GENPIN inquiries, following
the primary GnuPG pinentry implementation. Quality queries are debounced and stale
scores discarded. Enforced constraints require affirmative inquiry completion;
errors/cancellation do not approve. Required checks refuse input beyond the
upstream 300-byte inquiry limit, instead of approving an unchecked suffix.
Generation begins masked and requires explicit reveal before acceptance, allowing
users to record the value. Hiding it again is allowed; repeat and policy checks
still apply. No clipboard or local password generator is involved.

A bounded inquiry worker keeps pipe reads outside rendering. Requests and data
buffers are erased, replies have an absolute two-second deadline including partial
reads, and cancellation retires the window while the outstanding query winds down.
Malformed/timeout transport retires the protocol connection to prevent replaying
unconsumed inquiry bytes as commands. Explicit agent ERR/CAN replies remain aligned.

Accessibility actions are checked after updating the current presentation's
revision, closing the pre-publication window between consumer state transitions
and the next frame. Consumer phase, identity and prompt checks remain authoritative.

Private tests use real PAM with a fixture module, independent D-Bus/AT-SPI/Assuan
peers, and real GnuPG encryption in a temporary keyring. They do not establish
physical device, distribution PAM or display-manager seat handoff acceptance.

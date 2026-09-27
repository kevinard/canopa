# All named actions in banner cards

An advertised `actions` capability must provide the sender's actions to the user.
Project every named action into its banner, in sender order, while keeping default
activation separate. Lay out at most two controls per row and let ADR 0046's bounded
card scrolling contain additional rows. This supersedes the two-action projection
preserved by ADR 0046; it does not change focus, timeout, replacement or transient
retention policy. Keys and revision-bound targets remain opaque.

The Notification Center remains the intentional keyboard interaction surface and
the view of notifications excluded by banner policy. Its pagination and accessibility
requirements remain independent of the number of actions on a visible banner.

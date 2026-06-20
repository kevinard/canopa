# First decisive notification policy rules

Notify evaluates notification delivery policy as ordered rules where the first decisive matching rule determines whether a notification is shown, silenced, or dropped. This deliberately avoids Dunst-style cumulative mutation for delivery decisions so app-permission behavior remains auditable and easy to explain.

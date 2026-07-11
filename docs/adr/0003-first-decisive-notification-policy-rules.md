# First decisive notification policy rules

Notify evaluates notification delivery policy as ordered rules where the first decisive matching rule determines whether a notification is shown, silenced, or dropped. This deliberately avoids Dunst-style cumulative mutation for delivery decisions so app-permission behavior remains auditable and easy to explain.

Configuration-authored rules retain declaration order. User Rule Store rules use a fixed specificity order within one App Identity: category plus urgency, category-wide, app plus urgency, then app-wide. This preserves existing app-wide behavior while making category controls deterministic.

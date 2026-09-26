# Ordered body segments for body images

Freedesktop body images are part of notification body markup, not notification-level visual hints. Canopa Notify represents interpreted notification body presentation as ordered body segments, with styled text and body-embedded image references kept in sender order and the plain Notification Body retained as the fallback, instead of storing body images as a separate side list.

This keeps banners, Notification Center projections, and diagnostic JSON aligned on a single body-content shape, preserves text-image ordering, and avoids confusing Body-Embedded Notification Images with notification-level Notification Images. Because the shared panel protocol must carry this generic rich body content, the protocol shape advances to a new version when the feature is exposed; the freedesktop `body-images` capability remains unadvertised until Notify-owned body-display surfaces can render the segments end to end.

# Generic styled text in panel protocol

Notification body markup is interpreted by Canopa Notify, but the shared panel protocol carries generic styled text runs rather than raw freedesktop notification markup or Notify-specific body structures. This lets panel renderers preserve inline emphasis for notification center content while keeping `canopa-panel-protocol` as reusable panel infrastructure instead of a notification-domain protocol.

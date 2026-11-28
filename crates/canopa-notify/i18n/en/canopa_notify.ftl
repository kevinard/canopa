# Canopa Notifications. English is the source language.

## Banners

# Window title of the layer-shell surface that holds notification banners.
notify-banner-window-title = Notification banners
# Accessible name of the banner window, read by screen readers.
notify-banner-region = Canopa Notify notifications
# Screen-reader announcement of a new banner. $app is the sending application,
# $summary and $body are the notification's own text (not translated).
notify-banner-announcement = { $app }: { $summary }. { $body }
# Accessible name of a banner's close (×) button.
notify-banner-dismiss = Dismiss notification
# Accessible name of a banner's default action.
notify-banner-open = Open notification
# Placeholder of the inline reply field when the application gives none.
notify-reply-placeholder = Reply…
# Sends the inline reply.
notify-reply-send = Send
# Closes the inline reply field without sending.
notify-reply-cancel = Cancel
# Summary shown instead of a notification's content while content is hidden
# (screen sharing, lock screen).
notify-hidden-summary = New notification
# Lock-screen feed: name shown for a sender that gave no application name.
notify-lock-unnamed-app = Application

## Buttons Notify labels when an application sends a portal button with a
## known purpose but no label.

notify-portal-reply = Reply
notify-portal-answer = Answer
notify-portal-decline = Decline
notify-portal-hang-up = Hang up
notify-portal-speaker-on = Speaker on
notify-portal-speaker-off = Speaker off

## Notification Center

# Title of the Center's reading view.
notify-center-title = Notifications
# Title of the Center's settings view.
notify-center-settings-title = Notification settings
# Center title while Do Not Disturb is on. $state is a notify-dnd-* state,
# possibly followed by a suppression reason.
notify-center-title-dnd = Notifications · Do Not Disturb { $state }
# Center title while banners are suppressed automatically. $suppression is a
# notify-suppression-* message.
notify-center-title-suppressed = Notifications · { $suppression }
# Label of a notification's arrival time; the value is a time or a date.
notify-center-received = Received
# Accessible name and tooltip of the unread indicator.
notify-center-unread = Unread
# Group title for a sender that gave no application name.
notify-center-unknown-app = Unknown application
# Opens the inline reply field of a notification.
notify-center-reply = Reply…
notify-center-open = Open notification
notify-center-dismiss = Dismiss
notify-center-mark-read = Mark as read
notify-center-remove = Remove
notify-center-clear-active = Clear active notifications
notify-center-delete-history = Delete notification history
notify-center-mark-all-read = Mark all read
notify-center-clear-all = Clear all
# Restores the notifications that "Clear all" removed, for a few seconds.
notify-center-undo-clear = Undo clear
# Per-application delivery control.
notify-center-app-permission = App permission
# Per-category delivery control. $category is a freedesktop category such as
# email.arrived (not translated).
notify-center-category-permission = Category: { $category }
notify-center-first-page = First page
notify-center-next-page = Next page
notify-center-previous-page = Previous page
# Opens the Center's settings view.
notify-center-settings = Settings
notify-center-empty = No notifications
notify-center-empty-body = New notifications appear here.
# Returns from the settings view to the notifications.
notify-center-back = Back to notifications
# Groups the reading view by application.
notify-center-group-by-app = Group by app
# Returns to the single chronological list.
notify-center-timeline = Show timeline
# Errors shown in the Center.
notify-center-action-too-large = One stored notification action exceeds the display budget.
notify-center-item-too-large = One stored notification exceeds the display budget.
# $socket is a file path, $error the system's error message.
notify-center-daemon-unavailable = Canopa Notify daemon unavailable at { $socket }: { $error }
notify-center-stream-invalid = Notification Center stream ended with an invalid or incomplete frame; reopen the panel.
notify-center-too-many-subscriptions = Too many Notification Center subscriptions; close a panel and retry.
notify-center-too-large = Notification Center exceeds the 1 MiB display limit; use notification history controls.

## Delivery choices: per-application options in the Center.

notify-permission-allow = Allow
notify-permission-silence = Silence
notify-permission-drop = Drop
# The settings view's names for the same choices.
notify-delivery = Delivery
notify-delivery-allow = Show banners
notify-delivery-silence = Save quietly
notify-delivery-drop = Block

## Grouped view

# Summary of one application's group.
notify-group-summary = { $count ->
    [one] { $count } notification
   *[other] { $count } notifications
}
# Summary of one application's group with unread notifications.
notify-group-summary-unread = { $count ->
    [one] { $count } notification
   *[other] { $count } notifications
} · { $unread } unread
# Shows a collapsed group's notifications.
notify-group-show = Show
# Collapses a group to its newest notification.
notify-group-collapse = Collapse
# Body of a collapsed group, after its newest notification. $count is the
# number of other notifications.
notify-group-more = and { $count } more

## Do Not Disturb

notify-dnd-title = Do Not Disturb
notify-dnd-off = Off
notify-dnd-on = On
# $until is a time today or a date and time.
notify-dnd-on-until = On until { $until }
# Turned on by the daily schedule.
notify-dnd-scheduled = On (schedule)
notify-dnd-scheduled-until = On (schedule) until { $until }
notify-dnd-for-hour = For 1 hour
notify-dnd-until-tomorrow = Until tomorrow
# A Do Not Disturb state and an automatic suppression reason.
notify-dnd-with-suppression = { $state } · { $suppression }
notify-suppression-fullscreen = Fullscreen: banners are saved quietly
notify-suppression-sharing = Screen sharing: banners are hidden

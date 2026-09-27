# Policy changes apply to future notification requests

Changes to Notification Rules, Do Not Disturb, and Notification Configuration policy rules apply to subsequent Notification Server Requests instead of re-evaluating Active Notifications. Existing Notification Threads retain the Delivery Outcome and Notification Banner state produced when their current revision was accepted, avoiding retroactive lifecycle changes, partial re-evaluation failures, and receipt-versus-policy ordering machinery for little user-visible gain.

Replacement Notification Server Requests are subsequent requests and use the current Loaded Delivery Policy State even when they revise a Notification Thread created under an older policy snapshot. Turning off Do Not Disturb or removing a rule does not resurrect Notification Banners for existing threads.

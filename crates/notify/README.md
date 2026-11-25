# jetfish_notify

Desktop-notification parsing for jetfish: OSC 9, OSC 99 (kitty), and OSC 777
(urxvt).

Parse-only — the host owns actually displaying a `Notification` via its OS
notification backend (macOS `osascript`, D-Bus, …).

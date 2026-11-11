# Canopa shared formatting: units, durations and relative times.
# English is the source language.

format-percent = { $value }%

format-size-bytes = { $count ->
    [one] { $value } byte
   *[other] { $value } bytes
}
format-size-kib = { $value } KiB
format-size-mib = { $value } MiB
format-size-gib = { $value } GiB
format-size-tib = { $value } TiB
format-size-kb = { $value } kB
format-size-mb = { $value } MB
format-size-gb = { $value } GB
format-size-tb = { $value } TB
format-size-pb = { $value } PB

format-duration-seconds = { $seconds } s
format-duration-minutes = { $minutes } min
format-duration-hours = { $hours } h
format-duration-hours-minutes = { $hours } h { $minutes } min

format-relative-now = now
format-relative-minutes-ago = { $count ->
    [one] { $count } minute ago
   *[other] { $count } minutes ago
}
format-relative-hours-ago = { $count ->
    [one] { $count } hour ago
   *[other] { $count } hours ago
}
format-relative-days-ago = { $count ->
    [one] yesterday
   *[other] { $count } days ago
}
format-relative-months-ago = { $count ->
    [one] last month
   *[other] { $count } months ago
}
format-relative-years-ago = { $count ->
    [one] last year
   *[other] { $count } years ago
}
format-relative-minutes-ahead = { $count ->
    [one] in { $count } minute
   *[other] in { $count } minutes
}
format-relative-hours-ahead = { $count ->
    [one] in { $count } hour
   *[other] in { $count } hours
}
format-relative-days-ahead = { $count ->
    [one] tomorrow
   *[other] in { $count } days
}
format-relative-months-ahead = { $count ->
    [one] next month
   *[other] in { $count } months
}
format-relative-years-ahead = { $count ->
    [one] next year
   *[other] in { $count } years
}

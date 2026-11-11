# Canopa : formats partagés (unités, durées et dates relatives).
# Traduction française : assistée par machine, à relire (machine-assisted, needs review).

format-percent = { $value } %

format-size-bytes = { $count ->
    [one] { $value } octet
   *[other] { $value } octets
}
format-size-kib = { $value } Kio
format-size-mib = { $value } Mio
format-size-gib = { $value } Gio
format-size-tib = { $value } Tio
format-size-kb = { $value } ko
format-size-mb = { $value } Mo
format-size-gb = { $value } Go
format-size-tb = { $value } To
format-size-pb = { $value } Po

format-duration-seconds = { $seconds } s
format-duration-minutes = { $minutes } min
format-duration-hours = { $hours } h
format-duration-hours-minutes = { $hours } h { $minutes } min

format-relative-now = à l’instant
format-relative-minutes-ago = il y a { $count ->
    [one] { $count } minute
   *[other] { $count } minutes
}
format-relative-hours-ago = il y a { $count ->
    [one] { $count } heure
   *[other] { $count } heures
}
format-relative-days-ago = { $count ->
    [one] hier
   *[other] il y a { $count } jours
}
format-relative-months-ago = { $count ->
    [one] le mois dernier
   *[other] il y a { $count } mois
}
format-relative-years-ago = { $count ->
    [one] l’année dernière
   *[other] il y a { $count } ans
}
format-relative-minutes-ahead = dans { $count ->
    [one] { $count } minute
   *[other] { $count } minutes
}
format-relative-hours-ahead = dans { $count ->
    [one] { $count } heure
   *[other] { $count } heures
}
format-relative-days-ahead = { $count ->
    [one] demain
   *[other] dans { $count } jours
}
format-relative-months-ahead = { $count ->
    [one] le mois prochain
   *[other] dans { $count } mois
}
format-relative-years-ahead = { $count ->
    [one] l’année prochaine
   *[other] dans { $count } ans
}

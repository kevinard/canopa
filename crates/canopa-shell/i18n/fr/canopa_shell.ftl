# Canopa : Shell : barre, panneaux et lanceur.
# Traduction française : assistée par machine, à relire (machine-assisted, needs review).

## Panel titles

shell-panel-overview = Vue d’ensemble
shell-panel-quick-settings = Réglages rapides
shell-panel-calendar = Calendrier
shell-panel-sound = Son
shell-panel-network = Réseau
shell-panel-notifications = Notifications
shell-panel-idle-inhibitor = Maintenir éveillé
shell-panel-battery = Batterie
shell-panel-workspaces = Espaces de travail
shell-panel-focused-window = Fenêtre active
shell-panel-launcher = Lanceur
shell-panel-choose-item = Choisir un élément
shell-panel-wifi = Wi-Fi

## Built-in bar buttons (config/default.kdl `label-id` and `tooltip-id`)

shell-module-search = Rechercher
shell-module-overview = Vue d’ensemble
shell-module-controls = Réglages
shell-module-bluetooth = Bluetooth
shell-module-power-and-display = Alimentation et écran
shell-module-lock = Verrouiller
shell-module-suspend = Mettre en veille
shell-module-hibernate = Hiberner
shell-module-log-out = Se déconnecter
shell-module-reboot = Redémarrer
shell-module-power-off = Éteindre
shell-tooltip-launcher = Lanceur
shell-tooltip-overview = Vue d’ensemble
shell-tooltip-quick-settings = Réglages rapides
shell-tooltip-workspace = Espace de travail { $index }

## Built-in menus (config/default.kdl `title-id`, `prompt-id`, `label-id`)

shell-menu-launcher-prompt = Rechercher des applications, des fichiers et plus
shell-menu-log-out-title = Se déconnecter ?
shell-menu-reboot-title = Redémarrer ?
shell-menu-power-off-title = Éteindre ?
shell-menu-unsaved-work = Les travaux non enregistrés peuvent être perdus.
shell-menu-cancel = Annuler
shell-menu-log-out = Se déconnecter
shell-menu-reboot = Redémarrer
shell-menu-power-off = Éteindre

## Calendar

shell-calendar-today = Aujourd’hui
shell-calendar-today-cell = Aujourd’hui, { $date }
shell-calendar-previous-month = Mois précédent
shell-calendar-next-month = Mois suivant

## Shared control words

shell-switch-on = Activé
shell-switch-off = Désactivé
shell-status-unavailable = Indisponible
shell-action-retry = Réessayer
shell-control-description-status = { $description }, { $status }

## Notifications module

shell-notifications-do-not-disturb = Ne pas déranger
shell-dnd-until = Jusqu’à { $time }
shell-dnd-until-inline = jusqu’à { $time }
shell-dnd-scheduled-until = Programmé jusqu’à { $time }
shell-dnd-scheduled-until-inline = programmé jusqu’à { $time }
shell-dnd-until-turned-off = Jusqu’à désactivation
shell-dnd-until-turned-off-inline = jusqu’à désactivation
shell-dnd-weekday-time = { $weekday } { $time }
shell-notifications-fullscreen = Plein écran : les bannières sont enregistrées sans bruit
shell-notifications-screen-sharing = Partage d’écran : les bannières sont masquées
shell-notifications-attention = { $count ->
        [one] { $count } notification demande votre attention
       *[other] { $count } notifications demandent votre attention
    }
shell-notifications-dnd-off = Ne pas déranger désactivé
shell-notifications-dnd-on-detail = Ne pas déranger activé ({ $detail })
shell-notifications-click-help = Cliquer pour ouvrir · Clic droit pour activer ou désactiver Ne pas déranger
shell-notifications-unavailable-reconnecting = Notifications indisponibles · Reconnexion
shell-notifications-count-dnd = { $count } · Ne pas déranger
shell-notifications-unavailable = Notifications indisponibles
shell-notifications-banners-paused = Bannières suspendues · { $detail }
shell-notifications-banners-allowed = Bannières autorisées

## Idle inhibitor module

shell-idle-help-off = Maintien de l’éveil désactivé · Cliquer pour garder l’écran allumé
shell-idle-help-on = Maintien de l’éveil activé · Cliquer pour autoriser la mise en veille
shell-idle-help-pending = Mise à jour du maintien de l’éveil…
shell-idle-help-unavailable = Maintien de l’éveil indisponible · Nécessite une barre visible et la prise en charge par le compositeur
shell-idle-inhibited = Veille bloquée
shell-idle-keep-awake = Maintenir éveillé
shell-idle-paused = La mise en veille de l’écran est suspendue
shell-idle-automatic = L’écran peut se mettre en veille automatiquement
shell-idle-updating = Mise à jour…
shell-idle-stasis-unavailable = Stasis indisponible
shell-idle-compositor-unavailable = Indisponible avec ce compositeur
shell-idle-wait = Patienter
shell-idle-stasis-help = Stasis : { $state } · Profil : { $profile } · Maintien manuel : { $hold ->
        [on] activé
       *[off] désactivé
    } · Cliquer pour activer ou désactiver le maintien manuel
shell-idle-stasis-help-unavailable = Stasis indisponible · Démarrez canopa-idle et installez son service

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

## Network, sound and media cards

shell-network-wifi = Wi-Fi
shell-network-wired = Filaire
shell-network-vpn = VPN
shell-network-network = Réseau
shell-network-online = connecté à Internet
shell-network-limited = connexion limitée
shell-network-offline = hors ligne
shell-network-unknown = état inconnu
shell-network-connected = connecté
shell-network-portal = connexion au portail requise
shell-network-with-vpn = { $description } · VPN : { $vpn }
shell-network-status-unavailable = État du réseau indisponible
shell-volume-sound = Son
shell-volume-muted = Sortie coupée
shell-volume-devices = Haut-parleurs et microphones
shell-volume-unavailable = État du son indisponible
shell-volume-level = Volume de sortie
shell-volume-level-value = Volume de sortie, { $level }
shell-volume-decrease = Baisser le volume
shell-volume-increase = Augmenter le volume
shell-media-title = Multimédia
shell-tooltip-battery = { $value } % { $state }
shell-tooltip-volume = { $value } %

## Bar and panel chrome

shell-back = Retour
shell-close = Fermer
shell-search = Rechercher
shell-bar-name = Barre Canopa
shell-bar-name-on-output = Barre Canopa sur { $output }
shell-tray-menu = Menu de la zone de notification
shell-workspace-label = Espace
shell-config-alert = Erreur de configuration
shell-config-alert-description = La configuration du Shell a été refusée
shell-list-use-custom-value = Utiliser une valeur personnalisée
shell-list-use-custom-value-named = Utiliser la valeur personnalisée { $value }
shell-provider-failed = Échec du fournisseur
shell-provider-value-too-long = La valeur est trop longue. Raccourcissez-la et réessayez.
shell-provider-selected = Sélection : { $label }

## Bar accessibility (names read by screen readers)

shell-a11y-clock = Horloge
shell-a11y-battery = Batterie
shell-a11y-battery-full = complètement chargée
shell-a11y-battery-not-charging = ne charge pas
shell-a11y-battery-charging = en charge
shell-a11y-battery-discharging = en décharge
shell-a11y-battery-critical = niveau critique
shell-a11y-battery-low = niveau faible
shell-a11y-network = Réseau
shell-a11y-volume = Volume
shell-a11y-volume-muted = coupé
shell-a11y-notifications = Notifications
shell-a11y-unavailable = indisponible
shell-a11y-attention = { $count ->
        [0] aucune ne demande votre attention
        [one] { $count } demande votre attention
       *[other] { $count } demandent votre attention
    }
shell-a11y-dnd-on = Ne pas déranger activé
shell-a11y-workspace = Espace de travail { $label }
shell-a11y-workspace-focused = actif
shell-a11y-workspace-active = affiché
shell-a11y-workspace-urgent = urgent
shell-a11y-tray-item = Élément de la zone de notification
shell-a11y-tray = Zone de notification
shell-a11y-image = Image

## Notices

shell-notice-open-file = Ouvrir le fichier
shell-notice-open-settings = Ouvrir les Paramètres
shell-config-notice-summary = Erreur dans la configuration du Shell
shell-config-notice-defaults = Canopa Shell a démarré avec sa disposition intégrée. Corrigez le fichier et enregistrez-le pour appliquer votre configuration.
shell-config-notice-previous = Canopa Shell a conservé la dernière configuration valide. Corrigez le fichier et enregistrez-le pour appliquer vos modifications.

## Session actions (suspend, hibernate, reboot, power off)

shell-session-failed = { $action ->
        [suspend] Impossible de mettre en veille
        [hibernate] Impossible de mettre en hibernation
        [reboot] Impossible de redémarrer
       *[power-off] Impossible d’éteindre
    }
shell-session-not-authorized = { $action ->
        [suspend] Vous n’êtes pas autorisé à mettre en veille maintenant, ou l’authentification a été annulée.
        [hibernate] Vous n’êtes pas autorisé à mettre en hibernation maintenant, ou l’authentification a été annulée.
        [reboot] Vous n’êtes pas autorisé à redémarrer maintenant, ou l’authentification a été annulée.
       *[power-off] Vous n’êtes pas autorisé à éteindre maintenant, ou l’authentification a été annulée.
    }
shell-session-logind-unavailable = Le gestionnaire de connexion (systemd-logind) n’est pas disponible sur le bus système.
shell-session-blocked = { $count ->
        [one] Bloqué par { $blockers }. Terminez ou fermez cette application, puis réessayez.
       *[other] Bloqué par { $blockers }. Terminez ou fermez ces applications, puis réessayez.
    }
shell-session-blocked-by = Bloqué par { $blockers }.
shell-session-timed-out = Le gestionnaire de connexion n’a pas répondu à temps.
shell-session-more-blockers = { $count ->
        [one] { $count } autre
       *[other] { $count } autres
    }

## On-screen display

shell-osd-name = Affichage à l’écran
shell-osd-volume = Volume
shell-osd-microphone = Microphone
shell-osd-brightness = Luminosité
shell-osd-keyboard-brightness = Luminosité du clavier
shell-osd-muted = Coupé
shell-osd-off = Éteint
shell-osd-unavailable = Indisponible
shell-osd-announce-muted = coupé
shell-osd-announce-off = éteint
shell-osd-announce-unavailable = indisponible
shell-osd-announcement = { $channel } { $value }
shell-osd-announcement-device = { $channel } { $value }, { $device }

## Sound provider

shell-audio-title = Son
shell-audio-unavailable = Le son est indisponible. Reconnexion…
shell-audio-outputs = Haut-parleurs et casques
shell-audio-no-outputs = Aucun périphérique de sortie n’est disponible.
shell-audio-inputs = Microphones
shell-audio-no-inputs = Aucun périphérique d’entrée n’est disponible.
shell-audio-output-volume = Volume de sortie
shell-audio-set-output = Utiliser comme sortie
shell-audio-default-output = Sortie par défaut
shell-audio-input-level = Niveau d’entrée
shell-audio-set-input = Utiliser comme entrée
shell-audio-default-input = Entrée par défaut
shell-audio-mute = Couper le son
shell-audio-unmute = Rétablir le son
shell-audio-muted = Son coupé
shell-audio-applying = Application de la modification…
shell-audio-muted-applying = Son coupé · Application de la modification…
shell-audio-too-many-changes = Trop de modifications du son sont en attente. Veuillez réessayer.

## Media provider

shell-media-now-playing = En cours de lecture
shell-media-automatic = Automatique
shell-media-player = Lecteur
shell-media-player-option = { $player } · { $status }
shell-media-playing = En lecture
shell-media-paused = En pause
shell-media-stopped = Arrêté
shell-media-play = Lire
shell-media-pause = Pause
shell-media-previous = Précédent
shell-media-next = Suivant
shell-media-no-players = Aucun lecteur multimédia n’est ouvert.
shell-media-unavailable = Le multimédia est indisponible. Reconnexion…
shell-media-timed-out-retry = La commande multimédia n’a pas répondu à temps. Veuillez réessayer.
shell-media-busy = Les commandes multimédias sont occupées
shell-media-timed-out = La commande multimédia n’a pas répondu à temps
shell-media-disconnected = Les commandes multimédias sont déconnectées
shell-media-player-gone = Le lecteur n’est plus disponible
shell-media-expired = La demande multimédia a expiré ; réessayez

## Wi-Fi provider

shell-wifi-title = Wi-Fi
shell-wifi-unavailable = Wi-Fi indisponible
shell-wifi-check-again = Vérifier à nouveau
shell-wifi-set-up-in-settings = Configurer dans les Paramètres
shell-wifi-enterprise-in-settings = Connectez-vous aux réseaux WPA Entreprise dans les Paramètres, page Réseau.
shell-wifi-credential-required = Identifiant requis
shell-wifi-credential-rejected = Identifiant refusé
shell-wifi-off = Le Wi-Fi est désactivé
shell-wifi-turn-on = Activer le Wi-Fi
shell-wifi-turn-off = Désactiver le Wi-Fi
shell-wifi-connecting = Connexion à { $network }
shell-wifi-connected = Connecté à { $network }
shell-wifi-scanning-networks = Recherche de réseaux Wi-Fi
shell-wifi-choose = Choisissez un réseau Wi-Fi
shell-wifi-disconnect = Se déconnecter
shell-wifi-forget = Oublier { $network }
shell-wifi-problem = Problème de Wi-Fi
shell-wifi-credential-title = Identifiant du réseau
shell-wifi-credential-prompt = Saisissez l’identifiant de { $network }
shell-wifi-credential-label = Identifiant du réseau
shell-wifi-connect = Se connecter
shell-wifi-scanning = Recherche
shell-wifi-refresh = Actualiser
shell-wifi-error-target-changed = Le réseau sélectionné a changé. Sélectionnez-le à nouveau.
shell-wifi-error-no-control = Vérifiez que NetworkManager ou iwd est en cours d’exécution, puis réessayez.
shell-wifi-error-no-backend = Aucun service de contrôle du Wi-Fi pris en charge n’est disponible.
shell-wifi-error-scan-failed = La recherche de réseaux Wi-Fi a échoué.
shell-wifi-error-connection-failed = La connexion a échoué.
shell-wifi-error-credential-rejected = L’identifiant du réseau a été refusé.
shell-wifi-error-unsupported = Ce réseau Wi-Fi n’est pas pris en charge.
shell-wifi-error-authorization-required = Une autorisation est requise pour le Wi-Fi.
shell-wifi-network-description = { $known ->
        [yes] connu, { $security ->
            [open] ouvert
            [enterprise] entreprise
           *[secured] sécurisé
        }, { $signal }
       *[no] { $security ->
            [open] ouvert
            [enterprise] entreprise
           *[secured] sécurisé
        }, { $signal }
    }

## Bluetooth panel

shell-bluetooth-title = Bluetooth
shell-bluetooth-no-adapters = Aucun adaptateur Bluetooth trouvé.
shell-bluetooth-unavailable = Le service Bluetooth est indisponible.
shell-bluetooth-searching = Recherche de périphériques…
shell-bluetooth-turn-on = Activer
shell-bluetooth-turn-off = Désactiver
shell-bluetooth-find-devices = Rechercher des périphériques
shell-bluetooth-stop-searching = Arrêter la recherche
shell-bluetooth-connected = Connecté
shell-bluetooth-paired = Associé
shell-bluetooth-available = Disponible
shell-bluetooth-connect = Se connecter
shell-bluetooth-disconnect = Se déconnecter
shell-bluetooth-pair = Associer
shell-bluetooth-confirm-code = L’autre appareil affiche-t-il { $code } ?
shell-bluetooth-authorize = Autoriser cette demande d’association ?
shell-bluetooth-service = Autoriser cet appareil à se connecter ?
shell-bluetooth-enter-pin = Saisissez le code PIN affiché par l’autre appareil.
shell-bluetooth-enter-passkey = Saisissez la clé à six chiffres affichée par l’autre appareil.
shell-bluetooth-type-pin = Tapez { $pin } sur l’autre appareil et appuyez sur Entrée.
shell-bluetooth-type-passkey = Tapez { $passkey } sur l’autre appareil et appuyez sur Entrée ({ $entered ->
        [one] { $entered } chiffre saisi sur 6
       *[other] { $entered } chiffres saisis sur 6
    }).
shell-bluetooth-pair-with = Associer avec { $device }
shell-bluetooth-pairing-code = Code d’association
shell-bluetooth-invalid-code = Saisissez un code d’association valide.
shell-bluetooth-waiting = En attente de l’appareil…
shell-bluetooth-cancel-pairing = Annuler l’association
shell-bluetooth-confirm = Confirmer
shell-bluetooth-pairing-canceled = Association annulée.
shell-bluetooth-change-failed = Le Bluetooth n’a pas pu appliquer la modification.
shell-bluetooth-pairing-failed = L’association ou la connexion Bluetooth a échoué.

## Power and brightness panel

shell-power-title = Alimentation et luminosité
shell-power-built-in-display = Écran intégré
shell-power-brightness-unavailable = La luminosité de l’écran est indisponible dans cette session.
shell-power-profile = Mode d’alimentation
shell-power-profile-active = Actif : { $profile }
shell-power-saver = Économie d’énergie
shell-power-balanced = Équilibré
shell-power-performance = Performances
shell-power-profiles-unavailable = Le service des modes d’alimentation est indisponible.
shell-power-change-failed = Le système n’a pas pu appliquer la modification.

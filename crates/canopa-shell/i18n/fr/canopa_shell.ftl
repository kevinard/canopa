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
shell-provider-bluetooth-description = Appareils et association
shell-bluetooth-on = Activé, aucun appareil connecté
shell-bluetooth-connected-count = { $count } appareils connectés
shell-provider-power-description = Batterie, mode d’alimentation et luminosité
shell-provider-wifi-description = Réseaux Wi-Fi et connexions
shell-tile-details = Réglages { $name }
shell-battery-title = Batterie
shell-battery-charging = En charge
shell-battery-full = Complètement chargée
shell-battery-plugged-in = Branchée, pas en charge
shell-battery-on-battery = Sur batterie
shell-module-lock = Verrouiller
shell-module-suspend = Mettre en veille
shell-module-hibernate = Hiberner
shell-module-log-out = Se déconnecter
shell-module-reboot = Redémarrer
shell-module-power-off = Éteindre
shell-module-power = Alimentation
shell-tooltip-launcher = Lanceur
shell-tooltip-overview = Vue d’ensemble
shell-tooltip-quick-settings = Réglages rapides
shell-tooltip-workspace = Espace de travail { $index }

## Built-in menus (config/default.kdl `title-id`, `prompt-id`, `label-id`)

shell-menu-launcher-prompt = Rechercher des applications, des fichiers et plus
shell-menu-log-out-title = Se déconnecter ?
shell-menu-reboot-title = Redémarrer ?
shell-menu-power-off-title = Éteindre ?
shell-menu-power-title = Alimentation
shell-menu-log-out-more = Se déconnecter…
shell-menu-reboot-more = Redémarrer…
shell-menu-power-off-more = Éteindre…
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
shell-volume-mute = Couper le son
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
shell-a11y-taskbar = Barre des tâches
shell-a11y-taskbar-pinned = épinglée
shell-a11y-taskbar-focused = au premier plan
shell-taskbar-windows = { $count ->
    [one] 1 fenêtre
   *[other] { $count } fenêtres
}
shell-taskbar-open = Ouvrir
shell-taskbar-new-window = Nouvelle fenêtre
shell-taskbar-pin = Épingler à la barre des tâches
shell-taskbar-unpin = Détacher de la barre des tâches
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

## Launcher: scope keywords

shell-keyword-apps = applis applications
shell-keyword-files = fichiers
shell-keyword-windows = fenêtres fenetres
shell-keyword-settings = paramètres parametres
shell-keyword-clipboard = presse-papiers pp
shell-keyword-snippets = extraits
shell-keyword-emoji = émoji émojis emojis
shell-keyword-symbol = symbole symboles

## Launcher: window, search field and results

shell-launcher-scope-all = Tout
shell-launcher-scope-apps = Applications
shell-launcher-scope-files = Fichiers
shell-launcher-scope-windows = Fenêtres
shell-launcher-scope-settings = Paramètres
shell-launcher-search-apps = Rechercher des applications
shell-launcher-search-files = Rechercher des fichiers
shell-launcher-search-windows = Rechercher des fenêtres
shell-launcher-search-settings = Rechercher des paramètres
shell-launcher-close = Fermer le lanceur
shell-launcher-close-hint = Fermer le lanceur · { $key }
shell-launcher-clear = Effacer
shell-launcher-clear-search = Effacer la recherche
shell-launcher-clear-search-hint = Effacer la recherche · { $key }
shell-launcher-calculator = Calculatrice
shell-launcher-calculate-try = Essayez
shell-launcher-calculate-or = ou
shell-launcher-select = Sélectionner
shell-launcher-installed-apps = Applications installées
shell-launcher-browse = Parcourir
shell-launcher-results = Résultats
shell-launcher-actions = Actions
shell-launcher-actions-for = Actions pour { $name }
shell-launcher-open = Ouvrir
shell-launcher-open-named = Ouvrir { $name }
shell-launcher-launch = Lancer { $name }
shell-launcher-no-matches = Aucun résultat
shell-launcher-advice-scope = Essayez un autre terme ou choisissez Tout.
shell-launcher-advice-spelling = Vérifiez l’orthographe ou essayez une recherche plus courte.
shell-launcher-match-count = { $count ->
        [one] { $count } résultat
       *[other] { $count } résultats
    }
shell-launcher-match-count-approximate = { $count } résultats · { $approximate ->
        [one] { $approximate } approximatif
       *[other] { $approximate } approximatifs
    }
shell-launcher-hint-unpin = Retirer ce résultat des suggestions du lanceur
shell-launcher-hint-pin = Garder ce résultat dans les suggestions du lanceur
shell-launcher-hint-alias = Trouver ce résultat avec un terme de recherche personnalisé
shell-key-escape = Échap
shell-key-enter = Entrée
shell-key-tab = Tab
shell-key-ctrl-digit = Ctrl+{ $digit }

## Launcher: sessions, apps and status

shell-launcher-application = Application
shell-launcher-application-action = Action de l’application
shell-launcher-matched-command = Correspond au fichier ou à la commande de l’application
shell-launcher-matched-description = Correspond à la description de l’application
shell-launcher-matched-keywords = Correspond aux mots-clés de l’application
shell-launcher-approximate-match = Correspondance approximative des mots-clés
shell-launcher-open-with-path = Ouvrir avec · { $path }
shell-launcher-open-with-file = Ouvrir avec — { $file }
shell-launcher-recent = Récent
shell-launcher-filter-actions = Filtrer les actions
shell-launcher-filter-applications = Filtrer les applications
shell-launcher-snippet-name = Nom de l’extrait
shell-launcher-alias-for = Alias de { $name }
shell-launcher-applying-action = Exécution de l’action…
shell-launcher-snippet-name-help = Saisissez un nom d’extrait et appuyez sur Entrée pour l’enregistrer
shell-launcher-alias-help = Saisissez un alias unique d’un seul mot, puis Entrée · laisser vide supprime l’alias
shell-launcher-query-too-long = La recherche est limitée à { $bytes } octets
shell-launcher-enter-expression = Saisissez une expression arithmétique
shell-launcher-enter-file-name = Saisissez un nom de fichier ou de dossier
shell-launcher-finding-applications = Recherche des applications…
shell-launcher-calculating = Calcul…
shell-launcher-top-matches = Affichage des applications les mieux classées.
shell-launcher-indexing-files = Indexation des noms de fichiers…
shell-launcher-searching-files = Recherche de fichiers…
shell-launcher-searching-extension = Recherche dans { $name }…
shell-launcher-application-gone = L’application n’est plus disponible
shell-launcher-could-not-start = Impossible de démarrer { $name } : { $error }
shell-launcher-action-failed = Impossible d’exécuter l’action ; vérifiez que l’application ou le fichier est disponible
shell-launcher-more-matches = D’autres résultats existent ; affinez votre recherche
shell-launcher-query-long = La recherche est trop longue
shell-launcher-extension-error = { $name } : { $error }
shell-launcher-preferences-stopped = Le service des préférences du lanceur s’est arrêté
shell-launcher-remove-alias = Supprimer l’alias
shell-launcher-save-alias = Enregistrer l’alias
shell-launcher-save-alias-named = Enregistrer l’alias : { $alias }
shell-launcher-pinned = Épinglé
shell-launcher-shortcut = Raccourci
shell-launcher-pinned-alias = Épinglé · { $alias }
shell-launcher-shortcut-alias = Raccourci · { $alias }
shell-launcher-switch-to = Basculer vers
shell-launcher-new-window = Nouvelle fenêtre
shell-launcher-running-application = Application en cours d’exécution
shell-launcher-unpin = Détacher du lanceur
shell-launcher-pin = Épingler dans le lanceur
shell-launcher-set-alias = Définir un alias de recherche…
shell-launcher-edit-alias = Modifier l’alias de recherche…
shell-session-confirmed-label = { $action }…
shell-session-row = Session
shell-session-row-confirms = Session · demande une confirmation
shell-session-lock-keywords = verrouiller verrouillage écran ecran session
shell-session-log-out-keywords = déconnecter deconnecter déconnexion deconnexion quitter session
shell-session-suspend-keywords = veille suspendre
shell-session-hibernate-keywords = hiberner hibernation veille disque
shell-session-reboot-keywords = redémarrer redemarrer redémarrage redemarrage
shell-session-power-off-keywords = éteindre eteindre arrêter arreter arrêt arret extinction

## Launcher: windows, files and controls

shell-window-workspace = Espace de travail { $workspace }
shell-window-workspace-on-output = Espace de travail { $workspace } · { $output }
shell-window-workspace-unavailable = Espace de travail indisponible
shell-window-switch = Basculer vers la fenêtre
shell-window-launch-application = Lancer l’application
shell-window-description = Fenêtre · { $application } · { $workspace }
shell-files-index-partial = L’index des fichiers est partiel (limite d’analyse atteinte)
shell-files-locations-unreadable = Certains emplacements de recherche n’ont pas pu être lus
shell-files-top-matches = Affichage des fichiers les mieux classés.
shell-files-folder-in = Dossier · { $parent }
shell-files-file-in = Fichier · { $parent }
shell-files-copy-path = Copier le chemin
shell-files-open-folder = Ouvrir le dossier parent
shell-files-open-with = Ouvrir avec…
shell-files-quick-look = Aperçu rapide
shell-control-description = Paramètres et commandes
shell-control-wifi = Réseaux Wi-Fi
shell-control-sound = Son et volume
shell-control-notifications = Notifications et Ne pas déranger
shell-control-settings = Paramètres de Canopa
shell-control-clear-history = Effacer l’historique du lanceur
shell-control-bluetooth = Périphériques Bluetooth
shell-control-power = Alimentation et luminosité
shell-control-wifi-keywords = réseau reseau sans-fil internet connexion
shell-control-sound-keywords = son volume haut-parleur microphone sortie entrée
shell-control-notifications-keywords = notifications ne pas déranger alertes centre
shell-control-settings-keywords = paramètres parametres préférences preferences configuration bureau
shell-control-clear-history-keywords = effacer oublier réinitialiser historique récents fréquents
shell-control-bluetooth-keywords = bluetooth casque écouteurs associer reconnecter
shell-control-power-keywords = luminosité luminosite rétroéclairage performances équilibré économie énergie alimentation mode
shell-settings-page = Page des Paramètres
shell-settings-page-label = { $page } — Paramètres
shell-settings-not-installed = Les Paramètres de Canopa ne sont pas installés
shell-settings-pages-timed-out = Les Paramètres de Canopa n’ont pas listé leurs pages à temps
shell-settings-pages-failed = Les Paramètres de Canopa n’ont pas pu lister leurs pages
shell-settings-pages-too-large = La liste des pages des Paramètres est trop longue
shell-settings-pages-invalid = La liste des pages des Paramètres n’est pas valide
shell-settings-no-such-page = Les Paramètres n’ont pas cette page

## Launcher: calculator, characters and extensions

shell-calculator-too-long = Les calculs sont limités à { $bytes } octets
shell-calculator-target-unit = Choisissez une unité cible prise en charge
shell-calculator-amount = Saisissez une quantité numérique complète
shell-calculator-source-unit = Choisissez une unité source prise en charge
shell-calculator-dimensions = Ces unités mesurent des grandeurs différentes
shell-calculator-incomplete = Saisissez une expression arithmétique ou une conversion d’unités complète
shell-calculator-no-result-in-unit = L’expression n’a pas de résultat réel fini dans l’unité demandée
shell-calculator-no-result = L’expression n’a pas de résultat réel fini
shell-calculator-description = Calculatrice · { $query }
shell-calculator-copy-result = Copier le résultat
shell-calculator-failed = Calculatrice : { $error } (nécessite qalc)
shell-characters-help = Recherchez par nom ou mot-clé · Entrée pour copier
shell-provider-unavailable = L’exécutable du fournisseur est indisponible
shell-provider-io-unavailable = L’entrée ou la sortie du fournisseur est indisponible
shell-provider-io-failed = L’entrée ou la sortie du fournisseur a échoué
shell-provider-output-too-large = La sortie du fournisseur a dépassé 256 Kio
shell-provider-process-failed = Le processus du fournisseur a échoué
shell-provider-exit-failed = Le fournisseur s’est terminé avec une erreur
shell-provider-timed-out = Le fournisseur n’a pas répondu à temps
shell-extension-web = Web
shell-extension-date-time = Date et heure
shell-extension-web-search = Web · { $keyword }
shell-extension-duplicate-keyword = Mot-clé d’extension en double : { $keyword }
shell-extension-duplicate-search = Mot-clé de recherche en double : { $keyword }
shell-extension-web-description = Collez une URL HTTP(S) ou utilisez le mot-clé pour rechercher. Les actions du navigateur ne s’exécutent qu’une fois sélectionnées.
shell-extension-date-time-description = Dates locales, fuseaux horaires et horodatages Unix. Essayez (en anglais) time in Tokyo, 90 days from today ou timestamp now.
shell-extension-configured-description = Extension exécutable configurée. Ses actions apparaissent ici une fois qu’elle a renvoyé des résultats.
shell-extension-open-in-browser = Ouvrir dans le navigateur
shell-extension-copy-url = Copier l’URL
shell-extension-search-web = Rechercher sur le web : { $query }
shell-extension-web-result = Ouvrir dans votre navigateur par défaut · les requêtes partent une fois sélectionné
shell-extension-time-in = Heure à { $zone }
shell-extension-unix-timestamp = Horodatage Unix · secondes
shell-extension-timestamp-to-utc = Horodatage Unix en UTC
shell-extension-date-to-timestamp = Date en horodatage Unix · secondes
shell-extension-calendar-date = Date du calendrier · fuseau horaire local

## Launcher: clipboard history and snippets

shell-clipboard-copy = Copier
shell-clipboard-remove = Retirer de l’historique
shell-clipboard-recent-copy = Copie récente
shell-clipboard-save-as-snippet = Enregistrer comme extrait…
shell-clipboard-image = Image
shell-clipboard-image-named = Image { $description }
shell-clipboard-image-description = { $format } · { $width } × { $height }
shell-clipboard-clear-history = Effacer l’historique du presse-papiers
shell-clipboard-clear = Effacer l’historique
shell-clipboard-turn-off = Désactiver l’historique du presse-papiers
shell-clipboard-turn-on = Activer l’historique du presse-papiers
shell-clipboard-keep-recent = Conserver les copies récentes pendant cette session
shell-clipboard-rename-snippet = Renommer l’extrait…
shell-clipboard-delete-snippet = Supprimer l’extrait
shell-clipboard-new-snippet = Nouvel extrait depuis le presse-papiers
shell-clipboard-name-snippet = Nommer l’extrait…
shell-clipboard-loading = Chargement des préférences du presse-papiers…
shell-clipboard-history-off = L’historique est désactivé · tapez snip pour les extraits enregistrés
shell-clipboard-recording-text = Enregistrement du texte pour cette session · Entrée pour copier · Tab pour les actions
shell-clipboard-recording-text-images = Enregistrement du texte et des images pour cette session · Entrée pour copier · Tab pour les actions
shell-clipboard-snippets-help = Extraits enregistrés · Entrée pour copier · Tab pour renommer ou supprimer
shell-clipboard-help = Entrée pour copier · Tab pour les actions
shell-clipboard-save-snippet-named = Enregistrer l’extrait « { $name } »
shell-clipboard-save-snippet = Enregistrer l’extrait
shell-clipboard-entry-gone = Cette entrée du presse-papiers n’est plus disponible
shell-clipboard-copy-text-first = Copiez du texte (16 Kio au maximum) avant de créer un extrait
shell-clipboard-saving = Une modification du presse-papiers est en cours d’enregistrement
shell-clipboard-preferences-unavailable = Les préférences du presse-papiers sont indisponibles
shell-clipboard-save-stopped = L’enregistrement du presse-papiers s’est arrêté
shell-clipboard-reader-failed = Impossible de démarrer le lecteur du presse-papiers
shell-clipboard-wl-paste-failed = Impossible de démarrer wl-paste pour l’historique du presse-papiers
shell-clipboard-watcher-unreadable = Impossible de lire la surveillance du presse-papiers
shell-clipboard-text-watcher-stopped = La surveillance du texte copié s’est arrêtée. Vérifiez la prise en charge du contrôle des données Wayland
shell-clipboard-image-watcher-stopped = La surveillance des images copiées s’est arrêtée. Vérifiez la prise en charge du contrôle des données Wayland
shell-clipboard-preferences-stopped = La lecture des préférences du presse-papiers s’est arrêtée
shell-clipboard-image-copy-failed = Impossible de copier l’image dans le presse-papiers
shell-history-unreadable = L’historique du lanceur n’a pas pu être lu
shell-history-invalid = L’historique du lanceur n’est pas valide
shell-history-unsaved = L’historique du lanceur n’a pas pu être enregistré
shell-history-unavailable = Le service d’historique du lanceur est indisponible
shell-history-busy = L’historique du lanceur est occupé

## Panel providers

shell-provider-event-timed-out = Le fournisseur « { $provider } » n’a pas répondu à temps
shell-provider-not-configured = Le fournisseur « { $provider } » n’est pas configuré
shell-provider-not-available = Le fournisseur « { $provider } » n’est pas disponible
shell-provider-launch-failed = Impossible de lancer le fournisseur « { $provider } » : { $error }
shell-provider-handshake-timed-out = Le fournisseur « { $provider } » n’a pas démarré à temps
shell-config-notice-construction = Le Shell n’a pas pu construire les modules configurés
shell-launcher-search-in = Rechercher dans

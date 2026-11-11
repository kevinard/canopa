# Canopa : Boîtes de dialogue des portails.
# Traduction française : assistée par machine, à relire (machine-assisted, needs review).

## Shared

portal-window-title = { $title } — Canopa
portal-cancel = Annuler
portal-loading = Chargement…
portal-combo-position = { $position } sur { $count }

## File chooser

portal-place-home = Dossier personnel
portal-place-documents = Documents
portal-place-downloads = Téléchargements
portal-place-pictures = Images
portal-place-filesystem = Système de fichiers
portal-chooser-purpose = Choisissez parmi vos fichiers et dossiers locaux
portal-chooser-up = Dossier parent
portal-chooser-location = Emplacement
portal-chooser-go = Aller
portal-chooser-new-folder = Nouveau dossier
portal-chooser-navigation = Navigation
portal-chooser-places = Raccourcis
portal-chooser-open-entry = Ouvrir { $name }
portal-chooser-open-row = Ouvrir
portal-chooser-no-matches = Aucun fichier correspondant
portal-chooser-loading-folder = Chargement du dossier…
portal-chooser-item-count = { $count ->
    [one] { $count } élément
   *[other] { $count } éléments
}
portal-chooser-files = Fichiers
portal-chooser-name-column = Nom
portal-chooser-search = Filtrer les noms dans ce dossier
portal-chooser-name-label = Nom
portal-chooser-file-name = Nom du fichier
portal-chooser-save-many = { $count ->
    [one] Choisissez un dossier pour { $count } fichier
   *[other] Choisissez un dossier pour { $count } fichiers
}
portal-chooser-file-type = Type de fichier
portal-chooser-read-only = Ouvrir en lecture seule
portal-chooser-show-hidden = Afficher les fichiers cachés
portal-chooser-accept-open = Ouvrir
portal-chooser-accept-save = Enregistrer
portal-chooser-folder-name = Nom du dossier
portal-chooser-creating = Création…
portal-chooser-create = Créer
portal-chooser-overwrite-title = Remplacer les fichiers existants ?
portal-chooser-overwrite-question = La destination existe déjà. Autoriser l’application à la remplacer ?
portal-chooser-overwrite-names = { $more ->
    [0] { $names }
    [one] { $names } et { $more } autre
   *[other] { $names } et { $more } autres
}
portal-chooser-overwrite-more = { $count ->
    [one] et { $count } autre
   *[other] et { $count } autres
}
portal-chooser-keep-choosing = Continuer à choisir
portal-chooser-replace = Remplacer
portal-chooser-kind-folder-link = Lien vers un dossier
portal-chooser-kind-link = Lien
portal-chooser-kind-folder = Dossier
portal-chooser-kind-file = Fichier
portal-chooser-kind-typed-file = Fichier { $extension }
portal-chooser-kind-hidden = { $kind }, caché
portal-chooser-choose-existing-folder = Choisissez un dossier existant
portal-chooser-select-file = Sélectionnez un fichier
portal-chooser-select-type = Sélectionnez le type de fichier demandé
portal-chooser-one-file-name = Saisissez un seul nom de fichier, sans chemin de dossier
portal-chooser-choose-folder = Choisissez un dossier
portal-chooser-invalid-name = Nom de fichier non valide
portal-chooser-distinct-names = Les noms des fichiers à enregistrer doivent être différents
portal-chooser-special-destination = La destination est un dossier ou un fichier spécial
portal-chooser-destination-changed = Le dossier de destination a changé. Choisissez-le à nouveau.
portal-chooser-absolute-folder = Choisissez un chemin de dossier absolu
portal-chooser-folder-changed = L’emplacement du dossier a changé. Choisissez-le à nouveau.
portal-chooser-folder-name-rule = Saisissez un seul nom de dossier, sans barre oblique, de 255 octets au maximum

## Application chooser

portal-app-chooser-title = Ouvrir avec
portal-app-chooser-caption = Application
portal-app-chooser-heading = Choisissez une application pour { $item }
portal-app-chooser-heading-any = Choisissez une application pour cet élément
portal-app-chooser-loading = Chargement des applications…
portal-app-chooser-none = Aucune application disponible
portal-app-chooser-no-matches = Aucune application correspondante
portal-app-chooser-count = { $count ->
    [one] { $count } application
   *[other] { $count } applications
}
portal-app-chooser-search = Rechercher des applications
portal-app-chooser-applications = Applications
portal-app-chooser-open = Ouvrir

## Access

portal-access-title = Autorisation
portal-access-deny = Refuser
portal-access-allow = Autoriser

portal-requesting-app = L’application demandeuse
portal-requested-by = Demandé par { $requester }
portal-pixel-size = { $width } × { $height } pixels
portal-share = Partager

## Screenshot

portal-screenshot-window = Capture d’écran
portal-screenshot-color-window = Choisir une couleur
portal-screenshot-title = Partager une capture d’écran
portal-screenshot-color-title = Choisir une couleur
portal-screenshot-screen-hint = Tous les écrans seront partagés.
portal-screenshot-window-hint = Seule cette fenêtre sera partagée.
portal-screenshot-area-hint = Faites glisser pour sélectionner une zone. Utilisez les flèches et Espace pour sélectionner au clavier.
portal-screenshot-color-hint = Cliquez sur un pixel, puis utilisez la couleur. Les flèches l’ajustent ; Maj le déplace de dix pixels.
portal-screenshot-mode-screen = Écran
portal-screenshot-mode-area = Zone
portal-screenshot-mode-window = Fenêtre (Alt+W)
portal-screenshot-pixel = { $color }  ·  { $x }, { $y }  ·  { $display }
portal-screenshot-next-display = Écran suivant
portal-screenshot-too-large = Choisissez une zone plus petite ; cette image dépasse la taille maximale.
portal-screenshot-select-area = Sélectionnez une zone à partager.
portal-screenshot-use-color = Utiliser la couleur

portal-display-numbered = Écran { $number }

portal-an-application = Une application

## Screen sharing and remote desktop consent

portal-screencast-title = { $kind ->
    [screenshot] Choisir une fenêtre à capturer
    [clipboard] Autoriser le partage du presse-papiers
    [desktop] Autoriser le partage du bureau
    [keyboard] Autoriser le contrôle au clavier
    [pointer] Autoriser le contrôle du pointeur
    [control] Autoriser le contrôle du bureau
    [share-control] Partager et autoriser le contrôle
   *[screen] Partager votre écran
}
portal-screencast-action = { $action ->
    [capture-window] Capturer la fenêtre
    [share-clipboard] Partager le presse-papiers
    [allow-sharing] Autoriser le partage
    [share-screen-clipboard] Partager l’écran et le presse-papiers
    [share-control] Partager et autoriser le contrôle
    [allow-control] Autoriser le contrôle
   *[share] Partager
}
portal-screencast-shortcut = { $action ->
    [capture-window] Ctrl+Entrée pour capturer la fenêtre
    [share-clipboard] Ctrl+Entrée pour partager le presse-papiers
    [allow-sharing] Ctrl+Entrée pour autoriser le partage
    [share-screen-clipboard] Ctrl+Entrée pour partager l’écran et le presse-papiers
    [share-control] Ctrl+Entrée pour partager et autoriser le contrôle
    [allow-control] Ctrl+Entrée pour autoriser le contrôle
   *[share] Ctrl+Entrée pour partager
}
portal-screencast-keyboard = Autoriser le contrôle de votre bureau au clavier
portal-screencast-keyboard-hint = Saisir du texte et utiliser des raccourcis clavier.
portal-screencast-pointer = Autoriser le contrôle du pointeur sur votre bureau
portal-screencast-pointer-hint = Déplacer le pointeur, cliquer, glisser et faire défiler.
portal-screencast-touch = Autoriser le contrôle tactile (Alt+T)
portal-screencast-touch-hint = Toucher et balayer dans l’écran ou la fenêtre que vous partagez.
portal-screencast-clipboard = Partager le contenu du presse-papiers (Alt+C)
portal-screencast-clipboard-hint = Lire et remplacer le texte, les images et les autres données copiés.
portal-screencast-choose-display = Choisissez un écran à partager.
portal-screencast-choose-displays = Choisissez les écrans à partager.
portal-screencast-choose-window = Choisissez une fenêtre à partager.
portal-screencast-choose-windows = Choisissez les fenêtres à partager.
portal-screencast-choose-source = Choisissez un écran ou une fenêtre à partager.
portal-screencast-choose-sources = Choisissez les écrans ou fenêtres à partager.
portal-screencast-search = Rechercher des écrans et des fenêtres
portal-screencast-desktop-unavailable = Bureau indisponible. Nouvelle tentative…
portal-screencast-sources-unavailable = Sources indisponibles. Nouvelle tentative…
portal-screencast-clipboard-allowed = Partage du presse-papiers autorisé
portal-screencast-clipboard-undecided = Choisissez si vous partagez votre presse-papiers
portal-screencast-keyboard-allowed = Contrôle au clavier autorisé
portal-screencast-pointer-allowed = Contrôle du pointeur autorisé
portal-screencast-keyboard-pointer-allowed = Contrôle au clavier et du pointeur autorisé
portal-screencast-control-undecided = Choisissez si vous autorisez le contrôle
portal-screencast-selected = { $count ->
    [one] { $count } sélectionné
   *[other] { $count } sélectionnés
}
portal-screencast-selected-of = { $count ->
    [one] { $count } sélectionné · { $max } au maximum
   *[other] { $count } sélectionnés · { $max } au maximum
}

portal-screencast-display = Écran
portal-screencast-display-subtitle = Écran · { $model }
portal-screencast-untitled-window = Fenêtre sans titre
portal-screencast-application-window = Fenêtre d’application

## Sharing controls, shown while a screen, the input or the clipboard is shared

portal-sharing-title = { $kind ->
    [input-capture] Capture des entrées
    [clipboard-sharing] Partage du presse-papiers
    [desktop-sharing] Partage du bureau
    [remote-control] Contrôle à distance
   *[screen-sharing] Partage d’écran
}
portal-sharing-heading = { $kind ->
    [input-capture-is-allowed] La capture des entrées est autorisée
    [clipboard-sharing-is-active] Le partage du presse-papiers est actif
    [desktop-sharing-is-active] Le partage du bureau est actif
    [keyboard-control-is-active] Le contrôle au clavier est actif
    [pointer-control-is-active] Le contrôle du pointeur est actif
    [keyboard-and-pointer-control-are-active] Le contrôle au clavier et du pointeur est actif
    [sharing-and-control-are-active] Le partage et le contrôle sont actifs
   *[screen-sharing-is-active] Le partage d’écran est actif
}
portal-sharing-stop = { $kind ->
    [stop-input-and-clipboard-sharing] Arrêter le partage des entrées et du presse-papiers
    [stop-input-capture] Arrêter la capture des entrées
    [stop-all-sharing] Arrêter tous les partages
    [stop-control] Arrêter le contrôle
    [stop-sharing-and-control] Arrêter le partage et le contrôle
   *[stop-sharing] Arrêter le partage
}
portal-sharing-hint = { $kind ->
    [capture-clipboard] Ctrl+Alt+Échap suspend le partage des entrées et du presse-papiers. Arrêter ou fermer met fin aux deux.
    [capture] Ctrl+Alt+Échap suspend la capture. Arrêter ou fermer met fin à cette session.
    [clipboard] Échap ou la fermeture de cette fenêtre arrête le partage du presse-papiers
    [all] Échap ou la fermeture de cette fenêtre arrête tous les partages
    [control] Échap ou la fermeture de cette fenêtre arrête le contrôle
    [sharing-control] Échap ou la fermeture de cette fenêtre arrête le partage et le contrôle
   *[sharing] Échap ou la fermeture de cette fenêtre arrête le partage
}
portal-sharing-control = { $kind ->
    [keyboard-input-screen-edges] Saisie au clavier · Bords de l’écran
    [pointer-input-screen-edges] Pointeur · Bords de l’écran
    [keyboard-and-pointer-input-screen-edges] Clavier et pointeur · Bords de l’écran
    [keyboard-control-desktop] Contrôle au clavier · Bureau
    [pointer-control-desktop] Contrôle du pointeur · Bureau
    [touch-control-shared-screen-or-window] Contrôle tactile · Écran ou fenêtre partagés
    [keyboard-and-touch-control] Contrôle au clavier et tactile
    [pointer-and-touch-control] Contrôle du pointeur et tactile
    [keyboard-pointer-and-touch-control] Contrôle au clavier, du pointeur et tactile
   *[keyboard-and-pointer-control-desktop] Contrôle au clavier et du pointeur · Bureau
}
portal-sharing-clipboard-capture = Presse-papiers · Uniquement pendant la capture des entrées
portal-sharing-clipboard = Presse-papiers · Lire et remplacer le contenu copié
portal-sharing-input-to = Les entrées peuvent être envoyées à { $requester }
portal-sharing-control-to = Contrôle accordé à { $requester }
portal-sharing-with = Partage avec { $requester }

## Background activity

portal-background-window = Activité en arrière-plan
portal-background-question = Autoriser l’activité en arrière-plan ?
portal-background-explanation = Cette application s’exécute sans fenêtre ouverte. « Autoriser une fois » ne s’applique qu’à son exécution en cours. « Bloquer » arrête l’exécution en cours.
portal-background-description = { $name }. Cette application s’exécute sans fenêtre ouverte. « Autoriser une fois » ne s’applique qu’à son exécution en cours. « Bloquer » arrête l’exécution en cours.
portal-background-closing = Fermer cette question conserve l’exécution en cours sans modifier son autorisation.
portal-background-block = Bloquer
portal-background-allow-once = Autoriser une fois
portal-background-allow = Autoriser
portal-background-notification-title = { $name } s’exécute en arrière-plan
portal-background-notification-body = Autoriser { $app } à continuer de s’exécuter sans fenêtre ouverte ? Choisissez « Autoriser une fois » pour son exécution en cours uniquement.

portal-requested-by-application = Demandé par une application

## Print

portal-print-title = Imprimer
portal-print-save-title = Enregistrer le document imprimé
portal-print-document = Document
portal-print-accept = Imprimer
portal-print-preparing-picker = Préparation du sélecteur de destination…
portal-print-to-file = Imprimer dans un fichier
portal-print-choose-destination = Choisissez une destination
portal-print-destination = Destination
portal-print-refresh = Actualiser
portal-print-loading = Chargement des informations de l’imprimante…
portal-print-no-printers = Aucune imprimante n’est disponible. Configurez une imprimante, puis actualisez.
portal-print-not-accepting = Cette imprimante n’accepte pas de travaux.
portal-print-stopped = Cette imprimante est arrêtée. Les travaux envoyés peuvent attendre dans sa file.
portal-print-busy = Cette imprimante est occupée. Les travaux envoyés seront mis en file d’attente.
portal-print-idle = Les travaux d’impression sont envoyés dans la file de cette imprimante.
portal-print-printer-default = Valeur par défaut de l’imprimante
portal-print-media = { $name } — { $width } × { $height } mm
portal-print-one-sided = Recto
portal-print-two-sided-long = Recto verso, bord long
portal-print-two-sided-short = Recto verso, bord court
portal-print-color = Couleur
portal-print-monochrome = Monochrome
portal-print-custom-paper = Format de papier personnalisé
portal-print-width = Largeur (mm)
portal-print-height = Hauteur (mm)
portal-print-check-paper = Vérifier le format de papier
portal-print-paper-confirmed = Papier confirmé. Marges (mm) : haut { $top }, bas { $bottom }, gauche { $left }, droite { $right }.
portal-print-custom-paper-hint = Saisissez des millimètres avec deux décimales au plus, puis vérifiez ce format avec l’imprimante.
portal-print-paper = Papier
portal-print-sides = Faces
portal-print-check-settings = Vérifier les paramètres de l’imprimante
portal-print-copies-range = L’imprimante accepte de { $minimum } à { $maximum } copies.
portal-print-copies-unknown = L’imprimante n’a pas indiqué de nombre de copies pris en charge. Actualisez pour réessayer.
portal-print-file-format = Format de fichier
portal-print-choose-file-hint = Choisissez où enregistrer le document imprimé.
portal-print-choose-file = Choisir un fichier…
portal-print-paper-letter = Lettre US
portal-print-paper-legal = Légal US
portal-print-custom-size = Format personnalisé
portal-print-margin-top = Marge haute (mm)
portal-print-margin-bottom = Marge basse (mm)
portal-print-margin-left = Marge gauche (mm)
portal-print-margin-right = Marge droite (mm)
portal-print-all-pages = Toutes les pages
portal-print-page-ranges = Plages de pages
portal-print-current-page = Page courante
portal-print-selection = Sélection
portal-print-pages = Pages
portal-print-page-ranges-field = Plages de pages (par exemple 1-3, 5)
portal-print-copies = Copies
portal-print-scale = Échelle (%)
portal-print-collate = Assembler les copies
portal-print-reverse = Ordre inversé
portal-print-orientation = Orientation
portal-print-portrait = Portrait
portal-print-landscape = Paysage
portal-print-reverse-portrait = Portrait inversé
portal-print-reverse-landscape = Paysage inversé
portal-print-page-set = Pages à imprimer
portal-print-page-set-all = Toutes
portal-print-even-pages = Pages paires
portal-print-odd-pages = Pages impaires
portal-print-pages-per-sheet = Pages par feuille
portal-print-arrangement = Disposition des pages
portal-print-arrangement-order = { $order ->
    [lrbt] De gauche à droite, de bas en haut
    [rltb] De droite à gauche, de haut en bas
    [rlbt] De droite à gauche, de bas en haut
    [tblr] De haut en bas, de gauche à droite
    [tbrl] De haut en bas, de droite à gauche
    [btlr] De bas en haut, de gauche à droite
    [btrl] De bas en haut, de droite à gauche
   *[lrtb] De gauche à droite, de haut en bas
}
portal-print-check-values = Vérifiez les copies, l’échelle, les plages de pages et les dimensions du papier.
portal-print-format-unavailable = Le format de fichier précédent n’est pas disponible. Vérifiez le format de sortie.
portal-print-printer-unavailable = L’imprimante précédente n’est pas disponible. Choisissez une destination.
portal-print-preferences-unavailable = Certaines préférences d’impression de l’application ne sont pas disponibles. Vérifiez les paramètres actuels de l’imprimante.
portal-print-settings-changed = Les paramètres de l’imprimante ont changé. Vérifiez-les à nouveau.
portal-print-information-changed = Les informations de l’imprimante ont changé. Actualisez la liste des imprimantes.
portal-print-settings-unconfirmed = Impossible de confirmer ces paramètres d’imprimante ensemble. Modifiez le papier ou les options du périphérique, puis vérifiez à nouveau.
portal-print-information-failed = Impossible de charger les informations de l’imprimante. Réessayez ou enregistrez dans un fichier.
portal-print-select-destination = Choisir la destination
portal-print-format-filter = Document { $format }
portal-print-picker-failed = Impossible d’ouvrir le sélecteur de destination.
portal-print-paper-source = Source du papier
portal-print-paper-type = Type de papier
portal-print-quality = Qualité d’impression
portal-print-resolution = Résolution

portal-close = Fermer

## Print recovery: retained print files and unresolved printer jobs

portal-recovery-title = Récupération d’impression
portal-recovery-status-busy = Toujours utilisé
portal-recovery-status-missing = Dossier indisponible
portal-recovery-status-incomplete = Copie incomplète
portal-recovery-status-interrupted = Interrompu
portal-recovery-status-uncertain = Résultat non confirmé
portal-recovery-status-published = Déjà publié
portal-recovery-status-unavailable = Impossible de vérifier les fichiers
portal-recovery-explain-busy = Un processus d’impression utilise peut-être encore ces fichiers. Actualisez une fois qu’il est terminé.
portal-recovery-explain-missing = Le dossier conservé a peut-être été déplacé ou est devenu indisponible. Son dernier emplacement connu est indiqué ci-dessous.
portal-recovery-explain-incomplete = Aucun document complet n’a pu être établi. Vous pouvez examiner les fichiers conservés dans leur dossier.
portal-recovery-explain-interrupted = L’impression a été interrompue. Vous pouvez enregistrer une copie des données conservées vers une nouvelle destination.
portal-recovery-explain-uncertain = Le résultat de l’impression n’a pas pu être confirmé. Vérifiez la destination d’origine avant d’enregistrer une autre copie.
portal-recovery-explain-published = Le document a déjà été publié. Vous pouvez examiner les données conservées et enregistrer une autre copie si nécessaire.
portal-recovery-explain-unavailable = Les fichiers enregistrés ont changé ou n’ont pas pu être vérifiés. Vous pouvez examiner leur dernier dossier connu.
portal-recovery-preparing = Préparation de la copie récupérée…
portal-recovery-files-stay = Les fichiers conservés resteront en place.
portal-recovery-saving = Enregistrement de la copie récupérée…
portal-recovery-saving-hint = Les fichiers conservés resteront en place. Si vous fermez cette fenêtre, vérifiez la destination choisie avant de réessayer.
portal-recovery-checking = Vérification des travaux d’impression…
portal-recovery-checking-hint = Seuls les travaux identifiés dont l’acceptation n’est pas confirmée peuvent être annulés. Les documents ne sont jamais renvoyés.
portal-recovery-removing = Suppression de l’enregistrement de l’imprimante…
portal-recovery-removing-hint = Le travail d’impression restera inchangé.
portal-recovery-printed-document = Document imprimé
portal-recovery-printed-file = Fichier imprimé
portal-recovery-remove-printer-question = Supprimer cet enregistrement d’imprimante ?
portal-recovery-remove-printer-explain = Vérifiez votre imprimante avant d’imprimer une autre copie. Cette action supprime uniquement l’enregistrement de Canopa ; elle n’annule pas le travail d’impression et ne renvoie pas le document.
portal-recovery-remove-printer-effect = Canopa cessera de vérifier automatiquement cet enregistrement. Le supprimer libère une place dans la liste de récupération.
portal-recovery-keep = Conserver dans la liste
portal-recovery-remove-record = Supprimer l’enregistrement
portal-recovery-remove-record-ellipsis = Supprimer l’enregistrement…
portal-recovery-remove-work-question = Retirer ce travail de la liste ?
portal-recovery-remove-work-explain = Ses fichiers resteront en place. Notez l’emplacement du dossier si vous voulez les retrouver plus tard.
portal-recovery-remove-from-list = Retirer de la liste
portal-recovery-remove-from-list-ellipsis = Retirer de la liste…
portal-recovery-original-destination = Destination d’origine
portal-recovery-save-document = Enregistrer une copie du document…
portal-recovery-save-displaced = Enregistrer le fichier mis de côté…
portal-recovery-displaced-hint = Un fichier mis de côté peut contenir des données sans rapport. En enregistrer une copie le laisse en place.
portal-recovery-size-limit = Les copies jusqu’à { $size } peuvent être enregistrées ici. Utilisez le dossier pour les autres fichiers.
portal-recovery-retained-folder = Dossier conservé
portal-recovery-open-folder = Ouvrir le dossier
portal-recovery-loading = Chargement des travaux conservés…
portal-recovery-unavailable = Travaux conservés indisponibles
portal-recovery-empty = Aucun travail d’impression conservé
portal-recovery-empty-hint = Les fichiers d’impression interrompus apparaîtront ici lorsqu’ils pourront être trouvés.
portal-recovery-files-tab = Fichiers ({ $count })
portal-recovery-printers-tab = Imprimantes ({ $count })
portal-recovery-printer-unconfirmed = Résultat de l’impression non confirmé
portal-recovery-printer-accepted = Déjà accepté
portal-recovery-printer-unavailable = Impossible de vérifier l’enregistrement
portal-recovery-printer-explain-busy = Un processus d’impression utilise encore cet enregistrement. Actualisez une fois qu’il est terminé.
portal-recovery-printer-explain-unconfirmed = Canopa n’a pas pu confirmer le résultat de cette impression. Vérifiez votre imprimante avant d’imprimer une autre copie.
portal-recovery-printer-explain-accepted = Le service d’impression a déjà accepté ce document. Il est peut-être encore en cours d’impression. Supprimer cet enregistrement laisse le travail inchangé.
portal-recovery-printer-explain-unavailable = Cet enregistrement n’a pas pu être vérifié. Actualisez avant de le supprimer.
portal-recovery-printer = Imprimante
portal-recovery-server = Serveur d’impression
portal-recovery-job = Travail { $id }
portal-recovery-job-unknown = Numéro de travail indisponible
portal-recovery-printers-loading = Chargement des enregistrements d’imprimante…
portal-recovery-printers-unavailable = Enregistrements d’imprimante indisponibles
portal-recovery-printers-empty = Aucun enregistrement d’imprimante en suspens
portal-recovery-printers-empty-hint = Les enregistrements d’imprimante dont le résultat est incertain apparaîtront ici.
portal-recovery-check-all = Vérifier tous les enregistrements d’imprimante
portal-recovery-check-all-hint = Le nettoyage annule uniquement les travaux identifiés dont l’acceptation n’a pas été confirmée. Les travaux déjà acceptés ne sont pas touchés. Les documents ne sont jamais renvoyés.
portal-recovery-check-cleanup = Vérifier et nettoyer
portal-recovery-record-limit = Jusqu’à { $count } enregistrements peuvent être conservés. Les enregistrements non confirmés restent jusqu’au nettoyage ou jusqu’à ce que vous les supprimiez.
portal-recovery-printers-failed = Impossible de charger les enregistrements d’imprimante. Actualisez pour réessayer.
portal-recovery-record-removed = Enregistrement supprimé. Le travail d’impression est resté inchangé.
portal-recovery-record-removal-unconfirmed = Impossible de confirmer la suppression. L’enregistrement a peut-être changé ou est encore utilisé. Actualisez la liste.
portal-recovery-cleanup-report = Nettoyés : { $cleared } · non confirmés : { $unconfirmed } · encore utilisés : { $busy }. Vérifiez votre imprimante avant d’imprimer à nouveau.
portal-recovery-cleanup-unconfirmed = Impossible de confirmer le nettoyage. Actualisez la liste et vérifiez votre imprimante avant d’imprimer à nouveau.
portal-recovery-files-failed = Impossible de charger les travaux conservés. Actualisez pour réessayer.
portal-recovery-save-title = Enregistrer la copie récupérée
portal-recovery-save-copy = Enregistrer la copie
portal-recovery-verify-failed = Impossible de vérifier cette copie. Les fichiers conservés sont toujours disponibles dans leur dossier.
portal-recovery-choose-again = Choisissez à nouveau la destination.
portal-recovery-outside-folder = Choisissez une destination en dehors du dossier conservé.
portal-recovery-saved = Copie enregistrée dans { $path }. Les fichiers conservés sont inchangés.
portal-recovery-save-unconfirmed = Impossible de confirmer l’enregistrement. Vérifiez { $path } avant de réessayer. Les fichiers conservés sont toujours disponibles.
portal-recovery-removed = Retiré de la liste. Les fichiers conservés ont été laissés en place.
portal-recovery-removal-unconfirmed = Impossible de confirmer la suppression. Actualisez la liste.
portal-recovery-open-folder-failed = Impossible d’ouvrir le dossier conservé. Son dernier emplacement connu est indiqué ici.

## Printer jobs review

portal-jobs-title = Travaux d’impression
portal-jobs-stopped = L’examen des imprimantes s’est arrêté
portal-jobs-communication-failed = Impossible de communiquer avec le service d’examen des imprimantes.
portal-jobs-list-failed = Impossible de vérifier les travaux d’impression. Actualisez pour réessayer.
portal-jobs-signed-in = L’identification a été envoyée au service d’impression. Actualisez pour vérifier le travail ; l’impression est peut-être encore en cours.
portal-jobs-sign-in-cancelled = Identification annulée.
portal-jobs-sign-in-unconfirmed = L’identification n’a pas pu être confirmée. Actualisez et vérifiez le travail avant de réessayer.
portal-jobs-entry = Travail { $job } · { $status }
portal-jobs-existing-document = Le service d’impression a déjà ce travail. L’identification utilise son document existant.
portal-jobs-sign-in = S’identifier…
portal-jobs-identity-unconfirmed = L’identité de ce travail ou sa méthode d’identification n’a pas pu être confirmée. Actualisez pour vérifier à nouveau.
portal-jobs-unavailable = Travaux d’impression indisponibles
portal-jobs-select-printer = Sélectionnez une imprimante
portal-jobs-empty = Aucun travail d’impression actif
portal-jobs-select-job = Sélectionnez un travail d’impression
portal-jobs-held-hint = Les travaux en attente d’identification peuvent continuer avec le document déjà dans la file.
portal-jobs-status-receiving = Réception du document
portal-jobs-status-queued = En attente d’impression
portal-jobs-status-held = En attente
portal-jobs-status-needs-sign-in = Identification requise
portal-jobs-status-printing = Impression en cours
portal-jobs-status-stopped = Arrêté
portal-jobs-status-cancelled = Annulé
portal-jobs-status-aborted = Impression impossible
portal-jobs-status-completed = Terminé par le service d’impression

## Printer authentication

portal-print-auth-title = Authentification de l’imprimante
portal-print-auth-closing = Fermeture de l’authentification…
portal-print-auth-server-realm = Identifiez-vous auprès du serveur d’impression { $server } ({ $realm }).
portal-print-auth-server = Identifiez-vous auprès du serveur d’impression { $server }.
portal-print-auth-queue = Identifiants pour l’imprimante { $printer } sur { $server }.
portal-print-auth-local = Connecté par un socket d’imprimante local.
portal-print-auth-loopback = Connecté à cet ordinateur. Cette connexion n’est pas chiffrée.
portal-print-auth-tls = Connecté avec un chiffrement TLS.
portal-print-auth-rejected = Les identifiants précédents n’ont pas été acceptés. Réessayez ou annulez.
portal-print-auth-domain = Domaine
portal-print-auth-username = Nom d’utilisateur
portal-print-auth-password = Mot de passe
portal-print-auth-invalid = Vérifiez la longueur du nom d’utilisateur et des identifiants.
portal-print-auth-not-saved = Utilisés uniquement pour cette demande d’impression. Les identifiants ne sont pas enregistrés par Canopa.
portal-print-auth-authenticate = S’authentifier

## Account: sharing the person's profile

portal-account-window = Partager le profil
portal-account-image-window = Choisir une image de profil
portal-account-title = Partager votre profil
portal-account-enter-information = Saisissez les informations que vous voulez partager.
portal-account-select = Sélectionner
portal-account-images = Images
portal-account-picker-failed = Impossible d’ouvrir le sélecteur d’images.
portal-account-image-failed = Impossible de charger cette image.
portal-account-image-prepare-failed = Impossible de préparer l’image pour le partage. Réessayez ou désactivez le partage de l’image.
portal-account-preparing-picker = Préparation du sélecteur d’images…
portal-account-loading = Chargement du profil…
portal-account-no-image = Aucune image
portal-account-choose-image = Choisir une image
portal-account-clear-image = Effacer l’image
portal-account-share-image = Partager l’image de profil
portal-account-share-username = Partager le nom d’utilisateur
portal-account-share-name = Partager le nom
portal-account-username = Nom d’utilisateur
portal-account-name = Nom
portal-account-invalid-name = Ce nom est trop long ou contient un caractère non pris en charge. Modifiez-le avant de le partager.
portal-account-preparing-image = Préparation de l’image…
portal-account-sharing = Partage…

## USB device access

portal-usb-window = Accès aux périphériques USB
portal-usb-question = Autoriser l’accès aux périphériques USB ?
portal-usb-choose = Choisissez les périphériques auxquels cette application peut accéder.
portal-usb-description = { $requester }. Choisissez les périphériques auxquels cette application peut accéder.
portal-usb-selected = { $count ->
    [one] { $count } sur { $total } sélectionné
   *[other] { $count } sur { $total } sélectionnés
}
portal-usb-select-all = Tout sélectionner
portal-usb-clear-selection = Effacer la sélection
portal-usb-select-all-shortcut = Tout sélectionner (Alt+S)
portal-usb-clear-selection-shortcut = Effacer la sélection (Alt+S)
portal-usb-read-write = Accès en lecture et écriture demandé
portal-usb-read-only = Accès en lecture seule demandé
portal-usb-serial = Numéro de série : { $serial }
portal-usb-location = Emplacement : { $location }
portal-usb-use-read-only = Utiliser plutôt un accès en lecture seule
portal-usb-read-only-hint = Alt+R passe les périphériques sélectionnés en lecture seule.
portal-usb-allow-selected = Autoriser la sélection
portal-usb-allow-selected-shortcut = Autoriser la sélection (Alt+A)
portal-usb-device = Périphérique USB

## Dynamic launcher installation review

portal-launcher-window = Installer un lanceur
portal-launcher-install-application = Installer le lanceur d’application ?
portal-launcher-install-webapp = Installer l’application web ?
portal-launcher-name = Nom du lanceur
portal-launcher-name-placeholder = Saisissez un nom
portal-launcher-name-too-long = Utilisez un nom plus court, sur une seule ligne.
portal-launcher-name-missing = Saisissez un nom de lanceur.
portal-launcher-web-address = Adresse web
portal-launcher-hint = Le lanceur apparaîtra parmi vos applications.
portal-launcher-install = Installer
portal-launcher-description = Demandé par { $requester }.
portal-launcher-description-webapp = Demandé par { $requester }. Adresse web : { $address }.

portal-with-shortcut = { $label } ({ $shortcut })

## Input capture consent

portal-input-capture-window = Capture des entrées
portal-input-capture-question = Autoriser la capture des entrées ?
portal-input-capture-request = { $requester } souhaite recevoir les entrées de ce bureau.
portal-input-capture-explanation = Déplacer le pointeur au-delà d’un bord d’écran choisi par l’application active la capture des entrées que vous autorisez.
portal-input-capture-description = { $requester } souhaite recevoir les entrées de ce bureau. Déplacer le pointeur au-delà d’un bord d’écran choisi par l’application active la capture des entrées que vous autorisez.
portal-input-capture-pointer = Autoriser les entrées du pointeur
portal-input-capture-keyboard = Autoriser les entrées du clavier
portal-input-capture-clipboard = Partager le presse-papiers pendant la capture des entrées
portal-input-capture-hint = Ctrl+Alt+Échap suspend la capture. Vous pouvez mettre fin à cette session depuis sa fenêtre de contrôle.
portal-input-capture-allow = Autoriser la capture des entrées

## Wallpaper

portal-wallpaper-window = Définir le fond d’écran
portal-wallpaper-question = Changer le fond d’écran ?
portal-wallpaper-preview = Aperçu du fond d’écran
portal-wallpaper-background = Fond du bureau
portal-wallpaper-lock-screen = Écran de verrouillage
portal-wallpaper-both = Bureau et écran de verrouillage
portal-wallpaper-lock-hint = L’image de l’écran de verrouillage est utilisée par l’action Verrouiller de Canopa.
portal-wallpaper-apply = Appliquer
portal-wallpaper-description = Demandé par { $requester }. { $destination }, { $size }.

## Global shortcuts

portal-shortcuts-window = Raccourcis globaux
portal-shortcuts-configure = Configurer les raccourcis globaux
portal-shortcuts-question = Autoriser les raccourcis globaux ?
portal-shortcuts-explanation = Choisissez les raccourcis que cette application peut utiliser lorsque d’autres fenêtres ont le focus.
portal-shortcuts-none-chosen = Aucun raccourci choisi
portal-shortcuts-placeholder = CTRL+ALT+a ou LOGO+F9
portal-shortcuts-cancel-recording = Annuler l’enregistrement
portal-shortcuts-record = Enregistrer les touches
portal-shortcuts-preparing = Préparation de l’enregistrement du clavier…
portal-shortcuts-release-to-begin = Relâchez les touches enfoncées pour commencer.
portal-shortcuts-press = Appuyez sur les touches du raccourci.
portal-shortcuts-release-to-finish = Relâchez les touches pour terminer.
portal-shortcuts-finishing = Fin de l’enregistrement du clavier…
portal-shortcuts-unavailable = Ce raccourci n’est pas disponible. Choisissez un autre déclencheur.
portal-shortcuts-none-requested = Cette application n’a demandé aucun raccourci.
portal-shortcuts-syntax-hint = Utilisez CTRL, ALT, SHIFT ou LOGO (Super), suivi du nom d’une touche. Exemple : CTRL+ALT+a.
portal-shortcuts-applying = Application…
portal-shortcuts-save = Enregistrer les raccourcis
portal-shortcuts-unrepresentable = Cette combinaison de touches n’est pas disponible avec la disposition de clavier actuelle. Choisissez un autre raccourci.
portal-shortcuts-keymap-changed = La configuration du clavier a changé. Enregistrez à nouveau le raccourci.
portal-shortcuts-recording-interrupted = L’enregistrement du clavier a été interrompu. Donnez le focus à cette fenêtre et réessayez.
portal-shortcuts-recording-gone = L’enregistrement du clavier n’est plus disponible dans cette fenêtre.
portal-shortcuts-recording-unavailable = L’enregistrement du clavier n’est pas disponible. Saisissez un raccourci ou donnez le focus à cette fenêtre et réessayez.
portal-shortcuts-conflict = Un raccourci sélectionné est déjà utilisé. Choisissez un autre déclencheur.
portal-shortcuts-stale = Les raccourcis disponibles ont changé. Vérifiez vos choix et réessayez.
portal-shortcuts-unresolved = Une combinaison de touches sélectionnée n’est pas disponible avec la disposition de clavier actuelle.
portal-shortcuts-failed = Ces raccourcis n’ont pas pu être appliqués. Vérifiez les touches sélectionnées et réessayez.
portal-shortcuts-missing-key = Choisissez une touche pour chaque raccourci sélectionné.

portal-printer-recovery-title = Récupération des travaux d’impression

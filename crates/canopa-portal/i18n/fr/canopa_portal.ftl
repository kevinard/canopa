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

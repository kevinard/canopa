# Canopa : Fichiers.
# Traduction française : assistée par machine, à relire (machine-assisted, needs review).

## Commands: toolbar buttons, menu items, shortcut names (src/interface.rs)

files-command-back = Précédent
files-command-forward = Suivant
files-command-parent = Dossier parent
files-command-enter-folder = Entrer dans le dossier
files-command-open = Ouvrir la sélection
files-command-quick-look = Aperçu rapide
files-command-search = Rechercher des noms de fichiers
files-command-search-subfolders = Inclure les sous-dossiers dans la recherche
files-command-stop-search = Arrêter la recherche
files-command-more-search-results = Plus de résultats
files-command-reveal-search-result = Afficher dans le dossier parent
files-command-search-problems = Emplacements de recherche illisibles
files-command-location-focus = Modifier l’emplacement
files-command-refresh = Actualiser
files-command-show-hidden = Afficher les fichiers cachés
files-command-select-all = Tout sélectionner
files-command-new-folder = Nouveau dossier
files-command-rename = Renommer
files-command-trash = Mettre à la corbeille
files-command-delete-permanently = Supprimer définitivement
files-command-show-trash = Ouvrir la corbeille
files-command-connect-to-server = Se connecter à un serveur…
files-command-undo = Annuler
files-command-copy = Copier
files-command-cut = Couper
files-command-paste = Coller
files-command-columns = Vue en colonnes
files-command-list = Vue en liste
files-command-icons = Vue en icônes
files-command-new-tab = Nouvel onglet
files-command-new-window = Nouvelle fenêtre
files-command-close-tab = Fermer l’onglet
files-command-next-tab = Onglet suivant
files-command-previous-tab = Onglet précédent
files-command-move-tab-left = Déplacer l’onglet à gauche
files-command-move-tab-right = Déplacer l’onglet à droite
files-command-move-tab-to-window = Déplacer l’onglet dans une nouvelle fenêtre
files-command-operations = Opérations
files-command-recovery = Récupération
files-command-custom-actions = Actions personnalisées
files-command-preferences = Préférences
files-command-bookmark-folder = Ajouter ce dossier aux signets

## Built-in places in the sidebar

files-place-home = Dossier personnel
files-place-desktop = Bureau
files-place-documents = Documents
files-place-downloads = Téléchargements
files-place-pictures = Images
files-place-music = Musique
files-place-videos = Vidéos
files-place-templates = Modèles
files-place-public = Public
files-place-filesystem = Système de fichiers
files-place-trash = Corbeille
files-place-recovery = Récupération

## Thumbnail and icon size choices (Preferences)

files-thumbnails-local = Fichiers locaux uniquement
files-thumbnails-always = Tous les fichiers
files-thumbnails-never = Jamais
files-icon-size-small = Petite
files-icon-size-medium = Moyenne
files-icon-size-large = Grande
files-icon-size-huge = Très grande

## Toolbar and location bar

files-toolbar-search = Rechercher
files-toolbar-actions = Actions
files-toolbar-places = Emplacements
files-location = Emplacement
files-crumb-current-description = Dossier actuel. Activer pour modifier l’emplacement

## Shared window chrome

files-close-window = Fermer la fenêtre

## Browser window

files-toolbar = Barre d’outils
files-tabs = Onglets
files-tab-close = Fermer { $folder }
files-sidebar = Barre latérale
files-view = Affichage
files-configuration-not-applied = La configuration n’a pas été appliquée : { $error }
files-view-preferences-not-saved = Les préférences d’affichage n’ont pas été enregistrées : { $error }
files-retry = Réessayer
files-close = Fermer
files-searching-filenames = Recherche des noms de fichiers…
files-no-matching-filenames = Aucun nom de fichier correspondant
files-empty-folder = Dossier vide
files-loading-folder = Chargement du dossier…
files-folder-unavailable = Dossier indisponible
files-search-results = Résultats de la recherche
files-column-view = Vue en colonnes
files-list-view = Vue en liste
files-icon-view = Vue en icônes
files-view-current = Affichage : { $view }
files-view-columns = Colonnes
files-view-list = Liste
files-view-icons = Icônes
files-status-items = { $count ->
    [one] { $count } élément
   *[other] { $count } éléments
}
files-folder-items-at-least = { $count } éléments ou plus
files-folder-empty = Vide
files-status-results = { $count ->
    [one] { $count } résultat
   *[other] { $count } résultats
}
files-status-inspected = { $count ->
    [one] { $count } examiné
   *[other] { $count } examinés
}
files-status-selected = { $count ->
    [one] { $count } sélectionné
   *[other] { $count } sélectionnés
}
files-status-selected-size = { $count ->
    [one] { $count } sélectionné ({ $size })
   *[other] { $count } sélectionnés ({ $size })
}
files-status-selected-folder = « { $name } » · { $kind }
files-status-selected-file = « { $name } » · { $kind } · { $size }
files-status-stopping = Arrêt…
files-status-searching = Recherche…
files-status-stopped = Arrêtée
files-status-more-results = D’autres résultats sont disponibles
files-announce-items-selected = { $count ->
    [one] { $count } élément
   *[other] { $count } éléments
}, { $selected ->
    [one] { $selected } sélectionné
   *[other] { $selected } sélectionnés
}
files-announce-results = { $count ->
    [one] { $count } résultat
   *[other] { $count } résultats
}
files-announce-results-selected = { $count ->
    [one] { $count } résultat
   *[other] { $count } résultats
}, { $selected ->
    [one] { $selected } sélectionné
   *[other] { $selected } sélectionnés
}
files-announce-results-stopped = { $count ->
    [one] { $count } résultat, recherche arrêtée
   *[other] { $count } résultats, recherche arrêtée
}
files-announce-results-stopped-selected = { $count ->
    [one] { $count } résultat
   *[other] { $count } résultats
}, recherche arrêtée, { $selected ->
    [one] { $selected } sélectionné
   *[other] { $selected } sélectionnés
}

## Search bar

files-search-include-subfolders = Inclure les sous-dossiers
files-search-stop = Arrêter
files-search-in = Rechercher dans { $folder }
files-search-close = Fermer la recherche
files-search-more = Plus de résultats
files-search-reveal = Afficher dans le dossier
files-search-unreadable-details = { $count ->
    [one] { $count } illisible · Détails
   *[other] { $count } illisibles · Détails
}
files-search-unreadable-hide = { $count ->
    [one] { $count } illisible · Masquer
   *[other] { $count } illisibles · Masquer
}
files-search-failure = { $location } — { $error }
files-this-folder = Ce dossier

## Preferences window: pages and status line

files-preferences-page-views = Affichage
files-preferences-page-toolbar = Barre d’outils
files-preferences-page-locations = Barre latérale
files-preferences-page-shortcuts = Raccourcis
files-preferences-status-initial = Les modifications s’appliquent à toutes les fenêtres de Fichiers dès que vous les faites.
files-preferences-too-many-locations = 256 emplacements au maximum sont pris en charge.
files-preferences-column-width-invalid = La largeur des colonnes doit être un nombre entier compris entre 160 et 640.
files-preferences-bookmark-path-invalid = Un signet nécessite un chemin local absolu, un chemin ~/ ou une URI de fichier local.
files-preferences-shortcut-invalid = { $command } : { $error }
files-preferences-field-too-long = Ce champ est limité à { $max } octets.
files-preferences-location-required = Le champ d’emplacement doit rester disponible.
files-preferences-recording = Appuyez sur un raccourci. Échap annule ; Retour arrière désactive cette commande.
files-preferences-recording-cancelled = Enregistrement du raccourci annulé.
files-preferences-wait = Attendez la fin de la modification de la configuration.
files-preferences-another-change = Une autre modification de la configuration est encore en cours.
files-preferences-worker-unavailable = Le service de configuration est indisponible.
files-preferences-worker-stopped = Le service de configuration s’est arrêté avant de répondre.
files-preferences-saving = Enregistrement…
files-preferences-reloading = Rechargement…
files-preferences-saved = Enregistré. Les modifications s’appliquent à toutes les fenêtres de Fichiers.
files-preferences-reloaded = Préférences partagées de Fichiers rechargées.

## Preferences window: controls

files-preferences-subtitle = Choisissez l’apparence et le comportement de Fichiers.
files-preferences-read-only = Les valeurs par défaut intégrées sont en lecture seule. Démarrez Fichiers avec une configuration modifiable pour enregistrer les préférences.
files-preferences-changed-outside = La configuration a été modifiée en dehors de cet éditeur. Votre brouillon est conservé ; choisissez Annuler les modifications pour charger la nouvelle version avant d’enregistrer.
files-preferences-discard-question = Abandonner les modifications non enregistrées des préférences ?
files-preferences-reload-question = Recharger et abandonner les modifications non enregistrées des préférences ?
files-preferences-keep-editing = Continuer la modification
files-preferences-discard = Abandonner
files-preferences-reload = Recharger
files-preferences-pending = Appuyez sur Entrée pour appliquer la valeur saisie.
files-preferences-keys = Alt+1–4 pages · Ctrl+S Enregistrer · Ctrl+R Recharger
files-preferences-default-view = Vue par défaut
files-preferences-sort-by = Trier par
files-preferences-descending = Ordre décroissant
files-preferences-columns-view = Affichage en colonnes
files-preferences-all-views = Valeurs par défaut
files-preferences-each-view = Réglages par affichage
files-preferences-list-view = Affichage en liste
files-preferences-icons-view = Affichage en icônes
files-preferences-column-width = Largeur des colonnes
files-preferences-column-width-range = 160 à 640 pixels
files-preferences-list-details = Détails à afficher
files-preferences-modified-date = Date de modification
files-preferences-thumbnails = Miniatures
files-preferences-icon-size = Taille des icônes
files-preferences-views-note = Les dossiers héritent de ces réglages sauf s’ils ont leurs propres options d’affichage.
files-preferences-thumbnails-note = « Fichiers locaux uniquement » ignore les partages réseau et les supports amovibles.
files-preferences-location-field = Champ d’emplacement
files-preferences-toolbar-action = Action : { $name }
files-preferences-earlier = Avant
files-preferences-later = Après
files-preferences-remove = Retirer
files-preferences-find-command-to-add = Rechercher une commande à ajouter
files-preferences-add-item = Ajouter « { $item } »
files-preferences-toolbar-footer = Le champ d’emplacement reste disponible. Les actions personnalisées placées explicitement n’apparaissent qu’une fois ; les actions de barre d’outils non positionnées gardent leur propre rangée.
files-preferences-sidebar-auto = Automatique
files-preferences-sidebar-always = Quand il y a de la place
files-preferences-sidebar-never = Masquée
files-preferences-locations-and-bookmarks = Emplacements et signets
files-preferences-bookmark-name = Nom du signet
files-preferences-bookmark-folder = Dossier (chemin absolu, chemin ~/ ou URI de fichier local)
files-preferences-add-bookmark = Ajouter un signet…
files-preferences-locations-footer = Les dossiers manquants restent dans les signets. Quand la fenêtre est trop étroite pour la barre latérale, Emplacements… dans le menu de la barre d’outils propose les mêmes emplacements.
files-preferences-toolbar-preview = Aperçu de la barre d’outils
files-preferences-add-button = Ajouter un bouton…
files-preferences-show-sidebar = Afficher la barre latérale
files-preferences-places = Emplacements de la barre latérale, dans l’ordre. Les signets sont les emplacements que vous ajoutez pour vos propres dossiers.
files-preferences-add-place = Ajouter un emplacement…
files-preferences-no-shortcut = Aucun raccourci
files-preferences-record-for = Enregistrer un raccourci pour { $command }
files-preferences-reset-for = Rétablir le raccourci par défaut de { $command }
files-preferences-find-command = Rechercher une commande
files-preferences-command-shortcuts = Raccourcis de « { $command } »
files-preferences-shortcuts-field = Raccourcis (séparés par ;, vide pour désactiver)
files-preferences-record = Enregistrer…
files-preferences-use-defaults = Utiliser les valeurs par défaut
files-preferences-shortcuts-footer = Les touches de sélection, de focus et de saisie restent disponibles. L’enregistrement vérifie les conflits avec chaque commande et action personnalisée.

## File listing: columns and kinds

files-column-name = Nom
files-column-size = Taille
files-column-modified = Modification
files-column-kind = Type
files-column-name-sorted-by = Nom  ·  Trié par { $field }
files-sort-by-heading = Trier par { $field }
files-tree-expand = Ouvrir { $name } sur place
files-tree-collapse = Fermer { $name }
files-kind-link = Lien
files-kind-link-to-folder = Lien vers un dossier
files-kind-folder = Dossier
files-kind-file = Fichier
files-kind-extension = Fichier { $extension }
files-row-modified = modifié le { $date }
files-row-hidden = caché

## Window titles

files-window-title = { $title } — Fichiers Canopa
files-choose-folder = Choisir un dossier
files-recover-copy-title = Récupérer une copie
files-restore-to-folder-title = Restaurer dans un dossier
files-transfer-details-title = Détails du transfert
files-custom-actions-title = Actions personnalisées
files-title-search = Recherche : { $query } — { $folder }
files-title-searching = Recherche en cours : { $query } — { $folder }
files-title-search-stopping = Arrêt de la recherche : { $query } — { $folder }
files-title-search-stopped = Recherche arrêtée : { $query } — { $folder }
files-operations-save-before-closing = Enregistrez le résultat de l’opération avant de fermer la dernière fenêtre.

## Actions menu (toolbar menu and context menu)

files-menu-open = Ouvrir
files-menu-open-with = Ouvrir avec…
files-menu-open-in-new-tab = Ouvrir dans un nouvel onglet
files-menu-new-folder = Nouveau dossier…
files-menu-new-folder-short = Nouveau dossier
files-menu-rename = Renommer…
files-menu-delete-permanently = Supprimer définitivement…
files-menu-edit-custom-actions = Modifier les actions personnalisées…
files-menu-preferences = Préférences…
files-menu-bookmark-folder = Ajouter ce dossier aux signets…
files-menu-locations = Emplacements…
files-menu-view-options = Options d’affichage…
files-menu-other-application = Autre application…
files-menu-default-application = { $application } (par défaut)
files-action-not-applicable = « { $action } » ne s’applique pas à cette sélection.

## Devices: removal and menu items ($device is a device name)

files-device-power-off = Éteindre
files-device-eject = Éjecter
files-device-unmount = Démonter
files-device-open-named = Ouvrir { $device }
files-device-mount-named = Monter { $device }
files-device-power-off-named = Éteindre { $device }
files-device-eject-named = Éjecter { $device }
files-device-unmount-named = Démonter { $device }

## Devices: sidebar and requests

files-devices = Périphériques
files-devices-unavailable = Les périphériques sont indisponibles.
files-devices-error = Périphériques : { $error }
files-cancel = Annuler
files-review = Examiner
files-open-folder = Ouvrir le dossier
files-device-cancelling = Annulation…
files-device-needs-decision = Une décision est nécessaire
files-device-mounting = Montage…
files-device-powering-off = Mise hors tension…
files-device-ejecting = Éjection…
files-device-unmounting = Démontage…
files-device-connecting = Connexion…
files-device-mounted = Monté
files-device-connected = Connecté
files-device-connected-remote = Connecté sans dossier local : le démon FUSE de GVFS (gvfsd-fuse) n’est pas lancé, seule une liste en lecture seule est disponible
files-device-removed = Retiré
files-device-cancelled = Annulé
files-device-failed = Impossible de terminer : { $error }
files-device-finished = Terminé

## New folder and Rename dialog

files-new-folder-name = Nouveau dossier
files-create = Créer
files-name-full-name = Nom complet
files-name-folder-name = Nom du dossier
files-name-invalid = Saisissez un seul nom, sans barre oblique. Les noms . et .. sont réservés.
files-name-dialog-failed = Impossible d’ouvrir la boîte de dialogue du nom.
files-rename-worker-stopped = Le service des opérations s’est arrêté avant de préparer le renommage.
files-operation-queue-unavailable = La file des opérations est indisponible.
files-accepting-operation = Acceptation de l’opération…
files-home-path = Dossier personnel › { $path }

## Permanent deletion confirmation

files-delete-title = Supprimer définitivement ?
files-delete-title-empty-trash = Vider la corbeille ?
files-delete-title-no-trash = Impossible de mettre à la corbeille
files-delete-one = « { $name } » sera supprimé immédiatement. Cette action est irréversible.
files-delete-cannot-undo = L’élément ci-dessous sera supprimé immédiatement. Cette action est irréversible.
files-delete-many = { $count ->
    [one] { $count } élément sera supprimé immédiatement. Cette action est irréversible.
   *[other] { $count } éléments seront supprimés immédiatement. Cette action est irréversible.
}
files-delete-no-trash-one = « { $name } » se trouve sur un volume sans corbeille utilisable et ne peut donc pas être mis à la corbeille. Le supprimer définitivement à la place ? Cette action est irréversible.
files-delete-no-trash-many = { $count ->
    [one] { $count } élément se trouve sur un volume sans corbeille utilisable et ne peut donc pas être mis à la corbeille. Le supprimer définitivement à la place ? Cette action est irréversible.
   *[other] { $count } éléments se trouvent sur un volume sans corbeille utilisable et ne peuvent donc pas être mis à la corbeille. Les supprimer définitivement à la place ? Cette action est irréversible.
}
files-delete-empty-trash = { $count ->
    [one] Supprimer définitivement l’unique élément de la corbeille ? Cette action est irréversible.
   *[other] Supprimer définitivement les { $count } éléments de la corbeille ? Cette action est irréversible.
}
files-delete-empty-main-trash = { $count ->
    [one] Supprimer définitivement l’unique élément de la corbeille principale ? Cette action est irréversible.
   *[other] Supprimer définitivement les { $count } éléments de la corbeille principale ? Cette action est irréversible.
}
files-delete-from-trash-one = Supprimer définitivement « { $name } » de la corbeille ? Cette action est irréversible.
files-delete-from-trash-many = { $count ->
    [one] Supprimer définitivement { $count } élément de la corbeille ? Cette action est irréversible.
   *[other] Supprimer définitivement { $count } éléments de la corbeille ? Cette action est irréversible.
}
files-empty-trash = Vider la corbeille
files-delete-items = { $count ->
    [one] Supprimer { $count } élément
   *[other] Supprimer { $count } éléments
}
files-confirmation-failed = Impossible d’ouvrir la confirmation.

## Dates

files-date-time = { $date } · { $time }
files-date-today = Aujourd’hui, { $time }
files-date-yesterday = Hier, { $time }
files-date-this-year = { $date }, { $time }
files-unknown = Inconnue

## Operation questions: conflicts and Undo decisions

files-question-undo = L’annulation nécessite une décision
files-question-recovery = La copie de récupération nécessite une décision
files-question-folder-conflict = Conflit de dossiers
files-question-file-conflict = Conflit de fichiers
files-question-destination-unavailable = Destination indisponible
files-question-failed = Impossible d’ouvrir la question de l’opération. Consultez l’opération pour réessayer.
files-conflict-folder-exists = Un dossier nommé « { $name } » existe déjà dans la destination.
files-conflict-item-exists = Un élément nommé « { $name } » existe déjà dans la destination.
files-conflict-merge-explanation = Fusionner combine ces dossiers et demande quoi faire pour chaque collision. Conserver les deux renomme le dossier entrant. Ignorer le laisse intact.
files-conflict-recovery-explanation = « Conserver les deux » copie l’élément enregistré sous un nouveau nom. « Ignorer » laisse les deux éléments intacts.
files-conflict-replace-explanation = Remplacer enregistre le fichier existant pour pouvoir annuler. Conserver les deux renomme le fichier entrant. Ignorer laisse les deux intacts.
files-conflict-kind-explanation = Ce nom est déjà utilisé par un élément d’un autre type. Conserver les deux donne un autre nom à l’élément entrant. Ignorer laisse les deux éléments intacts.
files-conflict-saved-version = Version enregistrée dans Récupération
files-conflict-incoming = Entrant
files-conflict-existing = Existant
files-conflict-replace = Remplacer
files-conflict-merge = Fusionner
files-conflict-keep-both = Conserver les deux
files-conflict-skip = Ignorer
files-conflict-apply-folders = Appliquer aux conflits de dossiers restants
files-conflict-apply-files = Appliquer aux conflits de fichiers restants
files-transfer-paused = Le transfert est en pause
files-transfer-unavailable-explanation = Rétablissez l’accès à la destination, puis choisissez Réessayer. Annuler conserve les fichiers source non terminés.
files-destination = Destination
files-system-details = Détails système · { $details }
files-kind-special = Élément spécial
files-entry-metadata = { $bytes ->
    [one] { $kind } · { $bytes } octet
   *[other] { $kind } · { $bytes } octets
}
    Modifié le { $modified }
files-entry-metadata-folder = { $kind }
    Modifié le { $modified }
files-undo-keep-item = Conserver l’élément
files-undo-finish-step = Terminer l’étape
files-undo-move-remaining = Déplacer les éléments restants
files-undo-keep-remaining = Conserver les éléments restants
files-undo-finish-merge = Terminer cette fusion
files-undo-move-to-recovery = Déplacer l’élément modifié vers Récupération
files-undo-use-saved-version = Utiliser la version enregistrée
files-undo-return-item = Rétablir l’élément
files-undo-restore-previous = Restaurer la destination précédente
files-undo-restoring-from-trash = Restauration depuis la corbeille
files-undo-saved-previous = Destination précédente enregistrée
files-undo-existing-item = Élément existant
files-undo-unavailable = Indisponible · { $error }
files-undo-destination = Destination · { $location }
files-undo-return-to = Rétablir vers · { $location }
files-undo-explain-current = L’annulation ramènerait cet élément à un nom déjà utilisé. Examinez les deux versions avant de choisir. Remplacer conserve l’élément déplacé dans Récupération ; Conserver les deux le rétablit sous un nouveau nom. Ignorer laisse cette étape disponible pour plus tard.
files-undo-explain-trash = Un élément de la corbeille doit retrouver un nom déjà utilisé. Examinez les deux versions. Conserver les deux le restaure sous un autre nom ; Ignorer le garde disponible pour plus tard.
files-undo-explain-previous-destination = L’annulation restaure une destination précédente enregistrée, mais son nom d’origine est utilisé. Examinez les deux versions. Les éléments remplacés restent protégés dans Récupération.
files-undo-explain-nonempty-folder = Ce dossier créé contient maintenant des travaux plus récents. Conserver l’élément laisse le dossier et son contenu en place et termine son étape de création. Ignorer garde l’étape disponible pour plus tard.
files-undo-explain-changed-created-folder = Le dossier créé a été modifié, déplacé ou remplacé. Conserver l’élément laisse l’élément actuel intact et termine cette étape de création. Ignorer vous permet de décider plus tard.
files-undo-explain-unavailable-created-folder = Le dossier créé ne peut pas être examiné. Rétablissez son emplacement ou son accès avant de réessayer. Terminer l’étape laisse les fichiers intacts et abandonne cette étape de suppression ; Ignorer la garde pour plus tard.
files-undo-explain-merge-remainder = Des éléments restent dans la destination fusionnée. Déplacer les éléments restants poursuit le retour en traitant les nouveaux conflits. Conserver les éléments restants les laisse ici et termine cette branche. Ignorer vous permet de décider plus tard.
files-undo-explain-merge = Un dossier utilisé par cette fusion partielle a été déplacé, modifié ou ne peut pas être examiné. Rétablissez son emplacement d’origine ou son accès pour continuer. Terminer cette fusion laisse les fichiers en place et conserve les destinations précédentes enregistrées dans Récupération. Les éléments déjà rétablis restent où ils sont.
files-undo-explain-changed-copy = Cet élément copié a changé depuis la copie. Déplacer l’élément modifié vers Récupération préserve son contenu plus récent avant d’annuler la copie. Conserver l’élément le laisse ici et termine son étape de copie ; Ignorer vous permet de décider plus tard.
files-undo-explain-changed-retained = La version enregistrée a changé. Examinez l’élément enregistré et l’élément actuel avant de choisir de l’utiliser. Conserver l’élément préserve l’élément actuel et termine cette étape, en gardant les versions protégées dans Récupération.
files-undo-explain-changed-move = L’élément déplacé ou sa destination a changé. Rétablir l’élément ramène l’élément actuel examiné et préserve toute destination précédente enregistrée. Conserver l’élément laisse l’élément actuel en place et termine cette étape. Ignorer la garde pour plus tard.
files-undo-explain-unavailable-move = L’élément déplacé ne peut pas être examiné. Rétablissez son emplacement ou son accès avant de continuer. Restaurer la destination précédente ne rétablit que cette version enregistrée, si elle est disponible. Terminer l’étape laisse l’élément déplacé intact et préserve les données protégées dans Récupération.
files-undo-explain-unavailable-retained = Une version précédente enregistrée ne peut pas être examinée. Rétablissez son emplacement ou son accès pour réessayer. Rétablir l’élément n’effectue que le retour disponible, s’il est proposé. Terminer l’étape laisse les fichiers actuels intacts et conserve les données protégées enregistrées dans Récupération.

## Operation queue messages

files-jobs-admission-full = La file d’admission des opérations est pleine pendant le retrait d’un périphérique. Réessayez une fois le retrait terminé.
files-jobs-cancelled-before-start = Annulée avant le démarrage
files-jobs-startup-stopped = Le démarrage de l’opération a été arrêté.
files-jobs-queue-full = La file des opérations est indisponible ou pleine. Réessayez.
files-jobs-volume-worker-stopped = Le service des volumes s’est arrêté avant sa première observation.
files-jobs-recovery-busy = Le lecteur de Récupération est indisponible ou occupé. Réessayez.
files-jobs-interrupted-needs-review = Un travail interrompu doit être examiné dans Récupération avant d’autres modifications.
files-jobs-recovery-cancelled-removal = Examen de Récupération annulé pendant l’attente du retrait d’un périphérique.
files-jobs-drag-cancelled-removal = Préparation du glisser annulée pendant l’attente du retrait d’un périphérique.
files-jobs-close-worker-busy = Terminez ou annulez le travail en cours, reprenez ou abandonnez les transferts arrêtés et enregistrez chaque résultat avant de fermer le service des opérations.
files-jobs-needs-reconciliation = Une opération doit être réconciliée avant d’autres modifications.
files-jobs-kept-in-recovery = Élément conservé dans Récupération. Le travail interrompu est préservé ; récupérez une copie si nécessaire.
files-jobs-retry-interrupted = Réessayer l’opération interrompue
files-jobs-journals-rechecked = Journaux des opérations interrompues revérifiés.
files-jobs-worker-stopped = Le service des opérations s’est arrêté de manière inattendue
files-jobs-reconciliation-retained = réconciliation du journal terminée ; résultat enregistré conservé
files-jobs-interrupted-steps = Opération de fichiers n° { $operation } : { $steps } étapes confirmées restent dans l’historique. Examinez le travail effectué avant de lancer une nouvelle demande ; cette demande enregistrée ne sera pas relancée.
files-jobs-interrupted-journal = Opération de fichiers n° { $operation } : { $steps } étapes confirmées restent dans l’historique ; son journal doit encore être examiné dans Récupération. Examinez le travail effectué avant de lancer une nouvelle demande ; cette demande enregistrée ne sera pas relancée.
files-jobs-interrupted-no-history = Opération de fichiers n° { $operation } : { $steps } étapes confirmées restent dans l’historique ; son historique d’annulation n’est plus disponible. Examinez le travail effectué avant de lancer une nouvelle demande ; cette demande enregistrée ne sera pas relancée.
files-jobs-some-work-cannot-continue = Une partie du travail enregistré ne peut pas encore continuer : { $error }
files-jobs-transfer-cannot-continue = Le transfert enregistré ne peut pas encore continuer : { $error }
files-jobs-reconciliation-not-saved = Résultat de la réconciliation non enregistré : { $error }
files-jobs-result-not-saved = Résultat non enregistré : { $error }
files-jobs-save-failed = Impossible d’enregistrer l’opération : { $error }
files-undo-outcome = { $reversed } annulé(s) · { $kept } conservé(s) · { $skipped } ignoré(s) · { $dismissed } étape(s) écartée(s) · { $unrestored } destination(s) précédente(s) non restaurée(s) · { $unremoved } élément(s) non supprimé(s) · { $remaining } restant(s)
files-undo-outcome-cancelled = { $outcome } · annulé

## Operation titles ($name is a file name, $destination a folder name)

files-job-keep-in-recovery = Conserver { $name } dans Récupération
files-job-recover = Récupérer { $name }
files-job-create-folder = Créer le dossier { $name }
files-job-rename = Renommer { $source } en { $destination }
files-job-restore = Restaurer { $name }
files-job-trash = { $count ->
    [one] Mettre { $count } élément à la corbeille
   *[other] Mettre { $count } éléments à la corbeille
}
files-job-delete = { $count ->
    [one] Supprimer définitivement { $count } élément
   *[other] Supprimer définitivement { $count } éléments
}
files-job-empty-trash = { $count ->
    [one] Vider la corbeille ({ $count } élément)
   *[other] Vider la corbeille ({ $count } éléments)
}
files-job-delete-from-trash = { $count ->
    [one] Supprimer définitivement { $count } élément de la corbeille
   *[other] Supprimer définitivement { $count } éléments de la corbeille
}
files-job-copy = { $count ->
    [one] Copier { $count } élément vers { $destination }
   *[other] Copier { $count } éléments vers { $destination }
}
files-job-move = { $count ->
    [one] Déplacer { $count } élément vers { $destination }
   *[other] Déplacer { $count } éléments vers { $destination }
}

## Operation history records (src/history_labels.rs)
## Lower-case phrases that name a recorded operation; $operation is one of them.

files-history-undo = Annuler : { $operation }
files-history-interrupted = Interrompu : { $operation }
files-history-preserved-undo = Préservé par l’annulation : { $operation }
files-history-preserved-kept-item = Préservé en conservant un élément pendant l’annulation : { $operation }
files-history-preserved-unavailable-item = Préservé après avoir terminé un élément indisponible pendant l’annulation : { $operation }
files-history-preserved-unavailable-saved = Préservé après avoir terminé une version enregistrée indisponible pendant l’annulation : { $operation }
files-history-operation = opération
files-history-restore-permissions = Restaurer les permissions du dossier
files-history-kept-undo-folder = Dossier de travail d’annulation interrompue conservé
files-history-create-folder = création du dossier { $name }
files-history-rename = renommage de { $source } en { $destination }
files-history-copy = copie vers { $destination }
files-history-move = déplacement vers { $destination }
files-history-trash = { $count ->
    [one] mise à la corbeille de { $count } élément
   *[other] mise à la corbeille de { $count } éléments
}
files-history-restore = restauration de { $name }
files-history-recover = récupération de { $name }

## Desktop notifications

files-notifications-unavailable = Les notifications du bureau sont indisponibles.
files-notification-timed-out = La notification du bureau a expiré.
files-notification-not-sent = La notification du bureau n’a pas pu être envoyée.
files-notifications-busy = Les notifications du bureau sont occupées ou indisponibles.
files-app-full-name = Fichiers Canopa
files-notification-decision = Une opération nécessite une décision
files-notification-worker-stopped = Le service des notifications du bureau s’est arrêté avant de répondre.
files-completed-detail = Terminée.
files-notification-decision-body = { $operation } · Ouvrez Opérations pour l’examiner.

## File operations window

files-recovery-reader-stopped = Le lecteur de Récupération s’est arrêté avant de répondre.
files-operation-queue-unavailable-short = La file des opérations est indisponible
files-operation-worker-stopped-acceptance = Le service des opérations s’est arrêté avant de confirmer l’acceptation.
files-abandon-not-saved = La file des opérations s’est arrêtée avant l’enregistrement de l’abandon.
files-dismissal-not-saved = La file des opérations s’est arrêtée avant l’enregistrement du rejet.
files-history-heading = Historique des fichiers
files-history-latest = Plus récentes
files-history-older = Plus anciennes
files-history-preparing = Préparation des opérations sur les fichiers…
files-history-empty = Aucune opération sur les fichiers pour l’instant
files-history-empty-description = Les modifications de fichiers effectuées dans Fichiers apparaîtront ici.
files-history-moved-to-trash = Mis à la corbeille
files-history-renamed = Renommé
files-history-created-folder = Dossier créé
files-details = Détails
files-retry-save = Réessayer l’enregistrement
files-retry-failed-items = Réessayer les éléments en échec
files-continue-remaining = Poursuivre le travail restant
files-abandon-unfinished = Abandonner le travail inachevé
files-dismiss-request = Écarter la demande
files-dismiss-request-description = Écarter libère cette demande enregistrée. Le travail effectué et l’annulation restent disponibles.
files-retry-failed-recovery = Réessayer les copies de récupération en échec
files-continue-recovery = Poursuivre la copie de récupération
files-phase-completed = Terminée
files-phase-queued = En attente
files-phase-running = En cours
files-phase-awaiting-review = Non démarrée · examen nécessaire
files-phase-interrupted = Interrompue · examen nécessaire
files-phase-failed = Impossible de terminer
files-phase-stopped = Arrêtée · examen nécessaire
files-phase-start-save-failed = Non démarrée · enregistrement impossible
files-phase-save-failed = Résultat non enregistré
files-operation-summary = { $status } · { $operation }
files-operation-announcement = { $operation } : { $status }
files-operation-announcement-error = { $operation } : { $status }. { $error }
files-history-renamed-to = Renommé en { $name }
files-history-previously = Anciennement { $name }
files-history-accepted = Demande acceptée le { $date }
files-history-count = { $count ->
    [one] { $count } opération
   *[other] { $count } opérations
}
files-history-count-latest-first = { $count ->
    [one] { $count } opération  ·  Plus récentes d’abord
   *[other] { $count } opérations  ·  Plus récentes d’abord
}

## Transfer details and progress

files-details-completed = Destinations terminées ({ $count })
files-details-failed = Éléments en échec ({ $count })
files-details-skipped = Éléments ignorés ({ $count })
files-details-remaining = Éléments restants ({ $count })
files-details-sources = Dossiers source à examiner ({ $count })
files-details-completed-short = Terminés ({ $count })
files-details-failed-short = Échecs ({ $count })
files-details-skipped-short = Ignorés ({ $count })
files-details-remaining-short = Restants ({ $count })
files-details-sources-short = Sources ({ $count })
files-details-failure = Source
    { $source }
    Destination
    { $destination }
    { $error }
files-details-truncated = … (tronqué)
files-details-failed-to-open = Impossible d’ouvrir les détails du transfert. Réessayez Détails dans Opérations.
files-review-decision = Examiner la décision
files-details-section-empty = Aucun élément dans cette section.
files-details-entry = { $number }. { $entry }
files-details-page = Éléments { $first } à { $last } sur { $count }
files-details-gone = Cette opération n’est plus disponible.
files-previous = Précédent
files-next = Suivant
files-progress-paused-at = En pause sur { $name }
files-progress-checking = Vérification de { $name }
files-progress-transferring = Transfert de { $name }
files-progress-counts = { $total ->
    [one] { $done } sur { $total } élément · { $bytes } sur { $total_bytes }
   *[other] { $done } sur { $total } éléments · { $bytes } sur { $total_bytes }
}
files-progress-counting = { $total ->
    [one] { $done } sur { $total }+ élément · { $bytes } · comptage…
   *[other] { $done } sur { $total }+ éléments · { $bytes } · comptage…
}
files-progress-running-counts = { $done ->
    [one] { $done } élément · { $bytes }
   *[other] { $done } éléments · { $bytes }
}
files-progress-rate = { $size }/s
files-progress-time-left = environ { $duration } restantes
files-progress-few-seconds-left = encore quelques secondes
files-progress-rate-and-time = { $rate } · { $left }

## Operations window and custom commands

files-phase-waiting-for-device = En attente du retrait d’un périphérique…
files-output-stdout = Sortie standard
files-output-stderr = Sortie d’erreur
files-output-empty = (vide)
files-output-truncated = [La sortie a été tronquée]
files-actions-runtime-unavailable = Le service des actions personnalisées est indisponible.
files-actions-runtime-stopped-admission = Le service des actions personnalisées s’est arrêté avant de répondre à l’admission.
files-new-browser = Nouveau navigateur
files-operations-subtitle = Modifications de fichiers, transferts et résultats des commandes
files-custom-commands = Commandes personnalisées
files-latest-commands = Commandes récentes
files-older-commands = Commandes plus anciennes
files-hide-output = Masquer la sortie
files-show-output = Afficher la sortie
files-previous-page = Page précédente
files-next-page = Page suivante
files-copy-all-output = Copier toute la sortie

## Custom action commands

files-action-folder-changed = Le dossier d’origine de l’action a changé. Retournez dans ce dossier et lancez de nouveau l’action.
files-action-command-terminated = Commande interrompue
files-action-runtime-stopped = Le service des actions personnalisées s’est arrêté.
files-action-queue-full = La file des actions personnalisées est indisponible ou pleine. Réessayez.
files-action-close-busy = Terminez ou annulez les commandes acceptées et transmettez leurs avis de fin avant d’arrêter le service des actions personnalisées.
files-action-invalid-request = Une action personnalisée nécessite des chemins sélectionnés absolus et son dossier de travail d’origine.
files-action-too-many = Trop de commandes inachevées ou de résultats non remis. Terminez ou annulez le travail et récupérez ses avis de fin avant d’en lancer un autre.
files-action-sequence-exhausted = La séquence des actions personnalisées est épuisée.
files-action-worker-stopped = Le service des commandes s’est arrêté de manière inattendue.
files-output-heading = { $stream } :
files-custom-actions-error = Actions personnalisées : { $error }
files-command-job-folder = { $folder }  ·  { $count ->
    [one] { $count } sélectionné
   *[other] { $count } sélectionnés
}
files-action-exit-code = Échec · code de sortie { $code }
files-action-could-not-run = Exécution impossible : { $error }

## Properties window

files-properties = Propriétés
files-properties-title-reading = Propriétés — Lecture…
files-properties-title = { $count ->
    [one] Propriétés — { $count } élément
   *[other] Propriétés — { $count } éléments
}
files-properties-reading = Lecture des propriétés…
files-properties-cancelled = Examen des propriétés annulé.
files-properties-copy-location = Copier l’emplacement
files-properties-copy-locations = Copier les emplacements
files-properties-link-target = Cible du lien
files-properties-dimensions = Dimensions
files-command-properties = Propriétés
files-preferences-properties = Propriétés
files-properties-display-window = Dans une fenêtre
files-properties-display-panel = Dans un panneau latéral
files-properties-created = Création
files-properties-opens-with = S’ouvre avec
files-properties-change = Modifier…
files-properties-executable = Autoriser l’exécution comme programme
files-properties-rename-hint = Appuyez sur Entrée pour renommer
files-properties-advanced = Mode et identifiants
files-properties-access-saving = Mise à jour de l’accès…
files-permission-folder-none = Aucun accès
files-permission-folder-list = Lister les fichiers uniquement
files-permission-folder-access = Accéder aux fichiers
files-permission-folder-change = Créer et supprimer des fichiers
files-permission-file-none = Aucun accès
files-permission-file-read = Lecture seule
files-permission-file-write = Lecture et écriture
files-properties-panel-empty = Sélectionnez un élément pour voir ses propriétés.
files-properties-hide-panel = Masquer les propriétés
files-properties-pixels = { $width } × { $height } pixels
files-properties-size-detail = { $size } ({ $exact })
files-properties-access = Accès
files-properties-owner = Propriétaire
files-properties-group = Groupe
files-properties-others = Autres
files-properties-account = Compte
files-properties-permission = Autorisation
files-properties-access-identity = { $who } · { $identity }
files-properties-mode = Mode { $mode } · UID { $uid } · GID { $gid }
files-unavailable = Indisponible
files-kind-symlink = Lien symbolique
files-kind-special-file = Fichier spécial
files-size-bytes = { $bytes ->
    [one] { $bytes } octet
   *[other] { $bytes } octets
}
files-date-out-of-range = Hors de la plage de dates prise en charge
files-permission-none = Aucun
files-permission-x = Exécution
files-permission-w = Écriture
files-permission-wx = Écriture et exécution
files-permission-r = Lecture
files-permission-rx = Lecture et exécution
files-permission-rw = Lecture et écriture
files-permission-rwx = Lecture, écriture et exécution

## Trash window and Restore to folder

files-trash-restore = Restaurer
files-trash-restore-to = Restaurer vers…
files-trash-copy = Copier dans le presse-papiers
files-trash-empty = Vider la corbeille…
files-trash-all-locations = Tous les emplacements
files-trash-main = Corbeille principale
files-trash-original-unknown = Emplacement d’origine inconnu
files-trash-deleted = Supprimé le { $date }
files-trash-date-unknown = Date de suppression inconnue
files-trash-item-description = { $location }, { $date }
files-trash-items = Éléments de la corbeille
files-trash-items-heading = Éléments dans la corbeille
files-trash-loading = Chargement de la corbeille…
files-trash-original-unavailable = Les informations d’origine sont indisponibles. Copiez cet élément pour récupérer son contenu.
files-trash-count-select = { $count ->
    [one] { $count } élément · Sélectionnez un élément pour voir les options de restauration
   *[other] { $count } éléments · Sélectionnez un élément pour voir les options de restauration
}
files-select-item-for-restore = Sélectionnez un élément pour voir les options de restauration
files-trash-empty-title = La corbeille est vide
files-trash-empty-description = Les éléments mis à la corbeille apparaîtront ici.
files-trash-subtitle = Restaurez les fichiers supprimés à leur emplacement d’origine ou dans un autre dossier, ou supprimez-les définitivement.
files-restore-dialog-failed = Impossible d’ouvrir la boîte de dialogue de destination de la restauration.
files-operation-queue-not-ready = La file des opérations n’est pas prête. Réessayez.
files-enter-destination-folder = Saisissez un dossier de destination local.
files-folder-path = Chemin du dossier
files-location-line = Emplacement · { $location }
files-destination-folder = Dossier de destination
files-restore-name-taken = Si ce nom est déjà pris, l’élément reste dans la corbeille.
files-restore-here = Restaurer ici

## Recovery window, Recover a copy and the folder chooser

files-recovery-recover = Récupérer une copie…
files-recovery-keep = Conserver dans Récupération
files-recovery-load-more = Charger plus
files-recovery-reconcile = Réconcilier le travail interrompu
files-recovery-view-operations = Voir les opérations
files-recovery-held = Conservé pour l’annulation
files-recovery-held-modified = Modifié le { $date }  ·  Conservé pour l’annulation
files-recovery-saved-copies = Copies enregistrées
files-recovery-loading = Chargement de Récupération…
files-recovery-count = { $count ->
    [one] { $count } version conservée · Récupération crée une copie et garde la version enregistrée.
   *[other] { $count } versions conservées · Récupération crée une copie et garde la version enregistrée.
}
files-recovery-count-loaded = { $count ->
    [one] { $count } version conservée chargée · Récupération crée une copie et garde la version enregistrée.
   *[other] { $count } versions conservées chargées · Récupération crée une copie et garde la version enregistrée.
}
files-recovery-empty = Aucune version enregistrée
files-recovery-empty-description = Les versions enregistrées lors du remplacement de fichiers apparaissent ici. Opérations affiche vos modifications de fichiers récentes.
files-recovery-subtitle-interrupted = Versions enregistrées et opérations interrompues
files-recovery-subtitle = Récupérez une copie d’une version antérieure.
files-recovery-saved-versions = Versions enregistrées
files-recovery-select = Sélectionnez une version pour choisir où la récupérer
files-recovery-interrupted-notice = { $operation } : { $message }
files-recovery-destination-invalid = Saisissez un dossier de destination local et un nom de fichier valide.
files-recovery-name-invalid = Saisissez un nom de fichier, ou un chemin local complet ou une URI de fichier.
files-recovery-no-copy = Aucune copie n’a été faite. Vérifiez la destination et réessayez.
files-recovery-dialog-failed = Impossible d’ouvrir la boîte de dialogue de destination de Récupération.
files-recovery-keep-name = Garder le nom de fichier enregistré
files-file-name = Nom du fichier
files-recovery-new-name = Nouveau nom de fichier (facultatif)
files-recovery-submitting = Envoi de la copie de récupération…
files-recovery-in-progress = Demande de copie en cours. Fermer cette fenêtre laisse le travail accepté se poursuivre.
files-recovery-hint = ~ désigne votre dossier personnel. Laissez le nom de fichier vide pour garder le nom enregistré.
files-recovery-recover-here = Récupérer ici
files-destination-up = Monter
files-destination-no-subfolders = Aucun sous-dossier
files-destination-reading = Lecture des dossiers…
files-destination-use = Utiliser ce dossier
files-browse = Parcourir…

## Open With and launch confirmation ($name is a file name)

files-open-could-not-open = Impossible d’ouvrir « { $name } » : { $error }
files-open-default-failed = Impossible de définir { $application } par défaut : { $error }
files-open-start-failed = Impossible de démarrer { $name } : { $error }
files-open-start-file-failed = Impossible de démarrer « { $name } » : { $error }
files-open-link-failed = Impossible d’ouvrir le lien : { $error }
files-open-run-failed = Impossible d’exécuter « { $name } » : { $error }
files-open-terminal-missing = L’exécution dans un terminal nécessite xdg-terminal-exec ; installez-le pour choisir votre terminal.
files-open-failed = L’ouverture du fichier a échoué : { $error }
files-open-with-title = Ouvrir avec
files-open-run-script-title = Exécuter ce script ?
files-open-run-program-title = Exécuter ce programme ?
files-open-untrusted-title = Lanceur d’application non fiable
files-open-choose-one = Choisissez une application pour « { $name } ».
files-open-choose-many = { $count ->
    [one] Choisissez une application pour { $count } fichier.
   *[other] Choisissez une application pour { $count } fichiers.
}
files-open-script-message = « { $name } » est un script exécutable. Ne l’exécutez que si vous avez confiance en sa provenance.
files-open-program-message = « { $name } » est un programme exécutable. Ne l’exécutez que si vous avez confiance en sa provenance.
files-open-untrusted-message = « { $application } » ({ $file }) n’est pas installé dans un dossier d’applications et n’est pas marqué comme exécutable. Ne le lancez que si vous avez confiance en sa provenance. Marquer le fichier comme exécutable indique à Fichiers de lui faire confiance.
files-open-file-failed = Impossible d’ouvrir le fichier.
files-open-list-failed = Impossible de lister les applications.
files-open-interrupted = Le lancement a été interrompu.
files-open-dialog-failed = Impossible d’ouvrir la boîte de dialogue.
files-open-search = Rechercher des applications
files-open-looking = Recherche des applications…
files-open-no-match = Aucune application correspondante
files-open-recommended = Recommandées
files-open-other = Autres applications
files-open-applications = Applications
files-open-set-default = Définir par défaut pour { $mime }
files-open-in = Ouvrir dans { $application }
files-open-run-in-terminal = Exécuter dans un terminal
files-open-run = Exécuter
files-open-as-text = Ouvrir comme texte
files-open-launch = Lancer

## Device and server questions (passwords, choices)

files-device-q-connect = Se connecter au serveur
files-device-q-unlock = Déverrouiller le périphérique
files-device-q-busy = Le périphérique est occupé
files-device-q-host-identity = Vérifier l’identité du serveur
files-device-q-server = Question du serveur
files-device-q-device = Question du périphérique
files-device-q-failed = Impossible d’ouvrir la question du périphérique. Consultez-la dans Opérations pour réessayer.
files-device-q-too-long = Le champ dépasse sa limite de saisie.
files-device-q-pim-invalid = Saisissez un nombre entier pour le PIM.
files-device-q-username = Nom d’utilisateur
files-device-q-domain = Domaine
files-device-q-password = Mot de passe
files-device-q-pim = PIM (facultatif)
files-device-q-anonymous = Se connecter anonymement
files-device-q-hidden = Volume caché
files-device-q-system = Volume système
files-device-q-remembered = Un mot de passe mémorisé est conservé dans votre trousseau par GVFS, pas par Fichiers.
files-device-q-this-request = Fichiers utilise ces identifiants pour cette demande uniquement.
files-device-q-unlock-button = Déverrouiller
files-device-q-close-app = Fermez l’application avant de réessayer.
files-device-q-host-explanation = Ne continuez que si vous vous attendez à vous connecter à ce serveur, par exemple sur votre propre réseau. SSH mémorise son identité pour la prochaine fois ; si elle change plus tard, Fichiers ne pourra pas se connecter tant qu’elle n’aura pas été vérifiée.
files-remember-never = Oublier le mot de passe immédiatement
files-remember-session = Le mémoriser jusqu’à la déconnexion
files-remember-keyring = Le mémoriser dans le trousseau

## Network: sidebar section and Connect to Server

files-network = Réseau
files-network-title = Se connecter à un serveur
files-network-bookmarks-failed = Impossible de mettre à jour les signets GTK : { $error }
files-network-wait-for-folder = Attendez le chargement du dossier, puis reconnectez-vous.
files-network-unavailable = Les emplacements réseau sont indisponibles.
files-network-dialog-failed = Impossible d’ouvrir Se connecter à un serveur.
files-network-window-closed = Sa fenêtre de Fichiers a été fermée.
files-network-open-failed = Impossible d’ouvrir le fichier : { $error }
files-network-location = Emplacement réseau
files-network-phone = Téléphone
files-network-camera = Appareil photo
files-network-disconnect = Déconnecter { $device }
files-network-remove-bookmark = Retirer le signet { $name }
files-network-other-locations = Autres emplacements…
files-network-address = Adresse du serveur
files-network-address-hint = smb://, sftp://, ftp://, ftps://, dav://, davs:// ou nfs://. Fichiers demande un mot de passe si le serveur en a besoin.
files-network-add-bookmark = Ajouter aux signets
files-network-bookmarked = Serveurs dans les signets
files-network-server = { $name } — { $uri }
files-network-on-this-network = Sur ce réseau
files-network-looking = Recherche…
files-network-no-servers = Aucun serveur n’a été trouvé. Saisissez une adresse ci-dessus.
files-network-empty = Cet emplacement est vide.
files-network-truncated = Seules les premières entrées sont affichées.
files-network-connect = Se connecter

## Clipboard, drag and drop, search

files-clipboard-cut-update-stopped = Le presse-papiers de fichiers s’est arrêté avant de mettre à jour la sélection coupée
files-clipboard-unavailable-retry = Le presse-papiers de fichiers démarre ou est indisponible. Réessayez.
files-clipboard-storage-not-ready = Le stockage source n’est pas prêt. Réessayez lorsqu’il sera disponible.
files-clipboard-identities-exhausted = Identifiants du presse-papiers épuisés. Redémarrez Fichiers.
files-clipboard-prepare-stopped = Le service des opérations s’est arrêté avant de préparer la sélection
files-clipboard-nothing-ready = Le presse-papiers ne contient aucune sélection de fichiers prête. Attendez la fin de Copier ou Couper, puis réessayez.
files-clipboard-stopped = Le presse-papiers de fichiers s’est arrêté avant de répondre
files-clipboard-selection-gone = La sélection préparée n’est plus disponible
files-clipboard-unavailable = Le presse-papiers de fichiers est indisponible
files-clipboard-not-accepted = Le presse-papiers de fichiers s’est arrêté avant d’accepter la sélection
files-clipboard-original-gone = La sélection d’origine est indisponible. Copiez ou coupez de nouveau les fichiers.
files-drag-worker-stopped = Le service de préparation du glisser s’est arrêté avant de répondre
files-drag-changed = La liste des fichiers glissés a changé ou a été annulée. Commencez un nouveau glisser.
files-drag-origin-unavailable = L’origine native du glisser est indisponible
files-drop-status-gone = L’état du transfert déposé n’est plus disponible. Consultez Opérations pour son résultat.
files-quick-look-unavailable = Le fichier demandé pour l’aperçu rapide est indisponible
files-search-could-not-start = La recherche n’a pas pu démarrer : { $error }
files-drag-read-failed = Impossible de lire les fichiers glissés : { $error }
files-clipboard-cut-not-updated = Le déplacement est terminé, mais sa sélection coupée n’a pas pu être mise à jour : { $error }. Copiez ou coupez de nouveau les fichiers restants.
files-drop-copy = Copier vers { $folder }
files-drop-move = Déplacer vers { $folder }
files-drop-refused = Impossible de déposer dans { $folder }
files-drop-already-there = Déjà dans { $folder }
files-drop-badge = { $action } · { $items }
files-drop-checking = Vérification de { $folder }

## View options for one folder

files-view-state-worker-stopped = Le service d’état d’affichage s’est arrêté avant l’enregistrement.
files-pixels = { $pixels } px
files-folder-reverse-order = Ordre inversé
files-folder-tree = Afficher les dossiers en arborescence
files-folders-first = Dossiers avant les fichiers
files-folder-modified-time = Heure de modification
files-folder-list-details = Détails de la liste
files-folder-remembered = Les choix sont mémorisés pour ce dossier.

## Custom Actions editor

files-actions-count-invalid = { $action } : { $field } doit être un nombre entier positif ou nul.
files-actions-error = { $action } : { $error }
files-actions-minimum = Éléments minimum
files-actions-maximum = Éléments maximum
files-actions-status-initial = Configurez une action, puis enregistrez. Ctrl+S enregistre toutes les actions.
files-actions-saved = Enregistré. Les actions sont prêtes dans toutes les fenêtres de Fichiers.
files-actions-reloaded = Rechargé. Vos actions enregistrées sont prêtes.
files-actions-new-name = Nouvelle action
files-actions-recording = Appuyez sur un raccourci. Échap annule ; Retour arrière efface.
files-actions-shortcut-modifier = Utilisez Ctrl, Alt, Super ou une touche de fonction pour le raccourci d’une action.
files-actions-subtitle = Commandes pour les fichiers et les dossiers
files-actions-changed-outside = La configuration a été modifiée en dehors de cet éditeur. Votre brouillon est conservé ; rechargez pour utiliser la configuration enregistrée.
files-actions-add = Ajouter une action
files-actions-delete = Supprimer
files-actions-discard-question = Abandonner les modifications non enregistrées des actions ?
files-actions-reload-question = Recharger et abandonner les modifications non enregistrées des actions ?
files-actions-details = Détails de l’action
files-actions-name-example = Redimensionner les images
files-actions-icon = Icône (facultative)
files-actions-command = Commande
files-actions-command-hint = %f premier chemin · %F tous les chemins · %n premier nom · %N tous les noms
    %d dossier actuel · %% pourcentage
    Mettez entre guillemets les arguments contenant des espaces. Appelez explicitement un shell lorsqu’une commande en a besoin.
files-actions-availability = Disponibilité
files-actions-patterns = Motifs de noms de fichiers
files-actions-patterns-hint = Séparez les motifs par un point-virgule.
files-actions-files = Fichiers
files-actions-folders = Dossiers
files-actions-count-hint = Un minimum de 0 autorise une sélection vide. Un maximum de 0 autorise n’importe quel nombre d’éléments.
files-actions-ways-to-run = Façons de l’exécuter
files-actions-shortcut = Raccourci (facultatif)
files-actions-record = Enregistrer un raccourci…
files-actions-toolbar = Afficher dans la barre d’outils
files-actions-undo-note = Les commandes personnalisées s’exécutent en dehors de l’historique d’annulation de Fichiers.
files-actions-empty = Aucune action personnalisée pour l’instant
files-actions-empty-description = Ajoutez une commande au menu contextuel ou attribuez-lui un raccourci clavier.
files-actions-save = Enregistrer les actions

## Quick Look and media playback

files-preview-window-failed-browser = Impossible d’ouvrir un aperçu flottant pour ce navigateur
files-preview-window-failed-trash = Impossible d’ouvrir un aperçu flottant pour la corbeille
files-preview-window-failed-recovery = Impossible d’ouvrir un aperçu flottant pour Récupération
files-preview-stopped = La préparation de l’aperçu s’est arrêtée
files-preview-title = Aperçu rapide : { $name }
files-preview-close = Fermer l’aperçu
files-preview-zoom-out = Zoom arrière
files-preview-fit = Ajuster à la fenêtre
files-preview-actual-size = Taille réelle
files-preview-zoom-in = Zoom avant
files-preview-fit-short = Ajuster
files-preview-actual-short = 1:1
files-preview-open-with = Ouvrir avec { $app }
files-preview-previous-item = Élément précédent (Haut)
files-preview-next-item = Élément suivant (Bas)
files-preview-position = { $index } sur { $count }
files-preview-page = { $page } / { $pages }
files-preview-preparing = Préparation de l’aperçu…
files-preview-unavailable = Aperçu indisponible : { $error }
files-preview-folder = { $count ->
    [one] Dossier · { $count } élément
   *[other] Dossier · { $count } éléments
}
files-preview-media-unavailable = L’aperçu du média est indisponible. Ouvrez-le dans votre application habituelle.
files-preview-none = Aucun aperçu disponible. Ouvrez-le dans l’application habituelle.
files-preview-select = Sélectionnez un fichier à prévisualiser
files-preview-text-truncated = Texte tronqué
files-preview-preparing-short = Préparation…
files-preview-text = Aperçu du texte
files-preview-image = Aperçu de l’image
files-preview-pdf = Aperçu du PDF
files-preview-audio = Aperçu audio
files-preview-audio-short = Audio
files-preview-video = Aperçu vidéo
files-preview-video-short = Vidéo
files-media-preparing = Préparation du média…
files-media-stopped = Aperçu du média arrêté.
files-media-preparing-frame = Préparation de l’image vidéo…
files-media-playing = Lecture audio
files-media-ended = Lecture terminée
files-media-paused = Lecture en pause
files-media-ready = Prêt à lire
files-media-play = Lire
files-media-pause = Pause
files-media-mute = Couper le son
files-media-unmute = Rétablir le son
files-media-time = { $elapsed } / { $total }
files-media-time-seeking = { $elapsed } / { $total } · Déplacement…
files-media-seek = Position
files-media-seek-unavailable = Déplacement impossible
files-media-volume = Volume { $volume }

## Recovery items (why a version is kept)

files-recovery-kept-interrupted = Conservé par une opération interrompue. Récupérer une copie laisse la version enregistrée intacte.
files-recovery-newer-work = Cette version conservée contient des travaux plus récents ou inattendus. Elle est protégée du nettoyage automatique.
files-recovery-protected = Cette version est protégée du nettoyage automatique. Récupérer une copie la laisse intacte.
files-recovery-needed-by-undo = Encore nécessaire à une entrée d’annulation. Récupérer une copie la laisse disponible pour l’annulation.
files-recovery-undo-expired = Conservée après l’expiration de son entrée d’annulation. Récupérer une copie laisse la version conservée intacte.
files-recovery-saved-return = Retour interrompu d’une destination enregistrée. Conserver dans Récupération préserve son dossier de travail d’origine et ne valide que le retour entrant terminé.
files-recovery-saved-returned = La destination enregistrée a déjà été rétablie. Conserver dans Récupération préserve ce dossier de travail et termine l’annulation enregistrée sans rejouer son retour.
files-recovery-copy-undo = Dossier de travail d’une annulation de copie interrompue. Conserver dans Récupération préserve son contenu et laisse l’historique d’annulation d’origine disponible.
files-recovery-undo-cleanup = Dossier de travail d’une annulation terminée. Conserver dans Récupération préserve son contenu sans rejouer l’étape d’annulation terminée.
files-recovery-undo-return = Retour d’annulation interrompu. Conserver dans Récupération préserve tout ce dossier de travail et laisse l’historique d’annulation d’origine inchangé.
files-recovery-undo-recovery = Récupération d’annulation interrompue. Conserver dans Récupération préserve chaque dossier de travail et laisse l’historique d’annulation d’origine disponible.
files-recovery-work-folder = Contient tous les fichiers de ce dossier de travail. Conserver dans Récupération préserve tous les dossiers de travail et vous permet de poursuivre le transfert d’origine.
files-recovery-interrupted-file = Fichier interrompu. Conserver dans Récupération préserve tous les dossiers de travail et vous permet de poursuivre le transfert d’origine.
files-recovery-copy-cancelled = Copie de récupération annulée avant le début d’autres travaux.
files-recovery-copy-attention = La copie de récupération nécessite votre attention
files-recovery-identities-exhausted = Identifiants du catalogue de Récupération épuisés. Fermez puis rouvrez Récupération.
files-recovery-reader-stopped-reopen = Le lecteur de Récupération s’est arrêté. Fermez puis rouvrez Récupération.
files-recovery-read-cancelled = Lecture de Récupération annulée
files-recovery-queue-stopped = La file des opérations s’est arrêtée pendant la lecture de Récupération.
files-recovery-refresh-unavailable = Actualisation automatique indisponible pour { $path } : { $error }. Utilisez Actualiser.

## Device helper results

files-device-request-unsupported = Demande de périphérique non prise en charge ou en cours d’arrêt.
files-device-not-local = Le périphérique n’est plus disponible en tant que dossier local.
files-device-outside-mount = L’emplacement monté se trouve en dehors de son point de montage.
files-transfer-partial-kept = Les données partiellement copiées ont été conservées dans Récupération.
files-device-helper-stopped = L’assistant des périphériques s’est arrêté avant de confirmer cette demande. Le périphérique a peut-être changé. { $message }
files-transfer-destination-changed = Le dossier de destination est indisponible ou a changé : { $error }
files-transfer-source-retained = La source est conservée ; sa destination modifiée reste également : { $error }
files-undo-trash-info-unverified = Les informations de la corbeille ne peuvent pas être vérifiées : { $error }
files-jobs-completed-before-interruption = L’opération de fichiers n° { $operation } s’est terminée avant l’interruption ; son résultat enregistré a été restauré.
files-jobs-result-restored = { $detail } · résultat enregistré restauré après l’interruption

## Device requests: refusals and errors

files-device-already-settled = Cette demande de périphérique est déjà terminée.
files-device-no-more-answers = Cette demande de périphérique n’accepte plus de réponses.
files-device-observation-changed = L’état observé du périphérique a changé. Actualisez et réessayez.
files-device-gone = Le périphérique n’est plus disponible.
files-device-unsupported-operation = Le périphérique ne prend plus en charge cette opération.
files-device-request-id-used = Cet identifiant de demande de périphérique a déjà été utilisé.
files-device-another-request = Une autre demande de périphérique est encore en cours.
files-device-address-invalid = L’adresse du serveur n’est pas valide.
files-device-cannot-list = Cet emplacement ne peut pas être listé.
files-device-still-listing = Fichiers liste encore d’autres emplacements. Réessayez dans un instant.
files-device-listing-timeout = Le listage de cet emplacement a pris trop de temps.
files-device-capability-gone = Cette capacité du périphérique n’est plus disponible.
files-device-question-changed = Cette question du périphérique a changé.
files-device-invalid-credentials = Identifiants du périphérique non valides.
files-device-answer-mismatch = La réponse ne correspond pas à la question actuelle du périphérique.
files-device-runtime-stopping = Le service des périphériques s’arrête.
files-device-question-changed-or-cancelled = Cette question du périphérique a changé ou a été annulée.
files-device-prepare-failed = Impossible de préparer la demande de périphérique.
files-device-request-too-large = La demande de périphérique dépasse sa taille maximale.
files-device-helper-unavailable = L’assistant des périphériques est indisponible.
files-device-helper-full = L’assistant des périphériques ne peut pas accepter d’autres demandes.
files-device-notifications-pending = Des notifications de périphériques sont en attente. Attendez leur envoi avant de lancer d’autres travaux sur les périphériques.
files-device-removal-in-progress = Un retrait de périphérique est encore en cours.
files-device-observation-changed-refresh = L’état observé de ce périphérique a changé. Actualisez et réessayez.
files-device-cannot-remove = Ce périphérique ne peut pas être retiré pour l’instant.
files-device-cannot-mount = Ce périphérique ne peut pas être monté pour l’instant.
files-device-helper-stopped-short = L’assistant des périphériques s’est arrêté.
files-device-work-active = Un travail sur les périphériques est encore actif ou déjà en cours d’arrêt.
files-device-location-stale = Cet emplacement de périphérique n’est plus à jour. Sélectionnez de nouveau le périphérique pour l’ouvrir.

## Device notifications

files-device-notification-decision = Un périphérique nécessite une décision
files-device-notification-mounted = Périphérique monté.
files-device-notification-remote = Connecté, mais GVFS ne propose aucun dossier local pour cet emplacement.
files-device-notification-powered-off = Périphérique éteint.
files-device-notification-ejected = Périphérique éjecté.
files-device-notification-unmounted = Périphérique démonté.

## Operation and clipboard refusals

files-jobs-attention-full = La file des avis d’opérations est pleine. Examinez le travail en attente avant d’en soumettre davantage.
files-clipboard-other-runtime = La sélection précédente du presse-papiers appartient à une autre session de Fichiers. Copiez ou coupez une nouvelle sélection.
files-clipboard-other-history = Cette sélection appartient à un autre historique d’opérations ou à une autre session de Fichiers. Sélectionnez de nouveau les fichiers.
files-clipboard-cut-pasting = Cet élément coupé est déjà en cours de collage. Attendez son résultat avant de coller à nouveau.
files-clipboard-cut-finished = Ce couper est terminé. Copiez ou coupez une nouvelle sélection.
files-clipboard-cut-pasting-continue = Cet élément coupé est déjà en cours de collage. Attendez son résultat avant de continuer.
files-dismissal-not-ready = Cette demande ne peut pas encore être écartée. Terminez ou annulez le travail en cours et enregistrez d’abord son résultat.
files-dismissal-unavailable = La demande est indisponible.
files-dismissal-stopped-only = Seule une demande ordinaire arrêtée peut être écartée.
files-dismissal-done = Demande écartée après examen. Le travail effectué et l’annulation sont préservés.
files-drop-destination-changed = La destination du dépôt a changé. Vérifiez le dossier et commencez un nouveau glisser.
files-drag-ended = Ce glisser est déjà terminé. Commencez un nouveau glisser.
files-drag-too-many-folders = Ce glisser a déjà vérifié trop de dossiers. Commencez un nouveau glisser.
files-drop-binding-invalid = Un dépôt nécessite une destination absolue et le dossier d’origine.
files-drop-outside-folder = La cible du dépôt se trouve en dehors du dossier d’origine du navigateur.
files-drop-same-folder = Les éléments sont déjà dans ce dossier.
files-drag-cancelled = Préparation du glisser annulée.
files-recovery-not-ready = Cette copie de récupération n’est pas prête à continuer.
files-recovery-request-gone = La demande de récupération n’est plus disponible.
files-recovery-no-plan = Il ne reste aucun plan de récupération exact pour cet élément. Actualisez Récupération pour choisir une nouvelle copie.
files-recovery-handed-off = Cette suite de récupération a déjà été transmise ou est indisponible.
files-original-destination-gone = L’observation de la destination d’origine est indisponible.
files-original-source-gone = L’observation de la source d’origine est indisponible.
files-recovery-not-recovery = Le résultat arrêté n’est pas une copie de récupération.
files-rename-needs-entry = Le renommage nécessite un élément sélectionné et un dossier absolus.
files-rename-other-runtime = Ce renommage appartient à une autre session de Fichiers. Sélectionnez de nouveau l’élément.
files-rename-accepted = Ce renommage a déjà été accepté. Sélectionnez de nouveau l’élément.
files-transfer-not-ready = Ce transfert n’est pas prêt à continuer.
files-transfer-no-plan = Aucun plan de transfert actif exact ne reste pour cette demande. Les résultats enregistrés doivent être examinés avant tout nouveau travail.
files-transfer-handed-off = Cette partie du transfert a déjà été transmise ou est indisponible.
files-original-destination-placement-gone = L’emplacement de destination d’origine est indisponible.
files-original-source-placement-gone = L’emplacement source d’origine est indisponible.
files-transfer-not-transfer = Le résultat arrêté n’est pas un transfert.

## Restore from Trash results

files-restore-needs-finalization = La restauration est enregistrée ; la finalisation de l’historique doit être examinée.
files-restore-needs-review = La restauration doit être examinée avant d’autres modifications.
files-restore-cancelled = Restauration annulée avant le déplacement de l’élément.
files-restore-not-restored = L’élément conservé n’a pas été restauré.
files-restore-done = Élément restauré.

## Storage checks

files-storage-not-prepared = La protection du stockage n’est pas prête. Attendez le démarrage de Fichiers, ou redémarrez Fichiers si la surveillance du stockage a échoué.
files-storage-cancelled = La vérification du stockage a été annulée.
files-storage-location-changed = L’emplacement parcouru a changé après la demande. Vérifiez l’emplacement voulu et réessayez.
files-storage-not-returned = Le stockage d’origine de cette demande n’est pas revenu. Reconnectez le périphérique voulu avant de réessayer, ou lancez une nouvelle opération après avoir vérifié les emplacements.
files-storage-device-changed = Le périphérique à cet emplacement a changé après la demande. Vérifiez le périphérique voulu et réessayez.
files-storage-changed = Le stockage a changé pendant la préparation de la demande. Réessayez l’opération.
files-storage-device-unavailable = Le périphérique de cet emplacement est indisponible. Reconnectez le périphérique voulu et réessayez l’opération.
files-device-remove-named = Retirer { $device }
files-device-connect-named = Se connecter à { $device }
files-device-notification-connected = Connecté.
files-device-notification-cancelled = Annulé.

## Network locations: missing backends and connection failures

files-network-missing-gvfs = Fichiers ne peut pas ouvrir { $kind ->
    [smb] les partages Windows (SMB)
    [sftp] les serveurs SSH (SFTP)
    [ftp] les serveurs FTP
    [dav] les serveurs WebDAV
    [nfs] les serveurs NFS
    [mtp] les téléphones et lecteurs multimédias (MTP)
    [ptp] les appareils photo (PTP)
    [afc] les iPhone et iPad
    [network] la navigation dans le réseau local
   *[other] ce type d’emplacement
} : GVFS n’est pas installé ou son module { $scheme } est absent. Installez le paquet gvfs.
files-network-missing-package = Fichiers ne peut pas ouvrir { $kind ->
    [smb] les partages Windows (SMB)
    [sftp] les serveurs SSH (SFTP)
    [ftp] les serveurs FTP
    [dav] les serveurs WebDAV
    [nfs] les serveurs NFS
    [mtp] les téléphones et lecteurs multimédias (MTP)
    [ptp] les appareils photo (PTP)
    [afc] les iPhone et iPad
    [network] la navigation dans le réseau local
   *[other] ce type d’emplacement
} : installez le paquet { $package }, puis réessayez.
files-network-missing-backend = Fichiers ne peut pas ouvrir { $kind ->
    [smb] les partages Windows (SMB)
    [sftp] les serveurs SSH (SFTP)
    [ftp] les serveurs FTP
    [dav] les serveurs WebDAV
    [nfs] les serveurs NFS
    [mtp] les téléphones et lecteurs multimédias (MTP)
    [ptp] les appareils photo (PTP)
    [afc] les iPhone et iPad
    [network] la navigation dans le réseau local
   *[other] ce type d’emplacement
} : aucun module GVFS ne le prend en charge.
files-network-unreachable = Fichiers ne parvient pas à joindre { $server }. Vérifiez que le serveur est allumé et connecté au réseau, et que l’adresse est correcte ({ $detail }).
files-network-login-refused = { $server } a refusé la connexion. Vérifiez le nom d’utilisateur, le domaine et le mot de passe ({ $detail }).
files-network-connect-failed = Fichiers n’a pas pu se connecter à { $server }.
files-network-connect-failed-detail = Fichiers n’a pas pu se connecter à { $server } : { $detail }.

## Operation results that need review

files-drop-destination-unavailable = La destination du dépôt a changé ou est indisponible : { $error }
files-jobs-completion-unverified = L’achèvement enregistré n’a pas pu être vérifié ; examinez le travail interrompu : { $error }
files-jobs-completion-not-saved = Résultat de l’achèvement enregistré non sauvegardé : { $error }
files-jobs-transfer-unverified = Le transfert examiné n’a pas pu être vérifié : { $reason }
files-jobs-recorded-transfer-unverified = Le transfert enregistré n’a pas pu être vérifié ; examinez le travail interrompu : { $error }
files-jobs-transfer-end-not-saved = Fin du transfert non enregistrée : { $error }
files-jobs-transfer-result-not-saved = Résultat du transfert enregistré non sauvegardé : { $error }
files-jobs-transfer-receipt-not-saved = Le travail sur les fichiers est terminé, mais son reçu de fin de transfert n’a pas été enregistré : { $error }
files-jobs-receipt-not-saved = Le travail sur les fichiers est terminé, mais son reçu de fin n’a pas été enregistré : { $error }

## File clipboard

files-clipboard-changed = Le presse-papiers a changé pendant sa lecture. Collez de nouveau.
files-clipboard-busy = Le presse-papiers est occupé. Réessayez.
files-clipboard-busy-paste = Le presse-papiers est occupé. Attendez la fin de Copier ou Couper, puis collez de nouveau.
files-clipboard-owner-silent = Le propriétaire du presse-papiers n’a pas répondu. Copiez de nouveau les fichiers.
files-clipboard-not-accepted-compositor = Le compositeur n’a pas accepté la sélection de fichiers. Copiez de nouveau les fichiers.
files-clipboard-compositor-silent = Le compositeur n’a pas répondu pendant le démarrage du presse-papiers de fichiers
files-clipboard-starting = Le presse-papiers de fichiers démarre. Réessayez.
files-clipboard-disconnected = Le presse-papiers de fichiers Wayland a été déconnecté
files-clipboard-replaced = Le presse-papiers a été remplacé avant la fin de Copier ou Couper
files-clipboard-unsupported = Le compositeur ne prend pas en charge le protocole du presse-papiers de fichiers (ext-data-control-v1)
files-clipboard-no-seat = Aucun poste d’entrée Wayland n’est disponible
files-clipboard-read-unfinished = La lecture du presse-papiers ne s’est pas terminée
files-clipboard-worker-stopped = Le service du presse-papiers de fichiers s’est arrêté de manière inattendue
files-clipboard-busy-unavailable = Le presse-papiers de fichiers est occupé ou indisponible. Réessayez.
files-clipboard-no-files = Le presse-papiers ne contient aucun fichier
files-clipboard-too-large = La liste de fichiers du presse-papiers est trop longue
files-clipboard-invalid-location = Le presse-papiers contient un emplacement de fichier non valide. Copiez de nouveau les fichiers.
files-clipboard-non-local = La liste de fichiers contient un emplacement de fichier non local

## Properties, playback and device removal

files-properties-too-many = Les propriétés prennent en charge 64 éléments au maximum.
files-media-cancelled = Préparation du média annulée.
files-media-no-video-output = La sortie vidéo est indisponible.
files-media-cannot-play = Ce média n’a pas pu être lu. Ouvrez-le dans votre application habituelle.
files-media-no-seek = Le déplacement est impossible dans ce média.
files-media-timed-out = La préparation du média a expiré.
files-media-shut-down = La lecture des médias s’est arrêtée.
files-media-closing = La lecture des médias est encore en cours de fermeture.
files-media-playback-stopped = La lecture des médias est arrêtée.
files-media-controls-busy = Les commandes de lecture sont occupées.
files-removal-in-progress = Un retrait de périphérique est déjà en cours.
files-removal-wait = Attendez la fin des opérations de Fichiers et des actions personnalisées, ou annulez-les, avant de retirer un périphérique.
files-removal-cancelled = Travail annulé avant la fin du retrait du périphérique.
files-actions-wait-notifications = Attendez les notifications des actions personnalisées avant d’arrêter le service.
files-actions-runtime-stopped-reply = Le service des actions personnalisées s’est arrêté avant de répondre.
files-actions-runtime-busy = Le service des actions personnalisées est occupé ou arrêté.
files-app-work-pending = Fichiers a encore du travail accepté ou des résultats non enregistrés.
files-configuration-other-source = Cette configuration appartient à une autre sélection de source.
files-configuration-worker-stopped = Le service de configuration s’est arrêté.

## Configuration checks of the toolbar, places and shortcuts

files-config-toolbar-location = la barre d’outils doit contenir exactement un champ d’emplacement et au plus 64 éléments
files-config-toolbar-unknown = élément de barre d’outils inconnu ou en double
files-config-too-many-places = 256 emplacements au maximum sont pris en charge
files-config-duplicate-place = emplacement intégré en double
files-config-list-columns-invalid = les largeurs des colonnes de la liste sont hors de leur plage
files-config-bookmark-invalid = un signet nécessite un nom et un dossier local absolu
files-config-too-many-shortcuts = huit raccourcis au maximum sont pris en charge par commande
files-config-action-modifier = les actions personnalisées nécessitent un raccourci avec Ctrl, Alt, Super ou une touche de fonction
files-config-reserved-keys = les touches de sélection, de focus, Échap, du menu contextuel et de saisie sans modificateur restent disponibles
files-config-shortcut-too-long = le raccourci est trop long
files-config-modifier-invalid = modificateur de raccourci non valide
files-config-modifier-unsupported = modificateur de raccourci non pris en charge
files-config-modifier-duplicate = modificateur de raccourci en double
files-config-accelerator-hint = utilisez un raccourci tel que <Control><Shift>f ou F6
files-config-shortcut-conflict = « { $label } » et « { $other } » utilisent le même raccourci ({ $binding })

## Custom action and configuration checks

files-action-invalid = action personnalisée non valide
files-action-needs-file = %f nécessite un fichier sélectionné
files-action-needs-name = %n nécessite un fichier sélectionné
files-config-defaults-read-only = les valeurs par défaut intégrées ne peuvent pas être modifiées
files-config-needs-parent = la configuration nécessite un dossier parent
files-config-too-many-actions = 256 actions personnalisées au maximum sont prises en charge

## Operation result summaries (facts joined with " · ")

files-report-completed = { $count ->
    [one] { $count } terminé
   *[other] { $count } terminés
}
files-report-failed = { $count ->
    [one] { $count } en échec
   *[other] { $count } en échec
}
files-report-skipped = { $count ->
    [one] { $count } ignoré
   *[other] { $count } ignorés
}
files-report-remaining = { $count ->
    [one] { $count } restant
   *[other] { $count } restants
}
files-report-cancelled = annulé
files-report-needs-attention = attention requise
files-report-source-folders = { $count ->
    [one] { $count } dossier source à examiner
   *[other] { $count } dossiers source à examiner
}
files-report-failed-handed-off = travail en échec transmis
files-report-failed-abandoned = travail en échec abandonné
files-report-remaining-handed-off = travail restant transmis
files-report-remaining-abandoned = travail restant abandonné
files-report-confirmed-portion = { $detail } · partie confirmée ; le travail inachevé doit être réconcilié
files-report-handed-to-request = { $detail } · travail restant transmis à une autre demande
files-report-trashed = { $count ->
    [one] { $count } mis à la corbeille
   *[other] { $count } mis à la corbeille
}
files-report-deleted = { $count ->
    [one] { $count } supprimé définitivement
   *[other] { $count } supprimés définitivement
}
files-report-need-review = { $count ->
    [one] { $count } à examiner
   *[other] { $count } à examiner
}
files-report-not-started = { $count ->
    [one] { $count } non démarré
   *[other] { $count } non démarrés
}

## Preview errors

files-preview-regular-only = seuls les fichiers ordinaires peuvent être prévisualisés
files-preview-too-large = le fichier dépasse la taille maximale de l’aperçu

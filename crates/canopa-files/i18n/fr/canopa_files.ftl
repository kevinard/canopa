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

files-app-name = Fichiers
files-toolbar = Barre d’outils
files-tabs = Onglets
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
files-view-columns = Colonnes
files-view-list = Liste
files-view-icons = Icônes
files-status-items = { $count ->
    [one] { $count } élément
   *[other] { $count } éléments
}
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
files-preferences-page-locations = Emplacements
files-preferences-page-shortcuts = Raccourcis
files-preferences-status-initial = Les modifications s’appliquent à toutes les fenêtres de Fichiers après l’enregistrement.
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
files-preferences-saved = Enregistré. Les préférences s’appliquent à toutes les fenêtres de Fichiers.
files-preferences-reloaded = Préférences partagées de Fichiers rechargées.

## Preferences window: controls

files-preferences-subtitle = Choisissez l’apparence et le comportement de Fichiers.
files-preferences-read-only = Les valeurs par défaut intégrées sont en lecture seule. Démarrez Fichiers avec une configuration modifiable pour enregistrer les préférences.
files-preferences-changed-outside = La configuration a été modifiée en dehors de cet éditeur. Votre brouillon est conservé ; rechargez avant d’enregistrer.
files-preferences-discard-question = Abandonner les modifications non enregistrées des préférences ?
files-preferences-reload-question = Recharger et abandonner les modifications non enregistrées des préférences ?
files-preferences-keep-editing = Continuer la modification
files-preferences-discard = Abandonner
files-preferences-reload = Recharger
files-preferences-save = Enregistrer les préférences
files-preferences-keys = Alt+1–4 pages · Ctrl+S Enregistrer · Ctrl+R Recharger
files-preferences-default-view = Vue par défaut
files-preferences-sort-by = Trier par
files-preferences-descending = Ordre décroissant
files-preferences-column-layout = Disposition des colonnes
files-preferences-column-width = Largeur des colonnes
files-preferences-column-width-range = 160 à 640 pixels
files-preferences-list-details = Détails de la vue en liste
files-preferences-modified-date = Date de modification
files-preferences-thumbnails = Miniatures
files-preferences-icon-size = Taille des icônes
files-preferences-views-note = Les dossiers héritent de ces réglages sauf s’ils ont leurs propres options d’affichage. « Fichiers locaux uniquement » ignore les partages réseau et les supports amovibles.
files-preferences-location-field = Champ d’emplacement
files-preferences-toolbar-action = Action : { $name }
files-preferences-toolbar-note = Sélectionnez un élément de la barre d’outils pour le déplacer ou le retirer.
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
files-preferences-locations-footer = Les dossiers manquants restent dans les signets. Les fenêtres étroites conservent la navigation par emplacement dans le menu du dossier.
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

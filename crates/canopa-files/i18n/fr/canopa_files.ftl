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

## Actions menu (toolbar menu and context menu)

files-menu-open = Ouvrir
files-menu-open-with = Ouvrir avec…
files-menu-new-folder = Nouveau dossier…
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
files-home-path = Dossier personnel / { $path }

## Permanent deletion confirmation

files-delete-title = Supprimer définitivement ?
files-delete-title-empty-trash = Vider la corbeille ?
files-delete-title-no-trash = Impossible de mettre à la corbeille
files-delete-one = Supprimer définitivement « { $name } » ? Cette action est irréversible.
files-delete-many = { $count ->
    [one] Supprimer définitivement { $count } élément ? Cette action est irréversible.
   *[other] Supprimer définitivement { $count } éléments ? Cette action est irréversible.
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
files-recent-activity = Activité récente
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
files-properties-access = Accès
files-properties-owner = Propriétaire
files-properties-group = Groupe
files-properties-others = Autres
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
files-permission-none = aucun
files-permission-x = exécution
files-permission-w = écriture
files-permission-wx = écriture et exécution
files-permission-r = lecture
files-permission-rx = lecture et exécution
files-permission-rw = lecture et écriture
files-permission-rwx = lecture, écriture et exécution

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

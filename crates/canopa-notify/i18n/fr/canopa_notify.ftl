# Canopa : Notifications.
# Traduction française : assistée par machine, à relire (machine-assisted, needs review).

## Banners

notify-banner-window-title = Bannières de notification
notify-banner-region = Notifications de Canopa Notify
notify-banner-announcement = { $app } : { $summary }. { $body }
notify-banner-dismiss = Fermer la notification
notify-banner-open = Ouvrir la notification
notify-reply-placeholder = Répondre…
notify-reply-send = Envoyer
notify-reply-cancel = Annuler
notify-hidden-summary = Nouvelle notification
notify-lock-unnamed-app = Application

## Boutons de portail sans libellé

notify-portal-reply = Répondre
notify-portal-answer = Décrocher
notify-portal-decline = Refuser
notify-portal-hang-up = Raccrocher
notify-portal-speaker-on = Activer le haut-parleur
notify-portal-speaker-off = Désactiver le haut-parleur

## Centre de notifications

notify-center-title = Notifications
notify-center-settings-title = Paramètres des notifications
notify-center-title-dnd = Notifications · Ne pas déranger : { $state }
notify-center-title-suppressed = Notifications · { $suppression }
notify-center-received = Reçue
notify-center-unread = Non lue
notify-center-unknown-app = Application inconnue
notify-center-reply = Répondre…
notify-center-open = Ouvrir la notification
notify-center-dismiss = Fermer
notify-center-mark-read = Marquer comme lue
notify-center-remove = Supprimer
notify-center-clear-active = Effacer les notifications actives
notify-center-delete-history = Supprimer l’historique des notifications
notify-center-mark-all-read = Tout marquer comme lu
notify-center-clear-all = Tout effacer
notify-center-undo-clear = Annuler l’effacement
notify-center-app-permission = Autorisation de l’application
notify-center-category-permission = Catégorie : { $category }
notify-center-first-page = Première page
notify-center-next-page = Page suivante
notify-center-previous-page = Page précédente
notify-center-settings = Paramètres
notify-center-back = Retour aux notifications
notify-center-group-by-app = Regrouper par application
notify-center-timeline = Afficher la chronologie
notify-center-action-too-large = Une action de notification enregistrée dépasse la taille d’affichage autorisée.
notify-center-item-too-large = Une notification enregistrée dépasse la taille d’affichage autorisée.
notify-center-daemon-unavailable = Le service Canopa Notify est indisponible à l’adresse { $socket } : { $error }
notify-center-stream-invalid = Le flux du centre de notifications s’est terminé par une trame invalide ou incomplète ; rouvrez le panneau.
notify-center-too-many-subscriptions = Trop d’abonnements au centre de notifications ; fermez un panneau et réessayez.
notify-center-too-large = Le centre de notifications dépasse la limite d’affichage de 1 Mio ; utilisez les commandes de l’historique des notifications.

## Choix de distribution

notify-permission-allow = Autoriser
notify-permission-silence = Silencieux
notify-permission-drop = Bloquer
notify-delivery = Distribution
notify-delivery-allow = Afficher les bannières
notify-delivery-silence = Enregistrer sans bruit
notify-delivery-drop = Bloquer

## Vue groupée

notify-group-summary = { $count ->
    [one] { $count } notification
   *[other] { $count } notifications
}
notify-group-summary-unread = { $count ->
    [one] { $count } notification
   *[other] { $count } notifications
} · { $unread ->
    [one] { $unread } non lue
   *[other] { $unread } non lues
}
notify-group-show = Afficher
notify-group-collapse = Réduire
notify-group-more = { $count ->
    [one] et { $count } autre
   *[other] et { $count } autres
}

## Ne pas déranger

notify-dnd-title = Ne pas déranger
notify-dnd-off = Désactivé
notify-dnd-on = Activé
notify-dnd-on-until = Activé jusqu’à { $until }
notify-dnd-scheduled = Activé (planification)
notify-dnd-scheduled-until = Activé (planification) jusqu’à { $until }
notify-dnd-for-hour = Pendant 1 heure
notify-dnd-until-tomorrow = Jusqu’à demain
notify-dnd-with-suppression = { $state } · { $suppression }
notify-suppression-fullscreen = Plein écran : les bannières sont enregistrées sans bruit
notify-suppression-sharing = Partage d’écran : les bannières sont masquées

# Canopa : Verrouillage de session et fond d’écran.
# Traduction française : assistée par machine, à relire (machine-assisted, needs review).

## Lock screen

lock-title = Verrouillage Canopa
lock-heading = Déverrouiller la session
lock-subtitle = Saisissez votre mot de passe pour déverrouiller
lock-unlock = Déverrouiller
lock-try-again = Réessayer
lock-starting-authentication = Démarrage de l’authentification…
lock-timed-out = Le délai d’authentification est dépassé. Appuyez sur Entrée pour réessayer.
lock-could-not-start = Impossible de démarrer l’authentification. Appuyez sur Entrée pour réessayer.
lock-failed = Mot de passe incorrect. Réessayez.
lock-cancelled = Authentification annulée. Appuyez sur Entrée pour réessayer.
lock-password-change-required = Changement de mot de passe requis
lock-password-expired = Votre mot de passe a expiré. Changez-le pour déverrouiller, ou annulez pour rester verrouillé.
lock-change-password = Changer le mot de passe
lock-cancel-password-change = Annuler le changement de mot de passe
lock-change-password-shortcut = Changer le mot de passe · Entrée
lock-cancel-shortcut = Annuler · Échap

## Wallpaper picker

wallpaper-title = Fond d’écran Canopa
wallpaper-apply = Appliquer le fond d’écran
wallpaper-applying = Application…
wallpaper-close = Fermer
wallpaper-selection = Fond d’écran  ·  { $name }  ·  { $position } sur { $count }
wallpaper-all-monitors = Tous les écrans
wallpaper-previous-image = Image précédente   · ←
wallpaper-next-image = Image suivante   · →
wallpaper-monitor = Écran : { $output }   · ↑/↓
wallpaper-fit = Ajustement : { $fit }   · F
wallpaper-fit-fill = Remplir
wallpaper-fit-fit = Adapter
wallpaper-fit-stretch = Étirer
wallpaper-fit-center = Centrer
wallpaper-close-shortcut = Fermer   · Échap
wallpaper-applying-message = Application du fond d’écran…
wallpaper-preparing-preview = Préparation de l’aperçu de l’écran…
wallpaper-preview-hint = Entrée applique et enregistre · Tab sélectionne · F5 actualise
wallpaper-preview-failed = Impossible d’afficher l’aperçu de cette image. Choisissez-en une autre.
wallpaper-applied = Fond d’écran appliqué et enregistré
wallpaper-apply-failed = Échec du changement de fond d’écran ; l’attribution précédente est conservée
wallpaper-none-found = Aucune image n’a été trouvée dans votre dossier Images ni parmi les fonds d’écran installés. Choisissez plutôt un fichier image.

## canopa-idle
idle-not-installed = { $program } n’est pas installé dans le PATH
idle-already-running = Le superviseur de mise en veille de Canopa est déjà lancé pour cet affichage
idle-locker-missing = Installez canopa-lock à côté de canopa-idle
idle-coordinator-unavailable = Le service de verrouillage et de veille de Canopa ne tourne pas pour la session choisie ; démarrez d’abord canopa-lock.service
idle-suspend-timeout = La demande de mise en veille a expiré
idle-config-missing = Aucune configuration Stasis ; créez-en une avec canopa idle init
idle-config-invalid = La configuration Stasis doit être un fichier ordinaire d’au plus 1 Mio
idle-stasis-exited = Stasis s’est arrêté : { $status }
idle-coordinator-stopped = Le service de verrouillage et de veille de Canopa s’est arrêté

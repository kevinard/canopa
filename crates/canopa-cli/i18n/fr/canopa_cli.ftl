# Canopa : Interface en ligne de commande.
# Traduction française : assistée par machine, à relire (machine-assisted, needs review).

## Aide

cli-about = Outils du bureau Canopa
cli-settings-about = Ouvrir les Paramètres de Canopa pour les périphériques, l’apparence, le système et le bureau
cli-wallpaper-about = Service de fond d’écran et contrôle des images animées
cli-lock-about = Verrouillage sécurisé de la session ; passez preview ou --help pour les outils de rendu
cli-greeter-about = Interface graphique de connexion greetd et outils d’aperçu
cli-polkit-about = Agent graphique d’authentification polkit et outils d’aperçu
cli-pinentry-about = Boîte de dialogue de code PIN ou de phrase secrète GnuPG et outils d’aperçu
cli-idle-about = Politique d’inactivité Stasis, état de la diffusion et intégration de la mise en veille sécurisée
cli-config-about = Vérifier ou migrer la configuration de Canopa
cli-config-check-about = Vérifier tous les schémas propriétaires. Sans fichier, utiliser l’ordre de recherche habituel
cli-config-check-file = Fichier Canopa partagé (jamais interprété comme un fichier ancien)
cli-config-check-show-source = Afficher les diagnostics locaux des schémas, qui peuvent inclure le contenu de la configuration
cli-config-migrate-about = Envelopper les sources anciennes indiquées, vérifier tous les schémas, puis créer un nouveau fichier
cli-config-migrate-long-about = Envelopper les sources anciennes indiquées, vérifier tous les schémas, puis créer un nouveau fichier.
    Un composant omis utilise ses valeurs par défaut intégrées. Les fichiers existants ne sont jamais remplacés.

## Messages

cli-check-failed = la vérification de la configuration a échoué
cli-migrate-no-source = indiquez au moins une source ancienne
cli-migrate-too-large = la configuration combinée dépasse 1 Mio
cli-migrate-rejected = migration refusée par le schéma d’un composant
cli-migrate-created = { $path } a été créé. Installez ce fichier à l’emplacement de la configuration partagée (ou sélectionnez-le explicitement), puis redémarrez Canopa Shell et rechargez Notify.
cli-no-installation-directory = impossible de trouver le dossier d’installation
cli-validator-cannot-run = impossible d’exécuter le validateur du composant
cli-validator-invalid-report = le validateur du composant a renvoyé un rapport invalide
cli-validator-wrong-owner = le validateur du composant a renvoyé le mauvais propriétaire de schéma
cli-validator-inconsistent = le validateur du composant a renvoyé un résultat incohérent
cli-install-all-executables = installez tous les exécutables de Canopa à côté de l’exécutable canopa
cli-output-no-parent = la sortie doit avoir un dossier parent
cli-output-exists = la sortie existe déjà ou ne peut pas être créée
cli-show-source-copy = { $component } : { $path } (les diagnostics portent sur une copie temporaire immuable)

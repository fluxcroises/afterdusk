# Guide d'utilisation

Ce que l'interface ne dit pas, pour ne pas l'encombrer. À compléter au fil des versions ; il servira de base à l'aide en ligne.

## Import automatique

Après l'import d'une carte, Afterdusk propose d'importer seules les prochaines cartes du même boîtier (reconnu par son numéro de série). Ces cartes s'importent dès leur insertion, avec les réglages de leur premier import et un nom d'album par défaut, sauf si « À l'insertion d'une carte » est réglé sur « Ne rien faire ». La carte n'est que lue.

## Dossiers surveillés

Ajoutez le dossier où le logiciel de votre boîtier dépose les photos (connexion USB ou Wi-Fi). Chaque photo complète y est copiée et vérifiée dans l'album choisi, de la bibliothèque affichée, puis apparaît dans Trier en quelques secondes. Le dossier n'est jamais modifié ; ce qu'il contenait au moment de l'ajout est ignoré.

## Réception FTP

Pour les boîtiers qui envoient leurs photos en FTP par Wi-Fi ou Ethernet : elles arrivent dans Trier pendant la séance.

Sur le boîtier, réglez un serveur FTP avec l'adresse et le port affichés dans Réglages > Import, le mode passif et FTPS (TLS explicite).

- Si le boîtier demande de confirmer le certificat, comparez son empreinte à celle affichée par Afterdusk.
- S'il demande de l'importer, enregistrez le certificat sur une carte depuis Réglages > Import.
- Seul le réseau local peut se connecter. Windows peut demander d'autoriser Afterdusk sur les réseaux privés.
- Les fichiers reçus sont gardés jusqu'à ce que vous vidiez la réception ; seuls ceux déjà importés sont alors supprimés.

## Types de séance

Les types sont proposés pour les albums et à l'import. Un type renommé ou ajouté garde son nom dans toutes les langues de l'interface ; un type retiré laisse ses albums sans type.

## Clients

Une fiche client se rattache à un album depuis le panneau de droite (section « Client »).

## Galerie web

Le dossier créé s'ouvre aussi tel quel, d'un double-clic sur `index.html`. Quand le client renvoie sa sélection, Afterdusk retrouve la galerie grâce à son code.

## Modèles de livraison

Sans cadrage choisi, le cadre d'un modèle se centre sur le plus grand visage de la photo, sinon sur son milieu. Avec plusieurs photos sélectionnées, les autres que la photo active gardent leur cadrage, ou se centrent de la même façon.

## Logo du studio

Le logo (Réglages > Identité) figure en tête des planches contact et de l'inventaire du matériel, et sert de filigrane. Un PNG transparent rend le mieux.

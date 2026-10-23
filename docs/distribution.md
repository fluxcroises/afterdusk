# Distribution

Plan de la dernière étape de la feuille de route (cahier des charges, section 7) et démarches à faire par l'utilisateur. Choix fixés le 2026-10-23 : [décision 0011](decisions/0011-signature-des-installateurs.md) pour la signature, [décision 0013](decisions/0013-mises-a-jour.md) pour les mises à jour.

## Ordre imposé par SignPath

SignPath Foundation ne signe que des projets « déjà publiés sous la forme à signer ». La première version publique sort donc avec un installateur Windows non signé ; la signature arrive à la version suivante, une fois la candidature acceptée.

## Plan

1. **Mises à jour dans l'application** : extension `updater` de Tauri ; vérification au démarrage, au plus une fois par jour ; bandeau qui propose la version, installation sur clic, redémarrage ; case « Vérifier les mises à jour » dans Réglages > Général. Tests de la règle « une fois par jour » et de la case désactivée (aucune requête).
2. **Action de publication** (`.github/workflows/release.yml`), lancée par une étiquette `v0.5.0` :
   - Windows x64 : installateur NSIS, pour l'utilisateur, sans droits d'administrateur ;
   - macOS universel : `.dmg` et archive de mise à jour, signature ad hoc comme `build-macos.yml` ;
   - FFmpeg, ONNX Runtime et échantillons récupérés par les scripts qui vérifient leurs empreintes ;
   - archives de mise à jour signées avec la clé Tauri des secrets GitHub, `latest.json` produit ;
   - version GitHub créée en brouillon : rien n'est public avant ta relecture.
3. **Page du projet** (`README.md`, puis le site) : téléchargement, installation sur Mac (« Ouvrir quand même »), section « Politique de signature du code » et déclaration de confidentialité (voir plus bas).
4. **Version 0.5.0** : installateurs essayés sur une machine Windows et un Mac sans outils de développement, puis publication.
5. **Candidature SignPath** (toi), puis, une fois acceptée : étape de signature dans l'action, approbation manuelle de chaque signature, signature Tauri calculée après celle de SignPath (sinon la mise à jour serait refusée).
6. Critère « Installateurs signés pour Windows et macOS, mises à jour automatiques » coché après une mise à jour réelle 0.5.0 → 0.5.1 sur les deux systèmes.

## À faire de ton côté

### A. Clé de signature des mises à jour (avant l'étape 2)

Chaque mise à jour est signée par cette clé ; une version installée refuse tout fichier qui n'est pas signé par elle. **Perdue, elle empêche toute mise à jour des versions installées** : il faudrait réinstaller à la main.

1. Dans un terminal, à la racine du projet, crée la clé avec un mot de passe :

   ```bash
   npx tauri signer generate -w "$HOME/.tauri/afterdusk.key"
   ```

2. Range deux copies de `afterdusk.key` et du mot de passe ailleurs que sur ce PC (gestionnaire de mots de passe et clé USB rangée).
3. Ajoute-les aux secrets du dépôt (GitHub > Settings > Secrets and variables > Actions), ou avec `gh` :

   ```bash
   gh secret set TAURI_SIGNING_PRIVATE_KEY < "$HOME/.tauri/afterdusk.key"
   ```

   ```bash
   gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD
   ```

4. Donne-moi le contenu de `afterdusk.key.pub` : la clé publique n'est pas secrète, elle va dans `tauri.conf.json`. Ne me donne jamais `afterdusk.key` ni le mot de passe.

### B. Double authentification sur GitHub (avant la candidature)

Exigée par SignPath pour toute l'équipe : GitHub > Settings > Password and authentication > Two-factor authentication.

### C. Candidature SignPath Foundation (après la version 0.5.0)

Formulaire « Apply » sur https://signpath.org. Ce qu'il faut pouvoir montrer :

- licence GPL v3 pour tout le code, sans double licence commerciale ; les composants livrés (FFmpeg en LGPL, ONNX Runtime en MIT, LibRaw en LGPL) sont libres ;
- projet maintenu et déjà publié, fonctions décrites sur la page de téléchargement ;
- construction des installateurs entièrement par GitHub Actions, depuis le dépôt public ;
- page « Politique de signature du code » avec les rôles : auteurs (qui peut publier du code), relecteurs, approbateurs (qui valide chaque signature). Seul sur le projet, tu tiens les trois rôles ;
- déclaration de confidentialité.

SignPath reste libre d'accepter ou non. En cas de refus, la décision 0011 prévoit Certum Open Source (à partir de 49 € par an), sans changement de code.

## Déclaration de confidentialité (projet)

À publier avec la politique de signature :

> Afterdusk ne collecte aucune donnée et n'envoie aucune photo. Il fait trois sortes de requêtes, chacune désactivable dans les Réglages : les tuiles du fond de carte (OpenFreeMap), quand la carte est affichée ; la vérification des mises à jour (GitHub), au plus une fois par jour ; la réception FTP, sur le réseau local seulement, désactivée par défaut.

# 0013. Mises à jour et installateurs

Date : 2026-10-23 · Statut : **adoptée**, au choix de l'utilisateur

## Contexte

Une version installée garde pour toujours l'adresse où elle cherche ses mises à jour, son identifiant et son format d'installateur : ces choix sont à fixer avant la première publication. Plan complet dans `docs/distribution.md`.

## Décision

1. **Source des mises à jour : les versions GitHub** du dépôt, `https://github.com/lewisbedar/afterdusk-app/releases/latest/download/latest.json`, produit par l'action de publication. Rien à héberger. Le sous-domaine `afterdusk.flux-croises.fr` pourra être ajouté comme première adresse dans une version ultérieure ; les versions déjà installées continueront d'interroger GitHub.
2. **Vérification activée par défaut**, au démarrage et au plus une fois par jour, désactivable dans Réglages > Général (exception réseau de CLAUDE.md). La requête ne porte que sur `latest.json` ; une nouvelle version est proposée dans un bandeau et ne s'installe que sur clic.
3. **Windows : installateur NSIS seul**, installé pour l'utilisateur, sans droits d'administrateur. Pas de MSI : un seul fichier à faire signer, et passer d'un format à l'autre se passe mal pour les mises à jour.
4. **Identifiant inchangé : `app.afterdusk`**. Un identifiant est obligatoire (dossier des données, identité de l'application pour macOS et Windows) ; l'utilisateur a préféré le garder plutôt que `fr.flux-croises.afterdusk`, pour ne rien migrer. Le nom seul, `afterdusk`, a été écarté : il pourrait être partagé par un autre logiciel.
5. **Clé de signature des mises à jour** : créée et gardée par l'utilisateur, dans les secrets GitHub et en deux copies hors du PC. Sa perte empêcherait toute mise à jour des versions installées.

## Conséquences

- Première version publique : 0.5.0, installateur Windows non signé (SignPath exige un projet déjà publié), signature à partir de la version suivante.
- Changer plus tard de dépôt GitHub sans redirection couperait les mises à jour des versions qui n'ont pas encore l'adresse du sous-domaine.

# 0011. Signature des installateurs

Date : 2026-10-22 · Statut : **adoptée** le 2026-10-22, au choix de l'utilisateur

## Contexte

La V0.4 prévoit des « installateurs signés pour Windows et macOS, mises à jour automatiques », et la section 9 laissait ouvert le choix entre un certificat Apple (99 $ par an), un certificat Windows ou une distribution non signée au début.

Sans signature :

- **Windows** : SmartScreen affiche « Windows a protégé votre ordinateur » ; il faut cliquer sur « Informations complémentaires » puis « Exécuter quand même ».
- **macOS** : depuis macOS 15.1, le contournement par clic droit a disparu ; il faut tenter d'ouvrir l'application, puis passer par Réglages Système > Confidentialité et sécurité > « Ouvrir quand même ». Un photographe débutant risque d'abandonner.

## Options étudiées (conditions relevées le 2026-10-22)

| Système | Option                          | Coût                       | Éditeur affiché                  | Remarque                                                                                                                                                                   |
| ------- | ------------------------------- | -------------------------- | -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Windows | Non signé                       | 0                          | Éditeur inconnu                  | Avertissement à chaque installation                                                                                                                                        |
| Windows | **SignPath Foundation**         | Gratuit                    | SignPath Foundation              | Projet open source (licence OSI, sans double licence commerciale), construit automatiquement depuis le dépôt, politique de signature publiée ; clé dans un module matériel |
| Windows | Certum Open Source Code Signing | à partir de 49 € par an    | « Open Source Developer, » + nom | Développeur individuel, logiciel non commercial ; signature dans le nuage (SimplySign)                                                                                     |
| Windows | Certificat classique (OV)       | environ 200 à 400 € par an | Nom de l'auteur ou de la société | Clé sur support matériel obligatoire depuis 2023                                                                                                                           |
| Windows | Azure Artifact Signing          | 9,99 $ par mois            | Nom de l'auteur                  | Particuliers des États-Unis et du Canada seulement ; sociétés de l'UE d'au moins 3 ans                                                                                     |
| Windows | OSSign                          | Gratuit annoncé            | Non documenté                    | Service récent, n'acceptant pas de demandes au moment de l'étude, peu documenté                                                                                            |
| macOS   | Non signé                       | 0                          | —                                | Passage par « Ouvrir quand même »                                                                                                                                          |
| macOS   | **Apple Developer Program**     | 99 $ par an                | Nom de l'auteur                  | Developer ID et notarisation : seule voie sans avertissement ; aucun service tiers ne la remplace                                                                          |

## Décision

1. **Windows : SignPath Foundation.** Gratuit, clé en module matériel, signature intégrée à GitHub Actions. L'éditeur affiché est « SignPath Foundation » et non le nom de l'auteur, ce que l'utilisateur accepte. Il faut publier une politique de signature sur la page du projet, et garder une construction entièrement automatisée depuis le dépôt.
2. **macOS : non signé au début**, avec une page d'aide illustrée pour « Ouvrir quand même ». **Apple Developer Program plus tard** (99 $ par an), quand des utilisateurs Mac non techniciens arriveront ; la notarisation sera alors ajoutée à la CI.
3. **Mises à jour automatiques** : clé de signature Tauri propre au projet, gratuite et indépendante des certificats ci-dessus, gardée dans les secrets GitHub. La vérification des mises à jour fait partie des exceptions réseau de CLAUDE.md, désactivable dans les Réglages.

## Conséquences

- Candidature à SignPath Foundation à préparer : licence GPL v3 (OSI), dépôt public, politique de signature, construction des installateurs par GitHub Actions.
- Le critère de la V0.4 « Installateurs signés pour Windows et macOS » sera coché avec un installateur macOS non signé mais documenté : la signature Apple reste un point ouvert, daté par l'utilisateur.
- OSSign est à réévaluer quand son service sera ouvert et documenté ; changer de service de signature ne touche pas au code.

## Sources

- Microsoft, options de signature du code : https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options
- SignPath Foundation, conditions : https://signpath.org/terms
- Certum, Open Source Code Signing dans le nuage : https://shop.certum.eu/open-source-code-signing-on-simplysign.html
- Azure Artifact Signing, tarifs : https://www.itechguides.com/best/code-signing-software/azure-artifact-signing/
- OSSign : https://ossign.org/ et https://github.com/OSSign/OSSign
- macOS Sequoia, applications bloquées : https://www.macworld.com/article/2457844/what-to-do-when-you-cant-open-an-app-you-just-installed-in-macos-sequoia.html

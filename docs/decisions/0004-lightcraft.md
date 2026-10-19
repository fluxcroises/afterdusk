# 0004 — Ce que l'on peut reprendre de LightCraft

Date : 2026-10-19 · Statut : **adoptée** (réponses de l'utilisateur du 2026-10-19, en fin de document)

## Contexte

[LightCraft](https://github.com/storytold/lightcraft) est un logiciel libre de gestion et de développement photo en Rust pur, proche d'Afterdusk par son objet : bibliothèque, tri, retouche non destructive, XMP, carte. L'utilisateur l'a signalé comme une aide possible. Ce document fait l'inventaire de ce qui est réutilisable, crate par crate et version par version (cahier des charges §7), avant toute décision.

État étudié : commit `d999e38` du 2026-10-18, version 0.9.1.

- **Projet très jeune** : dépôt créé le 2026-09-30, il a trois semaines. Il se dit lui-même « young & moving fast », au stade alpha.
- **Projet très gros** : environ 200 000 lignes de Rust en 29 crates. Les plus grosses sont `engine` (87 000 lignes), `ui-egui` (78 000) et `raw` (36 000).
- **Aucune crate publiée sur crates.io** (`lightcraft-raw`, `-develop`, `-pipeline`, `-geocode`, `-lensdb` : « does not exist »). Une dépendance passerait donc par git, épinglée sur un commit.
- **Interface egui**, alors qu'Afterdusk est en Tauri + React. Le reste est séparé en couches L0 à L5, et une règle vérifiée en CI interdit aux couches basses de dépendre de l'interface.

Le contenu du dépôt a été lu comme une donnée. Aucun code n'a été copié dans Afterdusk ; la mesure ci-dessous a été faite dans un projet jetable, hors du dépôt.

## Licences

| Partie                                         | Licence                                                                       | Compatible GPL v3                  | Obligations si on reprend                                                                                                                    |
| ---------------------------------------------- | ----------------------------------------------------------------------------- | ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Presque toutes les crates                      | MIT ou Apache 2.0, au choix                                                   | Oui                                | Sous MIT : garder la mention « Copyright (c) 2026 ArtCraft Team and the LightCraft contributors » et le texte MIT dans les fichiers repris   |
| `crates/segment` (SAM 3), `crates/fetch`       | Apache 2.0 seule                                                              | Oui (GPL v3 seulement, pas v2)     | Texte Apache, reprise du `NOTICE`, mention des modifications dans chaque fichier modifié. Sans objet : on ne les reprend pas (voir plus bas) |
| Matrices couleur `raw/src/spectral_table.rs`   | Dérivées de rawtoaces-data (Apache 2.0) et des fonctions CIE 1931 (CC-BY 4.0) | Oui                                | Attribution d'ASWF / rawtoaces-data et de la CIE, en plus de LightCraft                                                                      |
| Poids SAM 3                                    | Licence SAM de Meta, non libre                                                | **Non**                            | Jamais distribués par LightCraft, et à ne pas distribuer non plus                                                                            |
| Modèle de débruitage RawNIND (option)          | GPL 3                                                                         | Oui                                | Sans objet : non retenu                                                                                                                      |
| Nom, logo et marques ArtCraft (`docs/brand/`)  | Marques, non libres                                                           | **Non**                            | Ne rien reprendre. Le nom LightCraft n'apparaît que dans les mentions d'attribution                                                          |
| Police Inter, photos d'exemple                 | OFL 1.1, domaine public                                                       | Oui                                | Sans objet                                                                                                                                   |
| Noms de lieux GeoNames (téléchargés par l'app) | CC-BY 4.0                                                                     | Oui (données)                      | Mention « GeoNames, geonames.org » dans l'application                                                                                        |
| Base lensfun (fournie par l'utilisateur)       | CC-BY-SA 3.0                                                                  | Oui, en fichiers de données à part | Fichiers distribués tels quels, avec leur licence et leur attribution                                                                        |

Le sens de compatibilité est le bon : du code MIT ou Apache 2.0 peut entrer dans un programme GPL v3, et l'ensemble reste sous GPL v3. Il faut seulement conserver les mentions. Le plus simple est de choisir l'option MIT pour tout ce qui la propose, et de tenir un fichier `THIRD-PARTY.md` à la racine plus un écran « À propos ».

## Version par version

### V0.1 — Importer et trier : on garde LibRaw

`lightcraft-raw` est un décodeur RAW en Rust pur. Il couvre DNG, CR2/CR3, NEF, ARW, RAF Bayer et X-Trans, RW2, PEF et ORF non compressé, avec dématriçage AHD, PPG, bilinéaire et X-Trans. Il annonce ne contenir aucun code de LibRaw, RawSpeed ou darktable : les formats ont été retrouvés à partir de documents publics et d'échantillons CC0.

**Mesure sur nos six échantillons**, avec le même chemin et les mêmes fichiers que la décision 0001. Projet jetable dépendant de `lightcraft-raw` par chemin, Windows 11, release, une passe, fichier en cache disque :

| Fichier            | En-tête   | Aperçu intégré (comme LibRaw) | Décodage complet                                                                     | Métadonnées                                                                   | Dématriçage AHD |
| ------------------ | --------- | ----------------------------- | ------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------- | --------------- |
| RAF X-T3 (X-Trans) | 2 ms      | 4416 × 2944, < 1 ms           | 8 ms                                                                                 | marque, modèle, objectif, ISO, vitesse, ouverture, focale, date               | 260 ms          |
| CR3 EOS R6         | < 1 ms    | 5472 × 3648, < 1 ms           | 126 ms                                                                               | complètes                                                                     | 243 ms          |
| NEF Z 6            | **échec** | 6048 × 4024, < 1 ms           | **échec** : « Nikon compressed NEF without a linearization table (maker note 0x96) » | —                                                                             | —               |
| ARW A7 III         | < 1 ms    | 1616 × 1080, < 1 ms           | 8 ms                                                                                 | complètes, plus correction d'objectif du boîtier                              | 300 ms          |
| RW2 G9 (80 Mpx)    | < 1 ms    | 1920 × 1440, < 1 ms           | 33 ms                                                                                | pas de nom d'objectif                                                         | 1 434 ms        |
| DNG GR III         | < 1 ms    | 6000 × 4000, < 1 ms           | 335 ms                                                                               | complètes, pas de balance des blancs boîtier (DNG : elle passe par le profil) | 268 ms          |

Les sommes SHA-256 des six originaux sont identiques avant et après la mesure.

Constats :

- **Aperçus intégrés** : exactement les mêmes que ceux retenus par LibRaw. Pour ce qu'utilise la V0.1, LightCraft n'apporte rien de plus.
- **Le NEF du Nikon Z 6 ne se décode pas.** C'est l'un des six formats exigés par la V0.1, et LibRaw le décode.
- **Couleur** : en dehors des DNG, LightCraft a des matrices pour 52 boîtiers (dont R6, X-T3 et A7 III, mais pas le G9 ni le Z 6) et sinon un modèle neutre. LibRaw embarque des matrices pour presque tous les boîtiers. Or la V0.2 exige un « rendu neutre fidèle au boîtier ».
- **Atouts** : Rust pur, aucun `unsafe` (`forbid(unsafe_code)`), aucune compilation C++, et des décodeurs écrits pour ne jamais planter sur un fichier abîmé, ce que vérifient leurs tests.

**Recommandation V0.1 : aucun changement.** LibRaw reste le décodeur (décision 0001). Il est validé, il couvre nos six formats, et sa liaison est déjà écrite et testée. On remesurera `lightcraft-raw` au début de la V0.2, quand le décodage complet servira vraiment.

### V0.2 — Retoucher et livrer : s'en inspirer, reprendre de petits modules, ne pas en dépendre

Les crates concernées :

- **`develop`** (2 800 lignes) : le modèle de réglages. Une structure serde où chaque champ a une valeur neutre par défaut (`#[serde(default)]`), avec un champ `version` (le schéma) **et** un champ `process` (le moteur de rendu, comme la « version de processus » de Lightroom). Une photo garde son rendu tant qu'on ne la passe pas au processus courant. S'y ajoutent des descriptions de curseurs (identifiant, bornes, valeur par défaut) et les presets.
- **`pipeline`** (12 000 lignes) : la référence sur processeur, en flottant, en espace linéaire large gamut (Rec.2020), indépendante de la résolution. Elle couvre tous nos réglages de la §4.5 et bien plus : masques, retouche de défauts, Upright. Les étapes sont mises en cache et une pyramide d'images source rend les curseurs fluides.
- **`gpu`** (8 700 lignes, WGSL sur wgpu) : le jumeau GPU de `pipeline`. Ses tests vérifient qu'il reste à 1/255 de la référence processeur, et il retombe sur le processeur si le GPU échoue. C'est exactement l'architecture que prévoit le cahier des charges (wgpu, flottant, linéaire large gamut).
- **`color`** (900 lignes) : fonctions de transfert, température de couleur, splines, espaces perceptuels. Elle est petite, autonome et ne dépend que de `geom`.

Pourquoi **ne pas en dépendre** directement :

- **Pas sur crates.io.** Une dépendance git épinglée, sans semver, sur un projet qui change chaque jour.
- **Dépendances en chaîne** : `pipeline` dépend de `raw`, de `inpaint` et de `develop` ; `gpu` dépend de `faces`. Prendre le pipeline, c'est prendre le décodeur RAW et une partie de l'IA.
- **Modèle de réglages imposé** : leur `DevelopSettings` deviendrait notre format, et le format des réglages est une décision difficile à défaire (CLAUDE.md).
- **Projet de trois semaines** en alpha : l'API n'est pas stable.

Ce qu'il est raisonnable de faire :

1. **Écrire notre pipeline** (`src-tauri/src/develop/`) avec la même architecture : référence processeur plus jumeau WGSL, tolérance vérifiée par des tests, retour au processeur en cas d'échec GPU.
2. **Reprendre avec attribution** quelques fichiers bien délimités, s'ils évitent un vrai travail : `color` en entier, et éventuellement des étapes isolées du pipeline ou des shaders WGSL. Chaque fichier repris garde la mention MIT et dit qu'il a été modifié ; la liste va dans `THIRD-PARTY.md`.
3. **Ajouter à notre JSON de réglages un champ « processus »** distinct de `version` : `version` décrit la forme du JSON et appelle une migration ; le processus fige le rendu. C'est l'idée la plus utile de `develop`, et elle complète la règle « réglages en JSON versionné ».
4. **Lire `docs/xmp-interop.md`** de LightCraft (correspondance avec l'espace `crs:` d'Adobe) au moment d'écrire les retouches dans le XMP (décision 0002).

**Corrections d'objectifs.** Le cahier des charges les place en V0.2 (« via lensfun »). **`lensdb`** (1 000 lignes, Rust pur) lit les fichiers XML de la base lensfun sans la bibliothèque C lensfun (LGPL, qui dépend de GLib : la même difficulté sous Windows qui a écarté Exiv2 dans la décision 0002). Il convertit le profil en polynômes DNG (`WarpRectilinear`, `FixVignetteRadial`), appliqués en un seul rééchantillonnage. LightCraft ne fournit pas la base, qui est en CC-BY-SA 3.0. `lightcraft-raw` lit aussi les corrections enregistrées par certains boîtiers (Sony, Nikon, Fujifilm), qui servent quand lensfun ne connaît pas l'objectif. **Recommandation** : reprendre `lensdb` avec attribution, plutôt que de lier lensfun en C, et livrer les fichiers XML de lensfun avec l'installateur (voir les questions).

**Recommandation V0.2** : référence d'architecture, reprise ciblée et attribuée (`color`, `lensdb`, puis éventuellement des étapes ou shaders isolés), aucune dépendance directe.

### V0.3 — Carte et matériel : reprendre `geocode`, embarquer les données

**`geocode`** (430 lignes, Rust pur, sans dépendance hors `miniz_oxide` et `serde`) fait le géocodage inverse hors ligne à partir des fichiers GeoNames `cities15000`, `admin1CodesASCII` et `countryInfo`. Il indexe les lieux par cellules, cherche le plus proche par distance haversine et ne répond rien au-delà de 50 km. C'est exactement le critère « Géocodage inverse hors ligne (pays, ville) ».

Écart avec notre règle : LightCraft **télécharge** les fichiers GeoNames à la demande de l'utilisateur. Chez nous, ce serait un appel réseau interdit. Il faut donc **embarquer les fichiers dans l'installateur** (quelques Mo compressés, CC-BY 4.0, mention « GeoNames, geonames.org » dans « À propos »).

**Recommandation V0.3** : reprendre `geocode` avec attribution, sans la partie téléchargement ; données GeoNames livrées avec l'application.

### V0.4 et au-delà

- **`merge`** (HDR, panoramas) et **`inpaint`** (suppression de défauts) visent l'« après V0.4 ». À regarder à ce moment-là.
- **Fonctions d'IA** (`faces`, `segment`, `denoise`) : à écarter. Elles reposent sur des poids téléchargés (dont SAM 3, non libre), et `denoise` sait envoyer des tuiles à un autre ordinateur par TCP. Notre détection locale des ratés (flou, yeux fermés) de la V0.4 restera légère.

## Idées d'architecture à reprendre (sans code)

- **Écritures sûres** (`catalog/src/safe_file.rs`). Deux politiques :
  - écriture **durable** (synchronisée sur le disque) pour ce qui ne se recrée pas : XMP, catalogue, réglages ;
  - écriture **atomique** seule pour ce qui se recrée : exports, aperçus.

  `write_new` écrit un fichier qui ne doit pas exister, par fichier temporaire, relecture puis lien dur, et ne remplace jamais rien. C'est exactement ce qu'il faut pour la copie d'import vers la bibliothèque. Une fonction `same_file` répond à « cette cible est-elle un original ? ».

- **Journal du catalogue** (`journal.rs`) : journal en ajout seul, avec une somme CRC par ligne, des instantanés compactés en arrière-plan et la récupération d'une écriture coupée. Nous utilisons SQLite, qui garantit déjà l'atomicité des transactions. Ce qu'il faut reprendre, ce sont leurs **tests de coupure** (`tests_torn_append.rs`), à transposer à notre reprise d'import après coupure.
- **Piles** (`stacks.rs`) : une pile est une donnée annulable du catalogue, avec un dessus et un état replié. Cela rejoint la décision 0003 et nos rafales de la V0.4.
- **Versions de processus** : voir la V0.2.
- **Tests** :
  - `proptest` dans une vingtaine de fichiers, sur le modèle de réglages et la couleur ;
  - décodeurs qui ne doivent jamais paniquer, vérifiés par des fichiers tronqués ;
  - lints Clippy `unwrap_used`, `expect_used` et `panic` interdits dans le code de production, ce qui rejoint notre règle « jamais de unwrap » et pourrait aller dans `clippy.toml` ;
  - une CI qui dit **quels tests ont été sautés et pourquoi** (`testkit`), utile pour « tester sur Windows et macOS avant de cocher ».
- **Couches vérifiées** (`cargo xtask layers`) : l'interface ne peut pas fuir dans le cœur. Une règle semblable entre `src-tauri/src/commands` et le reste du cœur est à envisager.

## Ce qui ne convient pas

- **`ui-egui`** et **`engine`** (165 000 lignes à eux deux) : l'interface et la session de LightCraft, liées à egui. Nous sommes en Tauri + React.
- **`fetch`** : téléchargements HTTPS (rustls) des poids d'IA et de GeoNames, à la demande de l'utilisateur. C'est contraire à notre règle « aucun appel réseau hors tuiles et mises à jour ». À ne pas reprendre, même si son principe (fichier `.part`, empreinte SHA-256 vérifiée avant renommage) pourra inspirer la vérification des mises à jour en V0.4.
- **`mcp`** et le canal de contrôle (`apps/lightcraft/src/control_server.rs`) : pilotage par des agents IA, hors de notre périmètre.
- **`denoise`** (inférence à distance par TCP non chiffré), **`segment`**, **`faces`** : voir la V0.4.
- **Marques ArtCraft** : rien à reprendre.

## Recommandation

| Version | Recommandation                                                                                                                                                                                                                                                               |
| ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| V0.1    | **Rien à changer.** LibRaw reste le décodeur ; `lightcraft-raw` échoue sur le NEF du Z 6 et n'apporte rien pour les aperçus.                                                                                                                                                 |
| V0.2    | **Inspiration et reprise ciblée, sans dépendance** : notre pipeline sur le même modèle (référence processeur, jumeau WGSL, tolérance testée) ; `color` et `lensdb` repris avec attribution ; champ « processus » dans nos réglages. Remesurer `lightcraft-raw` au démarrage. |
| V0.3    | **Reprendre `geocode`** avec attribution, données GeoNames embarquées, sans téléchargement.                                                                                                                                                                                  |
| Toutes  | Idées d'architecture (écritures sûres, tests de coupure, lints, rapport des tests sautés) à reprendre sans code ; un `THIRD-PARTY.md` dès le premier fichier repris.                                                                                                         |

## Questions à trancher

1. **Accepter du code repris d'un autre projet ?** Afterdusk est aujourd'hui entièrement écrit pour lui. La reprise de fichiers MIT est légale et compatible, mais elle apporte du code que nous n'avons pas écrit et une attribution à maintenir. Oui (recommandé, limité à `color`, `lensdb` et `geocode`), ou seulement de l'inspiration ?
2. **Champ « processus » dans le JSON de réglages** (distinct de `version`) : l'adopter dès la première version du format en V0.2 ? Recommandé : c'est un changement de format, donc difficile à faire plus tard.
3. **Données embarquées dans l'installateur** : GeoNames (CC-BY 4.0) et base lensfun (CC-BY-SA 3.0), soit quelques Mo de plus, avec leurs mentions dans « À propos ». D'accord pour les livrer, plutôt que de les faire télécharger (interdit par nos règles) ou de demander à l'utilisateur de les fournir ?
4. **Corrections d'objectifs : `lensdb` en Rust plutôt que la bibliothèque lensfun en C.** Le cahier des charges dit « via lensfun » : faut-il comprendre « la base lensfun » (recommandé, sans GLib) ou « la bibliothèque lensfun » ?
5. **Décodage complet en V0.2** : rester sur LibRaw seul (recommandé : un seul décodeur, toutes nos marques), ou passer à `lightcraft-raw` si une nouvelle mesure en début de V0.2 montre le NEF du Z 6 et une couleur fidèle pour nos six boîtiers ?
6. **Où placer les attributions** : `THIRD-PARTY.md` à la racine et un écran « À propos » listant LightCraft, GeoNames et lensfun ; ou aussi un en-tête dans chaque fichier repris (requis par la licence MIT, donc recommandé en plus) ?

## Décision

Réponses de l'utilisateur, le 2026-10-19 :

1. **Code repris de LightCraft** : oui, limité à `color`, `lensdb` et `geocode`, avec attribution. Le code repris est un point de départ : on l'améliore ou on le réécrit dès que nos besoins l'exigent, sans s'y sentir tenu.
2. **Champ « processus »** dans le JSON des réglages, distinct de `version`, dès la première version du format (V0.2).
3. **Données embarquées** : GeoNames (V0.3) et la base lensfun, livrées avec l'application et créditées dans « À propos » ; jamais téléchargées.
4. **Corrections d'objectifs** : la base lensfun, lue en Rust (`lensdb`), plutôt que la bibliothèque lensfun en C. « Via lensfun » dans le cahier des charges désigne la base.
5. **Décodage complet** : LibRaw seul ; `lightcraft-raw` n'est remesuré que si un besoin apparaît.
6. **Attributions** : `THIRD-PARTY.md` à la racine, la boîte « À propos », et un en-tête dans chaque fichier repris (exigé par la licence MIT).

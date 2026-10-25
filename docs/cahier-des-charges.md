# Afterdusk : cahier des charges

Oct 18, 2026 · @Lewis

## 1. Présentation du projet

Afterdusk est un logiciel de bureau gratuit et open source qui couvre tout le parcours d'une journée de photos : importer, trier, retoucher, livrer. Il vise les photographes semi-pro qui trouvent Lightroom et Capture One trop lourds.

**Public cible** : photographes amateurs avancés et semi-pro (sorties, voyages, événements, petites commandes clients), à l'aise avec leur boîtier mais pas forcément avec un logiciel de post-traitement.

**Principes directeurs**

- **Un flux guidé en quatre étapes visibles** : Importer, Trier, Retoucher, Livrer. L'utilisateur sait toujours où il en est.
- **De vrais dossiers, pas de catalogue opaque** : les fichiers restent lisibles dans l'explorateur, les réglages sont écrits dans des fichiers XMP compatibles avec les autres logiciels.
- **Non destructif** : un original n'est jamais modifié.
- **Rapide avant tout** : affichage instantané grâce aux aperçus JPEG intégrés aux RAW, traitements lourds en arrière-plan.
- **Simple par défaut, puissant sur demande** : quelques réglages essentiels visibles, les options avancées repliées.
- **Local et privé** : aucun compte, aucun cloud, toutes les analyses tournent sur la machine.
- **Universel** : tous les boîtiers du marché, photos et vidéos.

**Fiche d'identité**

| Élément | Choix |
| --- | --- |
| Plateformes | Windows 10/11 et macOS (Apple Silicon et Intel) |
| Licence | Open source (GPL v3 recommandée, voir section 3) |
| Langues | Français et anglais |
| Médias | Photos (RAW de tous boîtiers, JPEG, HEIF, TIFF, PNG) et vidéos (import et tri) |
| Prix | Gratuit |

## 2. Parcours utilisateur type

Le scénario de référence est une journée de photos ramenée le soir : chaque fonctionnalité doit servir ce parcours.

1. **Branchement** : l'utilisateur insère sa carte SD ou branche son boîtier. Afterdusk détecte le support et ouvre la fenêtre d'import.
2. **Import** : il voit les nouvelles photos et vidéos (celles déjà importées sont grisées), choisit ou crée un album (nom, date, lieu), et lance l'import. Les fichiers sont copiés, renommés et rangés automatiquement, avec une copie de sauvegarde sur un second support si elle est configurée.
3. **Localisation** : si les photos n'ont pas de GPS, Afterdusk propose une trace GPX, une photo de smartphone du même jour, ou un placement manuel de l'album sur la carte.
4. **Tri** : il parcourt la journée au clavier, note, marque comme retenue ou rejetée, compare les photos proches. Les rafales sont empilées et les photos ratées (floues, yeux fermés, mal exposées) sont signalées. Les rejets partent dans la corbeille interne.
5. **Retouche** : il ajuste les photos retenues, applique un preset à une série, synchronise les réglages entre photos similaires.
6. **Livraison** : il exporte avec un preset (originaux, web, Instagram, impression), ajoute un filigrane, retire les données GPS, ou génère une galerie web pour un client.
7. **Après coup** : l'album apparaît sur la carte du monde, et les statistiques du matériel sont mises à jour (boîtier et objectifs utilisés ce jour-là).

## 3. Architecture technique

Afterdusk est une application **Tauri 2** : un cœur en **Rust** qui fait tout le travail lourd (décodage, traitement d'image, base de données, fichiers) et une interface web en **TypeScript** affichée dans la vue web native du système. L'interface ne touche jamais directement aux fichiers : elle passe par des commandes Tauri.

&#91;embedded content: architecture d'Afterdusk · interface, Tauri, cœur Rust, stockage\]

L'interface dialogue uniquement avec le cœur Rust via Tauri ; seul le cœur lit et écrit les fichiers, la base et le cache.

**Briques retenues**

| Besoin | Brique | Remarque |
| --- | --- | --- |
| Application de bureau | Tauri 2 | Léger, Windows et macOS, mises à jour intégrées |
| Interface | React + TypeScript + Vite | État global avec Zustand, listes virtualisées pour les grilles de milliers de photos |
| Décodage RAW | LibRaw (liaison Rust) | Couvre les RAW de tous les boîtiers, y compris Fujifilm X-Trans ; licence LGPL compatible |
| Aperçus rapides | Extraction du JPEG intégré au RAW | Affichage immédiat pendant le tri, sans décodage complet |
| Traitement d'image | Pipeline Rust sur GPU (wgpu) | Calcul en virgule flottante, espace linéaire large gamut, sortie sRGB ou Display P3 |
| Corrections d'objectifs | Base lensfun | Distorsion, vignetage, aberrations chromatiques |
| Métadonnées | Exiv2 ou xmp-toolkit-rs | Lecture EXIF/IPTC/XMP, écriture des fichiers XMP à côté des originaux ; choix à valider en V0.1 |
| Vidéos | FFmpeg (ffprobe) | Métadonnées, miniatures, lecture |
| Base de données | SQLite (rusqlite) | Index local, migrations versionnées |
| Carte | MapLibre GL JS | Tuiles OpenStreetMap via un fournisseur gratuit (OpenFreeMap par exemple) |
| Noms de lieux hors ligne | Jeu de données GeoNames des villes | Géocodage inverse sans connexion |
| Traces GPS | Crate gpx | Synchronisation par horodatage |
| Détection des ratés | Analyse d'image en Rust + modèles ONNX locaux (crate ort) | Flou, exposition, yeux fermés ; rien n'est envoyé en ligne |
| Traductions | i18next | Fichiers JSON fr et en |

**Licence** : GPL v3. Elle autorise à reprendre du code de darktable ou RawTherapee (algorithmes de bruit, de couleur) et reste compatible avec Exiv2. Toutes les dépendances ajoutées devront être compatibles GPL v3.

**Stockage des données**

- **Les photos** restent dans la bibliothèque choisie par l'utilisateur (disque interne, externe ou NAS), rangées selon le modèle de nommage.
- **Les réglages de retouche, notes et étiquettes** sont écrits dans la base SQLite et dans un fichier XMP à côté de chaque original : la bibliothèque reste exploitable sans Afterdusk.
- **La base et le cache d'aperçus** vivent dans le dossier de données de l'application. Le cache garde trois tailles (256, 1024 et 2048 px) pour consulter un disque externe débranché.

**Structure du dépôt**

```
afterdusk/
├── CLAUDE.md              consignes pour Claude Code (section 8)
├── docs/                  ce cahier des charges, décisions d'architecture
├── src-tauri/             cœur Rust
│   ├── src/
│   │   ├── commands/      commandes exposées à l'interface
│   │   ├── import/        détection des supports, copie, renommage, sauvegarde
│   │   ├── library/       albums, médias, recherche
│   │   ├── raw/           décodage LibRaw, aperçus
│   │   ├── develop/       pipeline de retouche GPU
│   │   ├── export/        presets, filigrane, galerie web
│   │   ├── geo/           GPS, GPX, géocodage
│   │   ├── gear/          matériel, statistiques, entretien
│   │   ├── analysis/      flou, rafales, détection des ratés
│   │   ├── metadata/      EXIF, XMP
│   │   └── db/            schéma et migrations SQLite
│   └── tests/
├── src/                   interface React
│   ├── steps/             Importer, Trier, Retoucher, Livrer
│   ├── views/             Bibliothèque, Carte, Matériel, Réglages
│   ├── components/
│   └── i18n/              fr.json, en.json
└── test-assets/           échantillons RAW libres de droits (raw.pixls.us)
```

## 4. Spécifications fonctionnelles

Chaque module indique entre crochets la version qui le livre (voir section 7).

### 4.1 Import \[V0.1\]

- **Détection automatique** des cartes SD, boîtiers branchés en mode stockage de masse et dossiers DCIM. Une notification propose d'ouvrir l'import (désactivable).
- **Import depuis un dossier** quelconque, en plus des supports amovibles.
- **Aperçu avant import** : grille des médias du support, groupés par jour, avec les fichiers déjà importés grisés (détection par empreinte : nom, taille, date de prise de vue et hash partiel).
- **Sélection** : tout, rien, par jour, ou photo par photo.
- **Album de destination** : existant ou nouveau (nom, date, lieu facultatif). Un import de plusieurs jours peut créer un album par jour.
- **Rangement automatique** selon un modèle configurable, par défaut `Bibliothèque/AAAA/AAAA-MM-JJ Nom de l'album/`.
- **Renommage** optionnel selon un modèle (par exemple `AAAAMMJJ_HHMMSS_Boîtier_0001.RAF`), le nom d'origine étant conservé en métadonnée.
- **Sauvegarde simultanée** sur un second emplacement (disque externe ou NAS) si configurée.
- **Vérification** : chaque copie est contrôlée par somme de contrôle avant d'être marquée comme importée.
- **Après import** : proposition d'éjecter la carte. Afterdusk ne supprime jamais rien sur la carte.
- **Mots-clés et copyright** appliqués à l'import (auteur et mention de copyright par défaut dans les réglages).
- **Traitement en arrière-plan** : l'utilisateur peut commencer à trier pendant que l'import, les aperçus et les analyses continuent, avec une barre de progression.
- **Client et type de séance** \[V0.4\] : choisis dans le panneau de destination (voir 4.2), utilisables dans les modèles de rangement et de renommage (`{client}`, `{seance}`).

### 4.1 bis Import automatique \[V0.4\]

Demandé par l'utilisateur le 2026-10-21 (décision 0009). Les photos arrivent dans la bibliothèque sans passer par l'écran d'import, avec les mêmes garanties : copie vérifiée par somme de contrôle, sauvegarde si elle est configurée, rien d'écrit sur la source.

- **Carte connue** : pour une carte déjà importée une fois, l'utilisateur peut choisir « importer automatiquement » avec les derniers réglages (album, rangement, renommage). Les photos déjà importées sont ignorées, et une notification donne le nombre de photos ajoutées.
- **Dossier surveillé** : un ou plusieurs dossiers où l'application du fabricant (connexion USB ou Wi-Fi) ou un transfert sans fil dépose les photos. Afterdusk attend qu'un fichier soit complet (taille stable), le copie dans l'album choisi et l'affiche aussitôt dans le tri. Le dossier surveillé n'est jamais modifié.
- **Réception FTP** : un serveur FTP limité au réseau local reçoit les photos envoyées par les boîtiers qui savent transférer en FTP. Désactivé par défaut, protégé par un identifiant et un mot de passe propres à Afterdusk ; chaque fichier reçu suit le même chemin qu'un fichier de dossier surveillé. FTP et FTPS sont acceptés, FTPS conseillé : en FTP simple, un avertissement rappelle que le mot de passe et les photos circulent en clair sur le réseau local (décision 0009).
- **Délai et ordre** : une photo arrivée s'affiche dans le tri en moins de 5 secondes. Ordre de réalisation : carte connue, dossier surveillé, réception FTP.
- **Pendant une séance** : l'album en cours reste ouvert dans Trier et les nouvelles photos s'y ajoutent à la fin, sans changer la photo affichée.

### 4.2 Bibliothèque et albums \[V0.1\]

- **Vue Bibliothèque** : liste des albums (miniature de couverture, nom, dates, lieu, nombre de médias), triable par date, nom ou lieu.
- **Filtres** : date ou période, lieu (pays, ville), album, boîtier, objectif, focale, note, drapeau, couleur, type de média, mots-clés.
- **Recherche** texte sur noms d'albums, mots-clés, lieux et noms de fichiers.
- **Collections** : regroupements virtuels de photos de plusieurs albums (par exemple « Meilleures photos 2026 »), sans déplacer les fichiers.
- **Albums intelligents** : collections enregistrées à partir d'un filtre (par exemple « 5 étoiles prises au 23 mm ») \[V0.3\].
- **Clients et séances** \[V0.4\] (décision 0009) : une fiche client (nom, courriel et téléphone facultatifs, notes) et un type de séance (mariage, baptême, naissance, portrait, événement, grossesse, famille, entreprise, produit, sport, concert ; liste modifiable) se rattachent à un album. La Bibliothèque filtre et cherche par client et par type de séance, et la fiche d'un client liste ses albums. La galerie client (4.6) reprend son nom et réimporte ses choix dans le bon album. Les données des clients restent dans la base locale, jamais dans les XMP, et la suppression d'un client efface toutes ses données. Devis, factures, agenda et contrats restent hors périmètre.
- **Fiche d'informations** d'une photo : EXIF lisibles (boîtier, objectif, focale, ouverture, vitesse, ISO, date), lieu, histogramme.
- **Plusieurs bibliothèques** possibles, et bibliothèque hors ligne consultable grâce aux aperçus en cache, avec un message clair invitant à rebrancher le disque pour retoucher ou exporter.
- **Synchronisation avec le disque** : bouton « Rechercher les changements » qui détecte les fichiers ajoutés, déplacés ou supprimés hors d'Afterdusk.

### 4.3 Tri \[V0.1, analyses en V0.4\]

- **Visionneuse plein écran rapide** : passage d'une photo à l'autre sans attente (aperçu JPEG intégré, préchargement des suivantes), zoom 100 % sur un point au clic.
- **Notes** de 1 à 5 étoiles, **drapeaux** retenue ou rejetée, **étiquettes couleur** (5 couleurs), le tout au clavier (section 6).
- **Avance automatique** à la photo suivante après une notation (option).
- **Comparaison côte à côte** de 2 à 4 photos, avec zoom et déplacement synchronisés.
- **Rafales empilées** \[V0.4\] : regroupement des photos prises à moins de 2 secondes d'intervalle et visuellement proches (hash perceptuel). La pile affiche la meilleure photo au-dessus et s'ouvre d'un clic.
- **Détection des ratés** \[V0.4\], entièrement en local : flou ou bougé, yeux fermés, sous ou surexposition. Les photos concernées portent un badge, et un filtre « Ratés probables » permet de les revoir et de les rejeter en lot. Afterdusk ne rejette jamais rien seul.
- **Rejets** : un raccourci envoie toutes les photos rejetées d'un album dans la corbeille interne.

### 4.4 Corbeille \[V0.1\]

- Les fichiers supprimés sont déplacés dans un dossier corbeille de la bibliothèque, restaurables à leur emplacement d'origine pendant **30 jours** (durée réglable).
- Vidage automatique après le délai, ou manuel avec confirmation.
- La copie de sauvegarde n'est jamais supprimée automatiquement.

### 4.5 Retouche \[V0.2\]

Toute retouche est non destructive : un historique de réglages appliqué à l'original à l'affichage et à l'export.

- **Profil de base** : rendu neutre fidèle au boîtier par défaut, avec correction d'objectif automatique si l'objectif est reconnu.
- **Réglages essentiels** (toujours visibles) : exposition, luminosité (tons moyens éclaircis ou assombris sans déplacer le blanc ni le noir), balance des blancs (température, teinte, pipette, préréglages), contraste, hautes lumières, ombres, blancs, noirs, vibrance, saturation.
- **Réglages automatiques, demandés par l'utilisateur (2026-10-19)** : un bouton « Auto » dans la section Lumière règle exposition, contraste, hautes lumières, ombres, blancs et noirs d'après l'histogramme de la photo ; Maj + double-clic sur un curseur règle ce seul curseur (le double-clic simple le remet à zéro) ; « Auto » s'ajoute aux choix de balance des blancs (estimation du gris neutre de la scène). Le résultat n'est qu'un jeu de valeurs ordinaires, modifiables et annulables, inscrit dans l'historique comme une seule étape « Auto ». Calcul local et déterministe (même photo, même résultat), sans réseau ni service en ligne ; il se calibre une fois contraste, hautes lumières et ombres présents dans le pipeline.
- **Réglages intermédiaires** (sections repliables) : courbe de tonalité (RVB et par canal), couleurs par teinte (TSL sur 8 teintes), netteté, réduction du bruit (luminance et couleur), vignetage, grain.
- **Géométrie** : recadrage avec ratios prédéfinis, redressement avec horizon automatique, rotation et miroir. Ratios présentés en barre d'icônes à la forme du cadre, comme dans l'application Photos de Windows (demande de l'utilisateur, 2026-10-22) : libre, original, carré, 9:16, 16:9, 4:5, 5:4, 3:4, 4:3, 2:3, 3:2, 5:7, 7:5, 1:2, 2:1.
- **Presets** : fournis (une dizaine, sobres) et personnels, créés depuis les réglages d'une photo, appliqués à une sélection, avec intensité réglable de 0 à 100 %.
- **Galerie de filtres, inspirée de l'application Photos de Windows (2026-10-19)** : les presets fournis (simulations de film comprises) présentés en vignettes calculées sur la photo en cours, appliqués d'un clic et dosés par le curseur d'intensité. Un filtre n'est qu'un jeu de réglages ordinaires : il reste modifiable curseur par curseur et s'exporte comme les autres retouches. Objectif : traiter vite et de façon professionnelle, sans recréer la photo. Présentation précisée par l'utilisateur (2026-10-19, d'après Lightroom et Photos de Windows) : bouton « Optimisation automatique » en tête (le bouton Auto), vignette « Original » en premier (coche sur le choix actif), puis les presets en grille de trois colonnes, rangés par groupes repliables ; un groupe peut décliner un seul réglage (« Exposition » : -1, -2, -3 IL…), chaque vignette montrant la photo en cours ainsi réglée.
- **Simulations de film, demandées par l'utilisateur (2026-10-19)**, à placer et à valider : rendus inspirés des pellicules couleur et noir et blanc (négatifs couleur, inversibles, noir et blanc classiques), appliqués comme un preset avec intensité réglable, éventuellement accompagnés de grain. Mise en œuvre envisagée : tables de correspondance 3D (LUT `.cube` ou Hald CLUT) appliquées sur le GPU, plus une courbe et un grain propres à chaque film. Deux précautions : les noms commerciaux (Kodak Portra, Fujifilm Velvia, Ilford HP5…) sont des marques, donc des noms descriptifs (« Négatif couleur chaud », « Inversible saturé », « Noir et blanc 400 ») sont à préférer ; et toute LUT reprise d'ailleurs doit avoir une licence compatible GPL v3, sinon elle est recréée par nos soins. Présentation souhaitée par l'utilisateur (2026-10-19) : chaque simulation illustrée comme une boîte de pellicule. Les logos et photos d'emballages des fabricants étant des marques et des images protégées, les illustrations sont dessinées pour Afterdusk (cartouche et boîte vectorielles, couleurs évoquant le rendu, sensibilité en gros chiffres, nom descriptif), sans logo ni nom de marque.
- **Copier-coller et synchronisation** des réglages entre photos, avec choix des réglages à transmettre.
- **Avant/après** : touche maintenue ou vue scindée.
- **Historique** illimité avec annuler et rétablir, et **instantanés** nommés : panneau listant chaque traitement appliqué (« Exposition +0,7 », « Auto », « Balance des blancs : Tungstène »), un clic sur une ligne revient à cet état, Ctrl+Z et Ctrl+Maj+Z (Cmd sur Mac) pour annuler et rétablir.
- **Copies virtuelles** : plusieurs versions d'une même photo sans dupliquer le fichier.
- **Correcteur de défauts, remonté en V0.2 à la demande de l'utilisateur (2026-10-19)** : poussières du capteur et petites imperfections, au pinceau rond de taille et d'adoucissement réglables. Chaque correction remplace la zone par une zone voisine de la même photo, choisie automatiquement et déplaçable (mode correction, qui fond les bords, ou clonage) : rien n'est inventé. Les corrections sont des réglages comme les autres (position et rayon relatifs à l'image, donc indépendants de la définition), enregistrés dans le JSON et le XMP, copiables d'une photo à l'autre pour les poussières qui restent au même endroit d'une série ; une visualisation des poussières (contraste fortement accentué) aide à les repérer.
- **Avertissements d'écrêtage** des hautes lumières et des ombres sur l'image et l'histogramme.
- **Performance cible** : réglage reflété en moins de 100 ms sur un aperçu de 2048 px avec une carte graphique intégrée récente.

### 4.6 Livraison et export \[V0.2, galerie web et planches contact en V0.4\]

- **Presets d'export** fournis et modifiables :

| Preset | Format | Taille | Métadonnées |
| --- | --- | --- | --- |
| Originaux | Fichier d'origine, sans retouche | Inchangée | Toutes |
| Pleine qualité | JPEG qualité 95 | Pleine résolution | Toutes |
| Web | JPEG qualité 85, sRGB | 2048 px côté long | Sans GPS |
| Instagram | JPEG, sRGB | 1080 × 1350 (4:5) ou 1080 px | Sans GPS ni numéro de série |
| Impression | TIFF 16 bits ou JPEG qualité 100, Adobe RGB | Pleine résolution | Toutes |

- **Formats disponibles** : JPEG, HEIF, AVIF, TIFF 8 ou 16 bits, PNG, DNG pour les originaux.
- **Options** : redimensionnement (côté long, largeur, hauteur, pourcentage), espace colorimétrique, accentuation de sortie (écran ou papier), modèle de nom des fichiers, dossier de destination, ouverture du dossier à la fin.
- **Filigrane** texte, image (logo PNG) ou logo du studio (Réglages > Identité), position sur une grille de 9 points, taille relative à l'image, opacité, marge.
- **Nettoyage des métadonnées** : tout garder, retirer le GPS, retirer le numéro de série du boîtier, ne garder que le copyright, ou tout retirer.
- **Export multiple** : plusieurs presets en une seule opération (par exemple Web et Instagram), dans des sous-dossiers.
- **Galerie web** \[V0.4\] : génération d'un site statique (HTML, CSS, images) d'un album ou d'une collection, avec thème clair ou sombre, titre, texte d'introduction, téléchargement optionnel des images, à héberger où l'utilisateur le souhaite. Option de sélection client : le client coche ses photos préférées et renvoie la liste par courriel préparé ou par fichier, qu'Afterdusk sait réimporter comme drapeaux « Retenue », réunies dans une collection. Photos en 2 048 px tirées des aperçus retouchés, sans aucune métadonnée, filigrane en option ; la page n'appelle aucune ressource extérieure.
- **Modèles de livraison** \[V0.5\] (demande de l'utilisateur, 2026-10-22, inspirée de Canva) : les formats présentés en vignettes groupées par usage (réseaux sociaux, impression, web), chacune montrant la photo sélectionnée recadrée au bon format : story 9:16 (1080 × 1920), publication 4:5 (1080 × 1350), couverture et publication Facebook, publication LinkedIn, épingle Pinterest 2:3, miniature YouTube (1280 × 720), tirages 10 × 15, 13 × 18 et A4 à 300 dpi. Recadrage centré, ou sur le visage détecté, ajustable d'un glisser. Les réseaux sont nommés, sans reprendre leurs logos (marques déposées). Liste arrêtée avec l'utilisateur le 2026-10-22 : 4:5, carré, story 9:16, Facebook et LinkedIn (1200 × 630), couverture Facebook (1640 × 624), Pinterest 2:3, miniature YouTube, X 16:9 (1600 × 900) ; tirages 10 × 15, 13 × 18, 20 × 30 et A4 à 300 dpi, qui suivent l'orientation de la photo ; plein écran web (2560 × 1440).
- **Éditeur de livraison simple** \[V0.5\] (demande de l'utilisateur, 2026-10-22) : sur le rendu retouché, au format d'un modèle, ajouter du texte, le logo, un cadre ou une marge colorée, un fond, et un collage de 2 à 4 photos ; export à pleine résolution. Bibliothèque pressentie : Konva (`react-konva`, licence MIT). Pas de logiciel de graphisme : ni calques libres, ni dessin, ni effets. Dispositions ajoutées à la demande de l'utilisateur (2026-10-22) : bande photomaton (5 × 15 cm, doublable sur un 10 × 15) et Polaroid (88 × 107 mm, légende dans le bord du bas). Polices : cinq livrées (Inter, Caveat, Playfair Display, Cormorant Garamond, Bebas Neue, toutes OFL) et celles installées sur l'ordinateur, lues pour le rendu sans jamais être copiées ; une police absente est remplacée par Inter, et l'éditeur le signale.
- **Planches contact** \[V0.4\] (idée de l'utilisateur, décision 0009) : PDF d'un album ou d'une collection, pour un client ou pour l'archive. Grille de 2 à 8 colonnes, A4 ou Lettre, portrait ou paysage (par défaut 4 colonnes en A4 portrait). Sous chaque vignette, son numéro d'ordre et le nom du fichier. En-tête : nom de l'album ou de la collection, client et séance, date de la séance, photographe. Vignettes tirées des aperçus rendus, retouches comprises. Filigrane en option, désactivé par défaut : texte ou logo du studio. Logo du studio en tête de page s'il y en a un.

### 4.7 Localisation et carte du monde \[V0.3\]

- **Sources de lieu**, dans cet ordre de priorité :
  1. GPS déjà présent dans les photos (boîtier équipé ou application smartphone de la marque).
  2. Trace GPX importée (montre, téléphone) : chaque photo prend la position la plus proche dans le temps, avec réglage du décalage d'horloge du boîtier.
  3. Photo de smartphone du même jour : l'utilisateur dépose une ou plusieurs photos de téléphone, leur position s'applique aux photos prises dans une fenêtre de temps réglable (par défaut 30 minutes).
  4. Saisie manuelle : recherche d'une ville ou clic sur la carte, pour un album entier ou une sélection.
- **Fuseaux horaires** : détection du décalage entre l'heure du boîtier et l'heure locale du lieu, avec correction proposée.
- **Géocodage inverse hors ligne** : pays, région et ville déduits des coordonnées.
- **Carte du monde** : chaque album est un point (sa position moyenne ou principale), regroupés en grappes au dézoom, avec la miniature de couverture. Un clic ouvre l'aperçu de l'album, un double clic l'album lui-même.
- **Vue détaillée** : en zoomant sur un album, les photos apparaissent individuellement, et la trace GPX du jour si elle existe.
- **Filtre par zone** : dessiner un rectangle sur la carte pour afficher les photos de cette zone.
- **Hors ligne** : sans connexion, la carte affiche un fond simplifié (frontières des pays) embarqué dans l'application.

### 4.8 Gestion du matériel \[V0.3\]

- **Détection automatique** : chaque boîtier et objectif rencontré dans les métadonnées crée une fiche (marque, modèle, numéro de série si disponible). L'utilisateur confirme, fusionne les doublons ou masque un appareil qui n'est pas à lui (photos d'un ami).
- **Fiche détaillée** : type (boîtier, objectif, flash, filtre, trépied, autre), marque, modèle, numéro de série, date et lieu d'achat, prix, état, garantie jusqu'au, photo du matériel, notes.
- **Accessoires saisis à la main** (filtres, trépieds, sacs, batteries), puisqu'ils n'apparaissent pas dans les métadonnées.
- **Illustration de chaque appareil** (demande de l'utilisateur, 2026-10-19) : la marque et une image du boîtier ou de l'objectif sur sa fiche et dans la liste. Contraintes : aucun appel réseau (pas d'images téléchargées) et pas de logos ni de photos de fabricants livrés avec l'application (marques déposées et photos protégées). Proposé : la photo de l'utilisateur (choisie dans la bibliothèque ou déposée sur la fiche), à défaut une silhouette dessinée pour Afterdusk selon le type (hybride, reflex, compact, objectif fixe ou zoom), et le nom de la marque en toutes lettres. À trancher au début de la V0.3.
- **Statistiques** : nombre de photos par boîtier et par objectif, répartition des focales utilisées (histogramme), ouvertures et ISO les plus fréquents, évolution dans le temps, objectif favori par type de scène ou par album.
- **Compteur de déclenchements** : lu dans les métadonnées quand la marque l'y inscrit, sinon estimé à partir des photos importées.
- **Suivi d'entretien** : journal par appareil (nettoyage capteur, révision, réparation, mise à jour de firmware) avec date, coût et notes, et rappels optionnels (par exemple nettoyage capteur tous les 6 mois).
- **Export d'inventaire** en PDF ou CSV, utile pour l'assurance.

### 4.9 Vidéos \[V0.4\]

- Import, rangement, sauvegarde et tri identiques aux photos (notes, drapeaux, couleurs, corbeille).
- Miniature, durée, résolution, cadence et codec affichés ; lecture intégrée avec défilement image par image.
- Pas de montage ni de retouche vidéo ; export limité à la copie des originaux.
- **Traitement vidéo simple, demandé par l'utilisateur (2026-10-19)**, à placer après la V0.4 et à valider : couper le début et la fin d'un clip (sans réencodage quand c'est possible), extraire une image en photo, réglages de base (exposition, balance des blancs, contraste, saturation, recadrage, redressement) appliqués par le même moteur que les photos, export H.264 ou HEVC par FFmpeg. Pas de montage multi-pistes, de transitions ni d'étalonnage avancé : le niveau de DaVinci Resolve n'est pas visé.
- Fichiers annexes des boîtiers (proxys, fichiers XML de métadonnées) rattachés à leur vidéo et déplacés avec elle.
- **Dérushage** \[après la V0.4\] (demande de l'utilisateur, 2026-10-22), dans le prolongement du traitement simple : marquer entrée et sortie, découper un clip en morceaux, garder ou jeter chacun, exporter sans réencodage quand c'est possible, par FFmpeg. LosslessCut sert de référence de fonctionnement (licence à vérifier avant toute reprise de code). Les codecs que le lecteur intégré ne lit pas (HEVC, ProRes selon le système) passent par un proxy H.264 généré par FFmpeg.

### 4.10 Réglages \[V0.1 et suivantes\]

- Emplacement de la bibliothèque et de la sauvegarde.
- Modèles de rangement et de renommage, avec aperçu du résultat.
- Identité du photographe (nom, copyright, site) pour les métadonnées et le filigrane.
- Types de séance \[V0.4\] : ajouter, renommer ou retirer un type de la liste proposée pour les albums ; un type retiré laisse ses albums sans type.
- Logo du studio (PNG, JPEG ou WebP, copié dans les données d'Afterdusk) : en tête des documents PDF (planches contact, inventaire du matériel) et comme filigrane (idée de l'utilisateur).
- Langue (français, anglais, ou celle du système), thème (sombre par défaut, clair, système).
- Durée de la corbeille, taille maximale du cache d'aperçus.
- Comportement à l'insertion d'une carte (ouvrir l'import, notifier, ne rien faire, ou importer automatiquement une carte connue \[V0.4\]).
- Import automatique \[V0.4\] : dossiers surveillés, réception FTP (désactivée par défaut), et la liste des appels réseau autorisés, chacun désactivable.

## 5. Modèle de données

La base SQLite indexe la bibliothèque ; les fichiers et leurs XMP restent la référence. Si la base est perdue, une analyse de la bibliothèque doit pouvoir la reconstruire (albums, notes, retouches) à partir des dossiers et des XMP.

| Table | Champs principaux | Liens |
| --- | --- | --- |
| `libraries` | id, nom, chemin racine, chemin de sauvegarde, modèle de rangement, modèle de renommage, en ligne oui/non | Contient des albums |
| `albums` | id, nom, date de début, date de fin, dossier, lieu (lat, lon, ville, pays), couverture, notes, id du client, type de séance \[V0.4\] | Appartient à une bibliothèque ; contient des médias ; lié à un client |
| `clients` \[V0.4\] | id, nom, courriel et téléphone (facultatifs), notes | Plusieurs albums |
| `media` | id, type (photo ou vidéo), chemin, nom d'origine, taille, empreinte, date de prise de vue, fuseau, largeur, hauteur, orientation, lat, lon, source du lieu, note, drapeau, couleur, état (normal, corbeille), date de mise en corbeille | Appartient à un album ; lié à un boîtier et un objectif |
| `media_exif` | id du média, focale, ouverture, vitesse, ISO, compensation, mode, flash, numéro de série boîtier, compteur de déclenchements, EXIF bruts (JSON) | Un pour un avec `media` |
| `edits` | id, id du média, nom de version (original ou copie virtuelle), réglages (JSON versionné), date de modification | Plusieurs versions par média |
| `edit_history` | id, id d'édition, ordre, réglages, libellé de l'étape, instantané nommé oui/non | Historique d'une version |
| `presets` | id, nom, type (retouche ou export), réglages (JSON), fourni ou personnel | Aucun |
| `collections` | id, nom, type (manuelle ou intelligente), filtre (JSON) | Plusieurs médias via `collection_media` |
| `keywords` | id, mot | Plusieurs médias via `media_keywords` |
| `stacks` | id, id du média de tête, type (rafale) | Regroupe des médias via `media.stack_id` |
| `analysis` | id du média, score de netteté, yeux fermés, exposition, hash perceptuel, version de l'analyse | Un pour un avec `media` |
| `gear` | id, type, marque, modèle, numéro de série, date d'achat, lieu d'achat, prix, garantie, état, photo, notes, masqué, détecté automatiquement | Lié aux médias (boîtier, objectif) |
| `gear_aliases` | id du matériel, chaîne EXIF (nom du modèle tel que le boîtier l'écrit) | Permet de fusionner les variantes de noms |
| `maintenance` | id, id du matériel, date, type, coût, notes, rappel (intervalle) | Journal d'un matériel |
| `gpx_tracks` | id, id d'album, fichier, début, fin, points (géométrie simplifiée) | Rattaché à un album |
| `imports` | id, date, support source, nombre de fichiers, sauvegarde faite, erreurs | Journal des imports |
| `settings` | clé, valeur (JSON) | Aucun |

**Règles importantes**

- Les réglages de retouche sont un objet JSON **versionné** (champ `version`), pour que les évolutions du moteur restent compatibles avec les anciennes retouches.
- Toute modification de note, drapeau, couleur, mots-clés ou retouche est écrite dans la base **et** dans le XMP de l'original, de façon groupée et en arrière-plan.
- Les chemins sont stockés relativement à la racine de la bibliothèque, pour qu'un disque externe ou un NAS puisse changer de lettre ou de point de montage.

## 6. Interface et ergonomie

L'interface s'organise autour d'une barre d'étapes toujours visible en haut (Importer, Trier, Retoucher, Livrer) et d'une barre latérale pour les vues transverses (Bibliothèque, Carte, Matériel, Réglages).

**Principes**

- **Une tâche par écran** : chaque étape n'affiche que les outils qui la concernent. Pas de panneaux flottants ni de modules cachés.
- **Thème sombre neutre par défaut** (gris moyen, sans couleur parasite autour des photos), thème clair disponible.
- **Vocabulaire simple** : « Lumière » plutôt que « Tonalité », « Couleurs » plutôt que « TSL », avec le terme technique en infobulle.
- **Clavier d'abord pour le tri**, souris d'abord pour la retouche.
- **Retours visibles** : toute action longue affiche sa progression ; toute action destructive est annulable.
- **Première ouverture guidée** : choix de la bibliothèque, de la sauvegarde, du modèle de rangement et de l'identité du photographe en quatre écrans, avec des valeurs par défaut raisonnables.
- **Profils d'interface** \[V0.5\] (demande de l'utilisateur, 2026-10-22) : l'assistant de bienvenue demande comment l'utilisateur compte se servir d'Afterdusk et propose trois profils, Simplifiée, De base et Complète, pour qu'un débutant ne soit pas noyé. Un profil masque des fonctions sans les retirer : ce qui est masqué continue de tourner (analyse des ratés, sauvegarde, aperçus) et les données restent intactes. Le profil se change à tout moment (Réglages > Général, menu Affichage) ; une section masquée garde un lien « Afficher les réglages avancés ». Modules, sections de panneaux, entrées de menu et raccourcis suivent le même filtre. Les installations existantes restent en interface complète. Ce choix de l'utilisateur fait exception au principe « pas de modules cachés ».

| | Simplifiée | De base | Complète |
| --- | --- | --- | --- |
| Modules | Importer, Trier, Retoucher, Livrer | + Bibliothèque, Carte | + Matériel |
| Import | Carte ou dossier, album | + Renommage, mots-clés | + Sauvegarde, import automatique, dossiers surveillés, réception FTP |
| Tri | Notes, drapeaux, ratés probables | + Couleurs, filtres, rafales | + Recherche avancée, albums intelligents |
| Retouche | Lumière, couleur, recadrage, presets | + Courbes, TSL, détail, corrections d'objectif | + Copies virtuelles, instantanés, correcteur |
| Livraison | Presets d'export, filigrane simple | + Planches contact, galerie web | + Options d'export détaillées, métadonnées, clients |
- **Accessibilité** : navigation complète au clavier, contrastes suffisants, tailles de texte réglables.

**Écrans principaux**

| Écran | Contenu |
| --- | --- |
| Importer | Support détecté, grille groupée par jour, panneau de destination (album, rangement, renommage, sauvegarde, mots-clés) |
| Trier | Grille ou visionneuse plein écran, bandeau de miniatures, filtres rapides (non notées, retenues, rejetées, ratés probables), mode comparaison |
| Retoucher | Image au centre, bandeau de miniatures en bas, réglages à droite (essentiels ouverts, avancés repliés), presets et historique à gauche ; barre d'outils verticale au bord droit, comme Lightroom (demandée par l'utilisateur le 2026-10-20, pour ne pas noyer l'utilisateur sous les réglages) : Réglages, Recadrer, Corriger, puis Filtres avec la galerie, un seul outil à la fois dans le panneau |
| Livrer | Sélection, choix des presets, options (filigrane, métadonnées), aperçu du résultat, destination |
| Bibliothèque | Albums en grille ou en liste, filtres, recherche, collections |
| Carte | Carte du monde plein écran, albums en points groupés, panneau d'aperçu de l'album sélectionné |
| Matériel | Liste des appareils, fiche détaillée, statistiques, journal d'entretien |

**Raccourcis clavier** (identiques sur Windows et macOS, Ctrl devenant Cmd)

| Touche | Action |
| --- | --- |
| Flèches gauche et droite | Photo précédente ou suivante |
| 0 à 5 | Note de 0 à 5 étoiles |
| P | Retenue |
| X | Rejetée |
| U | Retirer le drapeau |
| 6 à 9 | Étiquettes rouge, jaune, vert, bleu |
| Espace | Zoom 100 % ou ajusté |
| C | Mode comparaison |
| G | Grille |
| E | Ouvrir dans Retoucher |
| \\ | Avant/après |
| Ctrl+Maj+C et Ctrl+Maj+V | Copier et coller les réglages |
| Ctrl+Z et Ctrl+Maj+Z | Annuler et rétablir |
| Suppr | Envoyer dans la corbeille |
| Ctrl+Maj+E | Exporter |
| ? | Aide des raccourcis |

Ces raccourcis reprennent ceux de Lightroom quand ils existent, pour faciliter la transition.

**Traductions** : aucun texte en dur dans l'interface. Toutes les chaînes passent par i18next, avec `fr.json` et `en.json` maintenus ensemble ; les dates, nombres et unités suivent la langue choisie.

## 7. Feuille de route

Le développement avance en quatre versions, chacune utilisable seule : on ne passe à la suivante que lorsque tous les critères d'acceptation de la précédente sont cochés.

### V0.1 : Importer et trier

Objectif : remplacer l'explorateur de fichiers pour décharger et trier une journée de photos.

- [x] Projet Tauri 2 qui se compile et se lance sur Windows et macOS, avec intégration continue GitHub Actions sur les deux systèmes
- [x] Première ouverture guidée (bibliothèque, sauvegarde, rangement, identité)
- [x] Une carte SD insérée est détectée en moins de 5 secondes et l'import proposé
- [x] Import de 500 RAW avec rangement, renommage, sauvegarde et vérification par somme de contrôle, sans aucune perte, en cas de coupure comprise (reprise propre au redémarrage)
- [x] Les photos déjà importées sont reconnues et grisées lors d'un second import
- [x] Aperçus affichés dès la fin de la copie de chaque fichier, sans attendre la fin de l'import
- [x] Passage d'une photo à la suivante en moins de 100 ms dans la visionneuse
- [x] Notes, drapeaux, couleurs au clavier, écrits en base et dans le XMP
- [x] Comparaison côte à côte de 2 à 4 photos avec zoom synchronisé
- [x] Corbeille interne avec restauration et vidage après 30 jours
- [x] Vue Bibliothèque avec albums, filtres de base (date, note, drapeau, boîtier, objectif) et recherche
- [x] Bibliothèque sur disque externe débranché consultable via le cache
- [x] RAW testés au minimum : Fujifilm (RAF X-Trans), Canon (CR3), Nikon (NEF), Sony (ARW), Panasonic (RW2), DNG
- [x] Interface en français et en anglais

### V0.2 : Retoucher et livrer

Objectif : traiter et exporter les photos retenues sans autre logiciel.

- [x] Pipeline de développement GPU avec rendu neutre fidèle au boîtier par défaut
- [x] Tous les réglages essentiels et intermédiaires de la section 4.5, avec retour visuel en moins de 100 ms
- [x] Corrections d'objectifs automatiques via lensfun
- [x] Recadrage, redressement, ratios
- [x] Correcteur de défauts (poussières, petites imperfections), corrections copiables entre photos
- [x] Presets fournis et personnels, intensité réglable, copier-coller et synchronisation des réglages
- [x] Historique, instantanés, copies virtuelles, avant/après
- [x] Retouches enregistrées dans le XMP et restaurées après reconstruction de la base
- [x] Les cinq presets d'export de la section 4.6, formats JPEG, HEIF, AVIF, TIFF, PNG
- [x] Filigrane texte et image, nettoyage des métadonnées, export multiple
- [x] Export de 100 photos en JPEG 2048 px en moins de 2 minutes sur une machine de milieu de gamme

### V0.3 : Carte et matériel

Objectif : retrouver ses photos par lieu et suivre son matériel.

- [x] Les quatre sources de localisation de la section 4.7, avec correction du fuseau horaire
- [x] Géocodage inverse hors ligne (pays, ville)
- [x] Carte du monde des albums avec grappes, aperçu et filtre par zone
- [x] Fond de carte simplifié hors ligne
- [x] Inventaire alimenté automatiquement, fusion des doublons, masquage
- [x] Fiches complètes, accessoires manuels, statistiques d'usage avec graphiques
- [x] Journal d'entretien et rappels
- [x] Export d'inventaire PDF et CSV
- [x] Albums intelligents

### V0.4 : Assistants et partage

Objectif : gagner du temps au tri et partager avec des clients.

- [x] Regroupement des rafales en piles
- [x] Détection locale du flou, des yeux fermés et des problèmes d'exposition, avec filtre « Ratés probables »
- [x] Analyse d'une journée de 1 000 photos en moins de 10 minutes en arrière-plan, sans ralentir le tri
- [x] Vidéos : import, rangement, tri, lecture, fichiers annexes
- [x] Galerie web statique avec sélection client et réimport des choix
- [x] Planches contact en PDF d'un album ou d'une collection, numérotées, avec en-tête et filigrane en option
- [x] Clients et types de séance rattachés aux albums, avec filtres, recherche et suppression complète d'un client
- [x] Import automatique : carte connue, dossier surveillé et réception FTP sur le réseau local, chaque photo affichée dans le tri moins de 5 secondes après son arrivée
- Installateurs signés et mises à jour automatiques : reportés à l'étape « Distribution », en dernier (choix de l'utilisateur, 2026-10-22).

### V0.5 : Livraison créative

Objectif : livrer aux bons formats, habiller une photo sans quitter Afterdusk, et adapter l'interface au niveau du photographe (demandes de l'utilisateur, 2026-10-22).

- [x] Modèles de livraison présentés en vignettes, groupés par usage, avec recadrage ajustable par format
- [x] Éditeur de livraison simple : texte, logo, cadre, fond, collage de 2 à 4 photos, export à pleine résolution
- [x] Dérushage vidéo : entrée, sortie, découpe, export sans réencodage
- [x] Profils d'interface Simplifiée, De base et Complète, proposés par l'assistant de bienvenue et modifiables à tout moment

### V0.6 : Retouches locales

Objectif : régler une partie de la photo seulement, un ciel, un visage, le sol (première piste de la V1, choix de l'utilisateur, 2026-10-24 ; décision 0014).

- [ ] Dégradés linéaire et radial, déplaçables, inversables, adoucis, sur RAW, JPEG et HEIF
- [x] Sept réglages par masque (exposition, contraste, hautes lumières, ombres, température, teinte, saturation), dans l'historique et le copier-coller des réglages
- [x] Même rendu à l'aperçu, au zoom 100 %, à l'export et après relecture du XMP
- [x] Retour visuel d'un réglage en moins de 100 ms avec 8 masques (mesuré avec le rendu de l'application, sur la carte graphique : 3 ms sur Mac M3 Pro, 8 ms sur PC ; limite connue : 104 ms sur le Mac au processeur seul, rendu de secours sans carte graphique utilisable)
- Pinceau et gomme : version suivante.

### Distribution : en dernier

Objectif : livrer Afterdusk au public une fois l'application entièrement opérationnelle (choix de l'utilisateur, 2026-10-22). Signature selon la décision 0011.

- [ ] Installateurs signés pour Windows et macOS, mises à jour automatiques

Plan et démarches : `docs/distribution.md` ; mises à jour, installateur et identifiant : décision 0013.

**Après la V0.5** (pistes pour une V1) : retouches locales (masques, pinceau, dégradés), suppression de défauts, simulations de film, connexion en direct au boîtier (tethering, voir ci-dessous), version Linux, traductions communautaires.

**Tethering** (demande de l'utilisateur, 2026-10-22) : en attendant, le dossier surveillé de la V0.4 sert de tethering simple, les logiciels gratuits des fabricants (EOS Utility, NX Tether, Imaging Edge, X Acquire) pilotant le boîtier et déposant les photos dans un dossier. Ensuite, libgphoto2 (LGPL 2.1) sous macOS et Linux. Sous Windows, libgphoto2 impose de remplacer le pilote USB de l'appareil ; les SDK des fabricants demanderaient une exception à la GPL : décision à prendre plus tard. Références proposées par l'utilisateur : darktable (GPL v3, même licence qu'Afterdusk, code réutilisable) pour l'intégration de libgphoto2 ; digiCamControl (MIT, réutilisable avec sa notice) pour Windows, qui pilote Nikon et Sony en PTP par l'interface Windows Portable Devices, sans remplacer le pilote USB, et Canon par le SDK du fabricant. La voie Windows Portable Devices est à prototyper en premier sous Windows.

## 8. Consignes pour Claude Code

Ce cahier des charges est exporté en Markdown dans `docs/cahier-des-charges.md` du dépôt (par exemple `G:\MesProjets\Afterdusk`), et un fichier `CLAUDE.md` à la racine donne les règles de travail. Claude Code travaille une version à la fois, par petites étapes vérifiables.

**Méthode de travail**

1. Lire le cahier des charges et `CLAUDE.md` en début de session.
2. Pour chaque critère d'acceptation : proposer un plan court, coder, tester, puis cocher le critère dans `docs/avancement.md`.
3. Valider chaque choix de bibliothèque marqué « à valider » par un petit prototype avant de l'adopter, et consigner la décision dans `docs/decisions/`.
4. Faire un commit par fonctionnalité, avec un message clair en français.
5. S'arrêter et demander avant toute décision difficile à défaire (schéma de base, format des réglages, structure des dossiers de la bibliothèque).

**Contenu de CLAUDE.md**

```markdown
# Afterdusk

Logiciel open source (GPL v3) de gestion photo pour photographes semi-pro.
Tauri 2 + Rust (src-tauri/) et React + TypeScript (src/).
Cahier des charges : docs/cahier-des-charges.md. Avancement : docs/avancement.md.

## Règles absolues
- Ne jamais modifier, déplacer ou supprimer un original sans action explicite de l'utilisateur.
- Ne jamais écrire sur la carte SD ou le boîtier source.
- Toute copie est vérifiée par somme de contrôle avant d'être considérée comme faite.
- Toute suppression passe par la corbeille interne.
- Aucun appel réseau hors des exceptions listées ici. Chaque exception se désactive dans les Réglages et n'envoie ni photo ni donnée personnelle à un tiers sans action de l'utilisateur. Exceptions : tuiles de carte, vérification des mises à jour, réception FTP sur le réseau local (désactivée par défaut, décision 0009). Toute nouvelle exception est ajoutée ici après accord de l'utilisateur.

## Conventions
- Rust : cargo fmt et cargo clippy sans avertissement ; erreurs avec thiserror, jamais de unwrap hors tests.
- TypeScript strict, ESLint et Prettier ; composants fonctionnels React.
- Aucun texte d'interface en dur : tout passe par i18next (fr.json et en.json à jour ensemble).
- Traitements longs dans des tâches Rust en arrière-plan, progression envoyée par événements Tauri.
- Chemins de médias stockés relativement à la racine de la bibliothèque.
- Réglages de retouche en JSON versionné ; toute évolution du format prévoit une migration.
- Migrations SQLite numérotées dans src-tauri/src/db/migrations/.
- Dépendances compatibles GPL v3 uniquement.

## Tests
- cargo test pour le cœur, Vitest pour l'interface.
- Tests d'import et de décodage sur les échantillons de test-assets/ (RAW libres de droits de raw.pixls.us).
- Un test par règle absolue (aucun original modifié, aucune écriture sur la source).
- Tester sur Windows et macOS via GitHub Actions avant de cocher un critère.

## Façon de travailler
- Une version à la fois, dans l'ordre de docs/cahier-des-charges.md section 7.
- Plan court avant chaque fonctionnalité, commit par fonctionnalité, messages en français.
- Demander avant une décision difficile à défaire.
```

**Prompt de démarrage** (à coller dans Claude Code pour lancer la V0.1)

```markdown
Lis docs/cahier-des-charges.md et CLAUDE.md.
Nous démarrons la V0.1 (Importer et trier).

1. Initialise le projet Tauri 2 avec React + TypeScript + Vite selon la structure de la section 3,
   avec l'intégration continue GitHub Actions pour Windows et macOS.
2. Prototype la liaison LibRaw en Rust : extraction du JPEG intégré et des EXIF
   sur les échantillons RAF, CR3, NEF, ARW, RW2 et DNG de test-assets/.
3. Prototype l'écriture XMP (Exiv2 ou xmp-toolkit-rs) et consigne ton choix dans docs/decisions/.
4. Propose le schéma SQLite de la V0.1 et attends ma validation avant de coder l'import.

Crée docs/avancement.md avec les critères de la V0.1 et coche-les au fur et à mesure.
```

## 9. Points ouverts et hors périmètre

**Points ouverts**

- [ ] Vérifier la disponibilité du nom Afterdusk auprès de l'INPI, de l'EUIPO et de l'USPTO (classes 9 et 42), et réserver un nom de domaine (afterdusk.app ou afterdusk.fr). Hébergement pressenti : `afterdusk.flux-croises.fr`, sur le domaine de l'association de l'utilisateur ; l'espace de noms XMP s'y rattache déjà (décision 0002)
- [x] Choisir le nom du dépôt GitHub (le compte « afterdusk » est déjà pris) : `fluxcroises/afterdusk`, sous l'organisation de l'association (d'abord `lewisbedar/afterdusk-app`)
- [x] Choisir entre Exiv2 et xmp-toolkit-rs pour les métadonnées (décision 0002 : xmp-toolkit-rs, EXIF des RAW lus par LibRaw)
- [x] Choisir le fournisseur de tuiles de carte et vérifier ses conditions d'usage (décision 0008 : OpenFreeMap, sans clé ni cookies, usage commercial permis, attribution OpenStreetMap et OpenMapTiles affichée)
- [x] Choisir les modèles ONNX de détection des yeux fermés et vérifier leur licence (décision 0010 : YuNet, MIT ; MediaPipe Face Mesh V2, Apache 2.0 ; ONNX Runtime 1.23.2, MIT)
- [x] Signature des applications (décision 0011 : SignPath Foundation pour Windows ; macOS non signé au début, Apple Developer Program plus tard ; clé de mise à jour Tauri)
- [ ] Logo, icône et identité visuelle

**Hors périmètre jusqu'à la V1**

- Retouches locales (masques, pinceau, dégradés), suppression d'objets ou de grandes zones, fusion HDR et panoramas ; le correcteur de poussières et de petites imperfections est prévu en V0.2 (section 4.5)
- Retouches génératives ou par intelligence artificielle qui inventent du contenu (effacement génératif, remplacement ou flou d'arrière-plan, agrandissement par IA) : Afterdusk traite la photo prise, il ne la recrée pas
- Montage vidéo (plusieurs clips, transitions, pistes) et étalonnage avancé ; le traitement simple d'un clip est prévu (section 4.9)
- Synchronisation cloud, comptes utilisateurs, applications mobiles
- Impression et mise en page, hormis les modèles et l'éditeur de livraison simple de la V0.5
- Connexion en direct au boîtier (tethering)
- Version Linux

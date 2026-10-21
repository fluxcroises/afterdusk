# Avancement

Un critère n'est coché qu'une fois testé sur Windows et macOS par GitHub Actions (CLAUDE.md).

## V0.1 : Importer et trier

- [x] Projet Tauri 2 qui se compile et se lance sur Windows et macOS, avec intégration continue GitHub Actions sur les deux systèmes
- [x] Première ouverture guidée (bibliothèque, sauvegarde, rangement, identité)
- [ ] Une carte SD insérée est détectée en moins de 5 secondes et l'import proposé
- [x] Import de 500 RAW avec rangement, renommage, sauvegarde et vérification par somme de contrôle, sans aucune perte, en cas de coupure comprise (reprise propre au redémarrage)
- [x] Les photos déjà importées sont reconnues et grisées lors d'un second import
- [x] Aperçus affichés dès la fin de la copie de chaque fichier, sans attendre la fin de l'import
- [ ] Passage d'une photo à la suivante en moins de 100 ms dans la visionneuse
- [x] Notes, drapeaux, couleurs au clavier, écrits en base et dans le XMP
- [x] Comparaison côte à côte de 2 à 4 photos avec zoom synchronisé
- [x] Corbeille interne avec restauration et vidage après 30 jours
- [x] Vue Bibliothèque avec albums, filtres de base (date, note, drapeau, boîtier, objectif) et recherche
- [x] Bibliothèque sur disque externe débranché consultable via le cache
- [x] RAW testés au minimum : Fujifilm (RAF X-Trans), Canon (CR3), Nikon (NEF), Sony (ARW), Panasonic (RW2), DNG
- [x] Interface en français et en anglais

## V0.2 : Retoucher et livrer

- [x] Pipeline de développement GPU avec rendu neutre fidèle au boîtier par défaut
- [ ] Tous les réglages essentiels et intermédiaires de la section 4.5, avec retour visuel en moins de 100 ms
- [x] Corrections d'objectifs automatiques via lensfun
- [x] Recadrage, redressement, ratios
- [x] Correcteur de défauts (poussières, petites imperfections), corrections copiables entre photos
- [x] Presets fournis et personnels, intensité réglable, copier-coller et synchronisation des réglages
- [x] Historique, instantanés, copies virtuelles, avant/après
- [x] Retouches enregistrées dans le XMP et restaurées après reconstruction de la base
- [x] Les cinq presets d'export de la section 4.6, formats JPEG, HEIF, AVIF, TIFF, PNG
- [x] Filigrane texte et image, nettoyage des métadonnées, export multiple
- [ ] Export de 100 photos en JPEG 2048 px en moins de 2 minutes sur une machine de milieu de gamme

## V0.3 : Carte et matériel

- [x] Les quatre sources de localisation de la section 4.7, avec correction du fuseau horaire
- [x] Géocodage inverse hors ligne (pays, ville)
- [x] Carte du monde des albums avec grappes, aperçu et filtre par zone
- [x] Fond de carte simplifié hors ligne
- [x] Inventaire alimenté automatiquement, fusion des doublons, masquage
- [x] Fiches complètes, accessoires manuels, statistiques d'usage avec graphiques
- [x] Journal d'entretien et rappels
- [x] Export d'inventaire PDF et CSV
- [x] Albums intelligents

## Journal

### 2026-10-18

- Projet initialisé : Tauri 2.12, React 19, TypeScript strict, Vite, ESLint, Prettier, Vitest, i18next, Zustand. Barre d'étapes et barre latérale aux couleurs de la charte graphique (`src/theme.css`), icônes Lucide, logo et icône d'application.
- CI GitHub Actions (`.github/workflows/ci.yml`) sur `windows-latest` et `macos-latest` : format, lint, types, Vitest, cargo fmt, clippy, cargo test, compilation Tauri. Le premier critère sera coché quand la CI passera sur les deux systèmes. Mac Intel non couvert par la CI pour l'instant.
- Prototype LibRaw validé localement (Windows) sur les six formats : voir [décision 0001](decisions/0001-liaison-libraw.md). Le critère « RAW testés » reste ouvert tant que le décodage complet (pas seulement l'aperçu) et macOS ne sont pas vérifiés.
- Prototype XMP avec xmp-toolkit-rs : voir [décision 0002](decisions/0002-metadonnees-xmp.md). Nommage des annexes et espace de noms à valider.
- Schéma SQLite V0.1 proposé : [schema-v0.1.md](schema-v0.1.md), en attente de validation avant de coder l'import.

- CI verte sur Windows et macOS (run 39164993762) : compilation, 14 tests Rust sur les vrais RAW, interface. Le premier critère attend encore une vérification du lancement sur macOS.
- Interface refondue sur les codes de Lightroom et Capture One, à partir des exemples fournis : barre de menus native traduite (Fichier, Édition, Photo, Affichage, Aide ; menu Afterdusk sur macOS), sélecteur de modules en texte à droite de la barre supérieure, panneaux latéraux à sections repliables, barre d'outils (vues, drapeaux, étoiles, couleurs, tri), pellicule avec en-tête et filtre, barre d'état. Raccourcis F6, F7, F8 et Ctrl+Alt+1 à 7 comme dans Lightroom ; Tab reste réservé à la navigation au clavier. Pas de sélection de texte, de menu contextuel ni de raccourcis du navigateur.
- Écart avec la section 6 du cahier des charges : les vues Bibliothèque, Carte et Matériel passent de la barre latérale au sélecteur de modules, et Réglages au menu (Ctrl+,), comme dans les logiciels concurrents.

Tests en place à cette date : 14 tests Rust et 10 tests Vitest.

### 2026-10-18 (suite) : import

- Schéma SQLite V0.1 validé et appliqué (migration `0001_initial.sql`). Ajout d'une colonne `imports.options` (JSON) pour pouvoir reprendre un import avec ses options ; la migration n'étant pas encore publiée, elle a été complétée plutôt que doublée d'une migration 0002.
- Moteur d'import (`src-tauri/src/import/`) : analyse de la source en lecture seule, empreinte `taille|date|BLAKE3(64 Kio début + fin)`, rangement et renommage par modèles, copie vers un fichier temporaire relu et comparé par BLAKE3, sauvegarde vérifiée, XMP annexe (`IMG.xmp` pour les RAW, `IMG.JPG.xmp` sinon), journal par fichier et reprise.
- Écran Importer branché : choix du dossier (aussi par Fichier > Importer depuis un dossier…, Ctrl+Maj+O), grille par jour avec les déjà importées grisées, sélection par jour ou photo par photo, bibliothèque, album (un par jour en option), renommage, sauvegarde, copyright et mots-clés, progression et arrêt, reprise des imports interrompus.
- Tests : 7 scénarios d'import de bout en bout (vrais RAW, second import grisé, interruption et fichier temporaire tronqué, carte retirée, coupure entre copie et base, bibliothèque hors ligne, 500 fichiers interrompus puis repris sans perte), source vérifiée inchangée (contenu, date, liste des fichiers).
- Reste pour cocher les critères d'import : aperçus en cache dès la fin de chaque copie (miniatures réelles dans la grille), détection automatique des cartes, essai réel sur 500 RAW et vérification sur macOS.

### 2026-10-18 (suite) : aperçus

- Cache d'aperçus 256, 1024 et 2048 px dans le dossier de données de l'application (`previews/`), indexé dans `previews`. JPEG intégré décodé à taille réduite (mise à l'échelle DCT), orientation appliquée : 100 à 270 ms pour les trois paliers d'un RAW en release. Palier 2048 limité à la taille de l'aperçu intégré pour l'ARW (1616 px) et le RW2 (1920 px), en attendant le développement réduit.
- Génération dans des fils d'arrière-plan dès la fin de la copie de chaque fichier (événement `preview-ready`), rattrapage au démarrage des photos sans aperçu.
- Protocole `preview://` : aperçus du cache, et miniatures des fichiers de la source avant import (lecture seule, limitée au dossier analysé).
- Grille d'import avec vraies miniatures ; Trier et Bibliothèque affichent la grille, la pellicule, les albums et la fiche d'informations de la photo sélectionnée ; flèches gauche et droite pour passer d'une photo à l'autre.
- Non vérifié à la main dans l'application : un import réel avec affichage des miniatures.

### 2026-10-19 : accueil, démarrage et barre de titre

- Fenêtre d'accueil indépendante, ouverte au démarrage (option pour la désactiver) : salutation, activité récente, ajout de photos par dossier ou dépôt, reprise des imports interrompus, parcours en quatre étapes. Chaque action ouvre la fenêtre principale sur le bon module ; le logo et Fenêtre > Accueil (Ctrl+Alt+0) la rouvrent.
- Écran de démarrage : le symbole tourne, puis « Afterdusk » apparaît dans la police et la graisse du logo de la barre ; simple fondu si le système demande moins d'animations.
- Barre de titre intégrée, comme Capture One. Sous Windows : fenêtre sans cadre natif, menus dessinés dans la barre (Alt, flèches, Échap, sous-menus), raccourcis gérés par l'interface, boutons Réduire, Agrandir et Fermer. Sous macOS : barre de titre transparente avec les pastilles du système, barre de menus du système. Fenêtre principale d'au moins 1024 px de large.
- Contrepartie sous Windows : le menu de disposition de Windows 11 au survol d'Agrandir n'est plus proposé (le glisser vers les bords de l'écran reste possible).
- Correction : les connexions à la base ne rejouent plus la configuration WAL ni les migrations (faites une fois au lancement), ce qui provoquait par moments une erreur à l'ouverture de la fenêtre principale.
- Vérifié sous Windows sur l'application compilée (captures des deux fenêtres). Non vérifié sur un Mac réel : la barre de titre transparente et le menu système.

### 2026-10-19 (suite) : notes, drapeaux et couleurs

- Au clavier dans Trier et Bibliothèque : 0 à 5 pour la note, P, X et U pour les drapeaux, 6 à 9 pour les étiquettes rouge, jaune, vert et bleu (la même touche retire la couleur), flèches pour passer d'une photo à l'autre. Les chiffres sont lus sur la touche physique, donc aussi sur un clavier AZERTY. Avance automatique en option dans la barre d'outils. Menu Photo > Note, Drapeau, Étiquette de couleur actif.
- Écrit en base aussitôt (affichage immédiat, valeur rétablie si le cœur refuse), puis dans le XMP par un fil d'arrière-plan qui regroupe les frappes : seuls `xmp:Rating`, `xmp:Label` et `afterdusk:Pick` changent, le reste du fichier est conservé. Une modification faite pendant l'écriture reste en file ; une bibliothèque débranchée est écrite à son retour (nouvel essai chaque minute).
- Import : la note, le drapeau, la couleur et les mots-clés d'un XMP déjà présent (laissé par un autre logiciel) sont repris en base au lieu d'être remis à zéro.
- Vérifié de bout en bout dans l'application compilée sous Windows (bibliothèque d'essai de trois RAW importée avec `examples/dev_seed.rs`) : miniatures réelles, touches 4, P et 6, valeurs en base et dans le XMP sur le disque, original identique à la source.
- Critère « Notes, drapeaux, couleurs au clavier, écrits en base et dans le XMP » coché : CI verte sur Windows et macOS (run 39230735901).

### 2026-10-19 (suite) : accueil épuré et couvertures d'album

- Accueil épuré, avec une composition propre à Afterdusk plutôt qu'une reprise d'Affinity : la photo d'illustration sert d'en-tête (barre de titre par-dessus, salutation et bibliothèque courante en bas à gauche, actions Bibliothèque, Dossier… et Importer en bas à droite), puis deux colonnes : planche contact du dernier import et liste des albums avec leur couverture. Pas d'onglets. La colonne de gauche, le texte d'introduction et les cartes du parcours sont retirés. La bibliothèque choisie est partagée avec la fenêtre principale.
- Photo d'illustration, comme les écrans de démarrage d'Adobe : tirée au hasard parmi les meilleures photos de la bibliothèque (retenues et mieux notées d'abord, rejetées exclues), avec l'album et la date en légende ; photo par défaut tant que la bibliothèque n'a rien à mettre en avant : coucher de soleil à Cabourg, © Lewis Bedar (recadrée en 2560 × 853 px, 250 Ko, sans métadonnées ; l'original avec ses données GPS n'est pas versionné).
- Couverture d'album : première photo par défaut, ou celle choisie par Photo > Définir comme couverture de l'album ou par clic droit sur la miniature ; badge sur la photo de couverture.
- Limite connue : la couverture choisie n'est enregistrée qu'en base ; elle serait perdue si la base devait être reconstruite à partir des dossiers et des XMP.

### 2026-10-19 (suite) : visionneuse

- Visionneuse dans Trier et Bibliothèque : Entrée ou double-clic pour l'ouvrir, G pour revenir à la grille, flèches pour naviguer, F pour le plein écran (panneaux et barres masqués, fenêtre en plein écran), Échap pour sortir (plein écran, puis zoom, puis visionneuse). E reste réservé à Retoucher (§6).
- Aperçu 2048 px ajusté ; les deux photos de part et d'autre sont décodées d'avance et l'image affichée ne change qu'une fois la suivante prête (jamais d'écran vide).
- Zoom 100 % au clic sur le point visé ou avec Espace ; glisser pour se déplacer ; un clic revient à l'ajusté. Le zoom utilise le JPEG pleine taille intégré au RAW (`preview://…/media/<id>/full`, original en lecture seule), orienté par l'interface, et tient compte de la densité de l'écran ; aperçu agrandi si la bibliothèque est débranchée.
- Mesure dans l'application compilée sous Windows (compilation de débogage, six RAW de formats différents, mesure de la frappe jusqu'à deux images d'écran après l'affichage, donc majorée) : p95 de 21 ms au rythme d'une photo toutes les 400 ms, 20 ms toutes les 120 ms, sur 20 passages chacun. Critère « moins de 100 ms » tenu sous Windows ; à mesurer sur un Mac avant de le cocher (la CI ne peut pas mesurer un affichage).
- Mesures affichées seulement dans une compilation d'essai (`VITE_PERF_OVERLAY=1`).

### 2026-10-19 (suite) : comparaison

- Sélection multiple dans la grille et la pellicule : Ctrl+clic (ajouter, retirer), Maj+clic (plage). La photo active (cadre bleu) reste distincte de la sélection ; dans la grille, notes, drapeaux et couleurs s'appliquent à toute la sélection.
- Comparaison (C) de 2 à 4 photos de la sélection, ou de la photo active et de la suivante : 2 ou 3 en colonnes, 4 en carré. Zoom et déplacement synchronisés (clic, Espace, glisser) ; le panneau actif reçoit notes et drapeaux, les flèches passent d'un panneau à l'autre, Entrée l'ouvre en visionneuse, C, G ou Échap reviennent à la grille.
- Zoom : quand le JPEG intégré est nettement plus petit que le capteur (ARW 1616 px, RW2 1920 px, RAF 4416 px pour 6240), il est agrandi aux dimensions du capteur et signalé « Aperçu agrandi », pour garder la même échelle entre boîtiers ; « 100 % » seulement quand il couvre le capteur.
- Composant `PhotoPane` partagé par la visionneuse et la comparaison (et bientôt le second écran).
- Vérifié dans l'application compilée sous Windows : 4 photos sélectionnées par clic puis Maj+clic, comparaison, zoom synchronisé à 100 %.
- Critère « Comparaison côte à côte de 2 à 4 photos avec zoom synchronisé » coché : CI verte sur Windows et macOS (run 39240754465).

### 2026-10-19 (suite) : second écran

Ajout hors du périmètre initial de la V0.1, à la demande de l'utilisateur (équivalent de l'affichage secondaire de Lightroom).

- Fenêtre > Second écran (Ctrl+F11, ⌘+F11 sur Mac) : une fenêtre sans barres, en plein écran sur le plus grand des autres moniteurs ; avec un seul écran, une fenêtre normale. Le même raccourci, Échap ou le bouton du bandeau la ferment, sans fermer l'application.
- Elle suit la fenêtre principale (photo active, sélection, marques, aperçus) : visionneuse ou comparaison au choix dans un bandeau qui n'apparaît qu'au survol ; le choix est mémorisé. Zoom propre au second écran. Le clavier reste à la fenêtre principale.
- La fenêtre principale n'envoie que la photo active, la comparaison et les voisines (préchargement), au plus une fois par image d'animation.
- Correction : la commande de création de fenêtre est asynchrone (une commande synchrone bloquait les échanges avec le cœur sous Windows) ; le badge de couverture ne marque plus que les couvertures choisies.
- Vérifié sous Windows avec deux écrans de 1920 × 1080 : ouverture en plein écran sur le second moniteur, suivi de la navigation. Non vérifié sur Mac.

### 2026-10-19 (suite) : corbeille interne

- Photo > Envoyer dans la corbeille (Suppr, ⌘⌫ sur Mac, ou clic droit) : la sélection de la grille, ou la photo affichée. Le fichier et son XMP sont déplacés dans `.afterdusk-corbeille/<id>/` à la racine de la bibliothèque (même disque, donc un simple renommage, jamais de copie ni d'écrasement). Les notes en attente sont écrites dans le XMP avant le déplacement.
- Annonce « Photo envoyée dans la corbeille » avec Annuler ; Ctrl+Z (⌘Z) restaure le dernier envoi, même après la disparition de l'annonce.
- Photo > Envoyer les rejetées dans la corbeille… (Ctrl+Retour arrière, ⌘⌥⌫ sur Mac) : les rejetées de la vue affichée, après confirmation.
- Catalogue > Corbeille : nombre de photos, jours restants sur chaque miniature, Restaurer (à l'emplacement d'origine ; suffixe -2 si un fichier y est arrivé entre-temps), Supprimer définitivement… et Vider la corbeille…, toujours avec confirmation. Pas de tri dans la corbeille.
- Vidage automatique au démarrage, en arrière-plan, des photos jetées depuis plus longtemps que le délai (30 jours par défaut, réglable de 1 à 365 dans Réglages > Corbeille et cache). Une bibliothèque débranchée est ignorée ; la suppression ne touche que des fichiers situés dans la corbeille (garde-fou), jamais la sauvegarde.
- Migration 0002 : l'index de recherche est recréé avec `contentless_delete` (une table FTS5 sans contenu refusait la suppression d'une fiche) et rempli à nouveau depuis la base.
- Tests : déplacement et restauration à l'identique (empreinte, XMP), restauration sans écraser, vidage limité à la corbeille avec la sauvegarde intacte, purge après le délai, bibliothèque débranchée ; côté interface, envoi, annulation, rejetées, corbeille sans tri, jours restants.
- Vérifié dans l'application compilée sous Windows, sur une bibliothèque de contrôle (les six RAW de test-assets, base de l'utilisateur mise de côté puis remise) : Suppr, rejetées avec confirmation, vue Corbeille, Restaurer, Vider.
- Critère « Corbeille interne avec restauration et vidage après 30 jours » coché : CI verte sur Windows et macOS (run 39245921321).

### 2026-10-19 (suite) : filtres, recherche et vue Albums

- Barre de filtres au-dessus de la grille, dans Bibliothèque et Trier (Affichage > Barre de filtres, ou \) : recherche, période (du… au…), note (« ★★★ et plus », 5 étoiles, non notées), drapeaux (retenues, sans drapeau, rejetées, cumulables), couleurs, boîtier et objectif (avec le nombre de photos), Réinitialiser. Les filtres se combinent ; la barre d'état affiche « 12 sur 340 photos ».
- Recherche (Ctrl+F, ⌘F) sur les albums, mots-clés, lieux et noms de fichiers, sans tenir compte des accents ni de la casse, par l'index FTS5 ; une partie de nom de fichier (« 7m3 ») est trouvée aussi. Le texte saisi n'est jamais interprété : pas d'erreur sur les guillemets ou les caractères spéciaux.
- Le filtrage est fait par le cœur en SQL. Une note ou un drapeau posé pendant le tri ne fait pas disparaître la photo de la grille : la liste se met à jour au prochain changement de filtre.
- Tri de la grille opérationnel : date de prise de vue, nom de fichier, note.
- Vue Albums dans Bibliothèque (bouton à gauche des vues) : couvertures, dates, lieu, nombre de photos ; tri par date, nom ou lieu ; la recherche porte alors sur les albums ; un clic ouvre l'album.
- Tests : filtres combinés, boîtier, objectif, période, recherche (accents, morceau de nom, caractères spéciaux), tri et corbeille sur les six RAW de test-assets ; côté interface, transmission du filtre, champs vidés, recherche et tri des albums.
- Vérifié dans l'application compilée sous Windows (bibliothèque de contrôle de deux albums, base de l'utilisateur mise de côté puis remise) : vue Albums, note, retenues, recherche, boîtier, tri par note.
- Critère « Vue Bibliothèque avec albums, filtres de base et recherche » coché : CI verte sur Windows et macOS (run 39249029586).

### 2026-10-19 (suite) : piles RAW + JPEG

Ajout hors du périmètre initial de la V0.1, à la demande de l'utilisateur ; choix consignés dans la décision 0003.

- Un RAW et son JPEG (ou HEIF) du même déclenchement forment une pile : même dossier, même nom sans extension (casse ignorée), prises de vue à 2 secondes près. Repliée, la pile occupe une case avec une pastille « 2 » et compte pour une photo ; S ou un clic sur la pastille la déplie ; Photo > « Mettre en haut de la pile » choisit le fichier affiché.
- Les marques appartiennent au déclenchement : elles valent pour les deux fichiers et sont écrites dans les deux XMP.
- Corbeille au choix : Suppr sur une pile repliée envoie les deux fichiers, sur une pile dépliée le fichier choisi ; Photo > « Envoyer le JPEG / le RAW dans la corbeille » garde l'autre. Le fichier resté prend la place du dessus ; un fichier restauré retrouve sa pile ; Ctrl+Z restaure le fichier jeté.
- Import : une case par paire sur la source ; option mémorisée « Importer aussi le JPEG des paires » ; un seul numéro `{seq}` et un même nom (y compris le suffixe -2) pour les deux fichiers, calculé sur le RAW.
- Aperçus et zoom 100 % d'un RAW tirés du JPEG du boîtier quand l'aperçu intégré est plus petit (Sony 1616 px, Panasonic 1920 px).
- Migration 0003 : colonne `companion_of` et paires déjà importées reliées (aucun fichier touché).
- Tests : reconnaissance (casse, heures écartées, autre dossier, HEIF), import, renommage, option sans JPEG, marques dans les deux XMP, corbeille au choix du format avec empreintes inchangées, pile entière, aperçus, migration ; côté interface, S, marques sur la pile, Suppr pile repliée ou dépliée, annulation.
- Vérifié dans l'application compilée sous Windows (carte de contrôle : ARW et RW2 avec un JPEG de boîtier, base de l'utilisateur mise de côté puis remise) : pastille, dépliage, note partagée, mise en haut de la pile, envoi du RAW seul. L'annulation de cet envoi restaurait la photo sélectionnée au lieu du RAW : corrigé (le cœur renvoie les fichiers déplacés), couvert par les tests, non revérifié dans l'application. L'écran d'import avec paires n'a pas été vérifié à l'écran (couvert par les tests).

### 2026-10-19 (suite) : première ouverture guidée

- Sans bibliothèque, l'accueil présente un assistant en quatre écrans (§6 du cahier des charges), chacun avec une valeur par défaut :
  1. Bibliothèque : `Images/Afterdusk`, créé à la fin, nom « Photos ».
  2. Sauvegarde : facultative, avec un avertissement si elle est sur le disque de la bibliothèque.
  3. Rangement : année / date et album, année / mois / jour, année / album ou modèle personnalisé ; nom du boîtier gardé ou date, heure, boîtier et numéro ; exemple du résultat calculé par le cœur.
  4. Identité : nom, copyright proposé à partir du nom (« © 2026 Prénom Nom »), site.
- L'identité est enregistrée en base (réglage `identity`) : le nom signe les XMP de chaque import (`dc:creator`) et le copyright est proposé dans le panneau d'import.
- Réglages complétés : Bibliothèques (emplacement, sauvegarde à choisir ou retirer), Import (rangement et renommage avec exemple, valables pour les prochains imports), Identité.
- Correction : la phrase « Ajouter des photos » de l'accueil se terminait par « ou » depuis la refonte de l'accueil.
- Tests : identité dans les XMP, sauvegarde refusée si le dossier manque, modèles validés et aperçu, même disque ; côté interface, assistant (étapes, enchaînement des commandes, copyright qui suit le nom), réglages Bibliothèques et Import.
- Vérifié dans l'application compilée sous Windows, sur une base vide (celle de l'utilisateur mise de côté puis remise) : les quatre écrans, choix d'un rangement et d'un renommage, création de la bibliothèque et de son dossier, retour à l'accueil habituel. Le dossier `Images/Afterdusk` créé par l'essai, vide, a été retiré. La saisie de l'identité et le choix d'un dossier par le sélecteur natif n'ont pas été pilotés à l'écran (couverts par les tests).
- Critère « Première ouverture guidée » coché : CI verte sur Windows et macOS (run 39258437611).

### 2026-10-19 (suite) : détection des cartes

- Le cœur relève les volumes chaque seconde, en arrière-plan : tout volume dont la racine contient un dossier `DCIM` est une carte ou un boîtier. Sous Windows, seuls les lecteurs locaux présents sont sondés (amovibles, fixes, virtuels) ; les lecteurs réseau, qui peuvent bloquer, et les lecteurs optiques sont écartés. Sous macOS, `/Volumes` ; sous Linux, `/media` et `/run/media`. La détection ne fait que lire.
- Marque devinée d'après les dossiers du DCIM (`100_FUJI` → « Carte FUJI (E:) ») ; les suffixes génériques (`100MSDCF` de Sony) donnent « Carte (E:) ».
- À l'insertion (réglage Import > « À l'insertion d'une carte ») :
  - proposer l'import (par défaut) : bandeau « Carte FUJI (E:) détectée · Importer » dans la fenêtre affichée, qui clignote dans la barre des tâches si elle n'a pas le focus ;
  - ouvrir l'import : Importer s'affiche avec la carte analysée, sauf pendant un import en cours (proposé seulement) ;
  - ne rien faire.
- Importer liste les cartes présentes dans « Cartes et boîtiers » ; l'historique des imports retient le nom de la carte.
- Corrections : le chemin d'une racine de lecteur s'affichait « \:X » (affichage de droite à gauche) ; le bandeau restait affiché une fois la carte ouverte dans Importer.
- Mesure dans l'application compilée sous Windows, avec un lecteur virtuel (`subst X:` vers un dossier contenant `DCIM/100_FUJI`), base de l'utilisateur mise de côté puis remise : bandeau affiché 330, 1 074 et 836 ms après l'apparition du lecteur ; retrait relevé en moins de 0,5 s. « Importer » ouvre la fenêtre principale sur la carte analysée.
- Mesure avec une vraie carte SD (Sony, exFAT, lecteur amovible `I:`), sur la bibliothèque de l'utilisateur : bandeau affiché 372 ms après le montage de la carte par Windows. La carte n'est que lue. Son dossier `DCIM/100MSDCF`, propre aux boîtiers Sony, était tenu pour générique (« Carte (I:) ») : il donne désormais « Carte SONY (I:) ».
- Critère « carte détectée en moins de 5 secondes » non coché : tenu sous Windows, reste à vérifier sur un Mac. Les deux corrections ci-dessus n'ont pas été revérifiées à l'écran (couvertes par les tests pour le bandeau).

### 2026-10-19 (suite) : illustration de la première ouverture

- L'espace à droite de l'assistant porte une icône par étape (photos, disque, arborescence, personne) sur un halo discret, qui change avec l'étape ; masquée quand la fenêtre est étroite. Un récapitulatif des choix a été essayé à cette place puis retiré : il alourdissait la fenêtre.
- Vérifié dans l'application compilée sous Windows (base vide, celle de l'utilisateur mise de côté puis remise) : les quatre étapes. L'alignement de l'icône à hauteur du titre, ajusté ensuite, n'a pas été revérifié à l'écran.

### 2026-10-19 (suite) : retours de l'essai sur une vraie carte Sony

- Sur une carte ou un boîtier (dossier `DCIM` à la racine), seul `DCIM` est analysé (norme DCF). Les vignettes JPEG que les boîtiers Sony écrivent pour leurs vidéos (`PRIVATE/M4ROOT/THMBNL/C0002T01.JPG`) apparaissaient comme des photos ; les dossiers `THMBNL` sont aussi ignorés dans un dossier quelconque.
- Les photos en hauteur étiraient leur case (et toute la rangée) dans la grille d'import : toutes les miniatures s'inscrivent désormais dans un cadre 3:2, photo entière.
- Revérifié dans l'application compilée sur la carte de l'utilisateur : « Carte SONY (I:) », photos en hauteur inscrites dans le cadre, paires reconnues, 375 déclenchements proposés au lieu de 439 (64 vignettes de vidéos écartées).

### 2026-10-19 (suite) : assistant sans défilement

- Aucune étape de la première ouverture ne défile, photo d'accueil à pleine hauteur (fenêtre par défaut, 1040 × 700) : « Dossiers » et « Noms de fichiers » côte à côte à l'étape Rangement, libellés raccourcis (l'exemple donne le résultat), modèle personnalisé saisi sur la ligne de « Personnalisé » avec les jetons en infobulle, note de l'identité retirée de l'assistant (elle reste dans les Réglages), marges resserrées. Les boutons de l'en-tête, grisés sans bibliothèque, sont masqués.
- Correction : choisir « Personnalisé » cochait « Année / album » (le modèle de départ était identique) ; le mode personnalisé est désormais un choix à part, qui part du modèle en cours.
- Vérifié dans l'application compilée sous Windows (base vide, celle de l'utilisateur mise de côté puis remise) : les quatre étapes, et l'étape 3 en mode personnalisé, sans barre de défilement.
- Descriptions des étapes sur une ligne : largeur du formulaire au lieu de 60 caractères, textes resserrés (110 caractères au plus en français). Le retour à la ligne reste possible en fenêtre étroite. Vérifié à l'écran sur les quatre étapes.
- Précédent et Suivant restent au même endroit d'une étape à l'autre : l'assistant occupe toute la hauteur sous la photo, le pied en bas. Vérifié à l'écran (étapes 1 et 3).

### 2026-10-19 (suite) : sauvegarde dans un dossier cloud

- Les dossiers synchronisés par OneDrive (personnel et entreprise), Dropbox (lu dans son `info.json`), Google Drive, iCloud Drive, Box, Synology Drive, Nextcloud et pCloud sont détectés (variables d'environnement, dossiers personnels, `~/Library/CloudStorage` sur Mac) et proposés en un clic comme sauvegarde, dans un sous-dossier `Afterdusk`, à la première ouverture et dans Réglages > Bibliothèques.
- Pour une sauvegarde dans un dossier cloud, l'avertissement « même disque » est remplacé par « Copie envoyée en ligne par OneDrive » : Afterdusk vérifie la copie locale ; l'envoi et l'espace disponible relèvent du service. Afterdusk ne fait aucun appel réseau (règle absolue) : c'est le logiciel du service qui envoie.
- Tests : détection (OneDrive par variable, Dropbox par `info.json`, dossiers personnels, sans doublon), dossiers `CloudStorage` du Mac, reconnaissance d'une sauvegarde dans un dossier cloud.
- Vérifié dans l'application compilée sous Windows avec le OneDrive de l'utilisateur (base vide, la sienne mise de côté puis remise ; rien créé dans OneDrive). Non vérifié sur Mac.

### 2026-10-19 (suite) : preuves des critères déjà remplis

- Essai de lancement en CI sur Windows et macOS : l'application compilée démarre avec `AFTERDUSK_SMOKE=1`, la première fenêtre affiche l'interface et le signale au cœur, qui arrête l'application avec succès ; au-delà de 90 s, l'essai échoue. En local sous Windows : interface affichée en 0,74 s.
- Test d'import resserré : chaque photo est annoncée (et ses aperçus demandés) dès la fin de sa copie, un événement par fichier, dans l'ordre, avant la fin de l'import.
- Inventaire des preuves déjà en place : six formats RAW lus en CI (`raw_samples`, échantillons de test-assets) ; second import reconnu par empreinte et photos grisées, non sélectionnables (cœur et interface) ; fr.json et en.json aux mêmes clés, menus vérifiés dans les deux langues.
- Critères cochés sur ces preuves, CI verte sur Windows et macOS avec l'essai de lancement (« interface affichée » dans les journaux des deux systèmes, run 39278550831) : projet Tauri 2 compilé et lancé, photos déjà importées grisées, aperçus dès la fin de chaque copie, six formats RAW, interface en français et en anglais.

### 2026-10-19 (suite) : bibliothèque sur disque débranché

- Le cœur vérifie toutes les 2 s le disque de chaque bibliothèque et note sa présence (`is_online`, `last_seen_at`). Débranchement ou retour : l'interface est prévenue (`library-status`) et se met à jour. Au retour du disque, les XMP en attente sont écrits et les aperçus manquants calculés, sans rien demander.
- Hors ligne, la bibliothèque reste consultable : grille, albums, filtres, fiche et aperçus en cache ; les notes, drapeaux et couleurs sont posés en base et leurs XMP attendent le disque. Bandeau « « Disque externe » est hors ligne. Rebranchez le disque (X:\Photos) pour importer, supprimer ou zoomer à 100 %. Vous pouvez parcourir et noter vos photos. 1 note sera écrite dans son XMP au retour du disque. Vu pour la dernière fois le … », dans la fenêtre principale et l'accueil ; « … est de nouveau disponible » à son retour. Import désactivé tant que le disque manque.
- Test d'intégration : bibliothèque débranchée parcourue (liste, aperçus en cache, fiche, pas de zoom 100 %), note posée et XMP en attente, corbeille et import refusés, puis disque rebranché : XMP écrit, originaux identiques (empreintes).
- Vérifié dans l'application compilée sous Windows, avec une bibliothèque sur un lecteur virtuel `X:` (base de l'utilisateur mise de côté puis remise) : bandeau moins de 4 s après le débranchement, miniatures toujours affichées, note de 5 étoiles posée hors ligne ; au rebranchement, message de retour et XMP écrit 3,4 s plus tard.
- Critère « Bibliothèque sur disque externe débranché consultable via le cache » coché : CI verte sur Windows et macOS (run 39285661736).

### 2026-10-19 (suite) : boîte « À propos »

- Aide > À propos d'Afterdusk (menu Afterdusk sur Mac, à la place du panneau standard du système) ouvre une boîte dessinée par l'application, sur le modèle des « À propos » des logiciels de bureau : bande photo à gauche (Cabourg, cadrée sur les deux silhouettes) avec le logo posé dessus ; à droite la version, le copyright, les composants et leur licence (LibRaw LGPL 2.1, XMP Toolkit BSD 3, SQLite domaine public, Tauri MIT / Apache 2.0, React MIT, Lucide ISC), le crédit de la photo et l'avis de licence GPL v3 ; boutons OK, Licence… (texte complet de la GPL) et Informations système… (version, système, WebView, composants copiés dans le presse-papiers pour un signalement).
- Vérifié dans l'application compilée sous Windows : ouverture depuis le menu Aide, licence, copie des informations système. Non vérifié sur Mac.

### 2026-10-19 (suite) : import de 500 RAW avec coupure

- Test d'intégration `import_500` (CI, Windows et macOS) : 500 fichiers RAW de 1 Mo aux six extensions, contenus tous différents, sur trois jours ; import avec rangement par jour, renommage `AAAAMMJJ_HHMMSS_boîtier_NNNN` et sauvegarde. L'import tourne dans un processus séparé, **tué net** une fois le tiers passé (comme une coupure de courant), puis l'application « redémarre » : l'import est proposé à la reprise et repris. Vérifié : 500 photos en base, chaque copie et chaque sauvegarde identiques à l'original (BLAKE3), numéros 0001 à 0500 sans trou ni doublon, un album par jour, aucun fichier `.afterdusk-part` laissé, carte intacte (empreintes et dates). En local sous Windows : coupure après 166 fichiers, 22 s au total.
- Essai réel sous Windows, application compilée, base de l'utilisateur mise de côté puis remise : 500 vrais RAW (CR3, ARW et DNG des échantillons, rendus distincts par quelques octets ajoutés à la fin), 13 Go, sur une carte montée en lecteur `X:` et détectée ; import depuis l'interface avec rangement par jour, renommage et sauvegarde. Application tuée net (`taskkill /F`) après 150 photos : un fichier copié dont la sauvegarde était à moitié écrite (`.afterdusk-part`), 349 en attente. Au redémarrage, l'accueil propose « Reprendre » ; reprise terminée 25 s plus tard. Contrôle indépendant (SHA-256) : 500 photos en base, 500 noms distincts, chaque copie et chaque sauvegarde identiques à l'original, aucun fichier temporaire restant, carte identique (contenus et dates).
- « À propos » allégée : les composants tiennent en un paragraphe discret (« Composants : LibRaw 0.22.2 (LGPL 2.1) · … »), la boîte prend la hauteur de son contenu ; la licence s'affiche dans un cadre de hauteur fixe. Vérifié à l'écran sous Windows.
- Critère « Import de 500 RAW … coupure comprise » coché : test `import_500` vert sur Windows et macOS (run 39289840424), essai réel de 500 vrais RAW sous Windows.

### 2026-10-19 (suite) : vérifications sur Mac reportées

- Les deux critères restants de la V0.1 (carte détectée en moins de 5 s, passage d'une photo à la suivante en moins de 100 ms) ne demandent plus qu'une mesure sur un Mac. L'utilisateur y a accès et préfère la faire plus tard, une fois le développement bien avancé ; les deux critères restent ouverts d'ici là.

### 2026-10-19 (suite) : V0.2, première étape de la retouche (décision 0005)

- Format des réglages : JSON creux par sections, `version` migrée à la lecture et `process` figé ; valeurs ramenées dans leurs plages. Migration 0004 (`edits`, `edit_history`, `presets`). Réglages écrits dans le XMP (`afterdusk:Develop`) par le fil d'écriture des marques, retirés au retour au neutre.
- Décodage complet par LibRaw (XYZ linéaire 16 bits, balance et matrice du boîtier, AHD) ; pipeline de référence au processeur en Rec. 2020 linéaire flottant : exposition, courbe neutre à épaule, sRGB.
- Processus 1 calibré sur les six formats : à +1 IL de base, la luminance moyenne du rendu neutre égale celle du JPEG du boîtier (rapports de 0,88 à 1,20, moyenne géométrique 1,00). Orientation conforme, originaux inchangés.
- Jumeau GPU (wgpu 30, WGSL ; Direct3D 12, Metal, Vulkan) : écart d'au plus un niveau sur 255 avec le processeur sur un dégradé et les six formats. Sans processeur graphique, rendu au processeur (`AFTERDUSK_GPU=0` pour le forcer).
- Écran Retoucher : photo de la pellicule développée depuis l'original, curseur d'exposition (double-clic : zéro), balance des blancs « Telle que prise », enregistrement 600 ms après le dernier geste ou au changement de photo ; durée du rendu affichée dans la barre d'outils.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises), RTX 3060 Ti, aperçu de 1506 × 1006 : rendu 2,5 à 8 ms au GPU et 9 ms au processeur ; image affichée 44 à 51 ms après le geste, transfert et dessin compris ; original décodé en 160 à 550 ms à l'ouverture. Réglage retrouvé en base, dans l'historique et dans le XMP ; originaux identiques (SHA-256). La cible de 100 ms est à mesurer sur une carte graphique intégrée.
- Reste pour ce critère et les suivants : l'aperçu est réduit par blocs entiers (1500 px au lieu de 2048 pour un capteur de 24 Mpx), retouche des JPEG, HEIF et TIFF, restauration des retouches depuis le XMP après reconstruction de la base.

### 2026-10-19 (suite) : V0.2, réglages essentiels

- Balance des blancs complète : « Telle que prise » (température et teinte du boîtier, lues dans ses multiplicateurs : environ 5000 K pour les scènes de jour des échantillons, 3050 K pour la scène d'intérieur du Nikon), six préréglages, température (curseur régulier en mireds) et teinte, pipette. Changement de balance par adaptation de Bradford depuis le blanc de la prise de vue.
- Lumière : exposition, luminosité (nouveau curseur, tons moyens), contraste, hautes lumières, ombres, blancs, noirs ; tonalité appliquée sur la luminance perçue, teinte gardée ; chaque curseur garde la courbe croissante, même à fond (test). Couleur : vibrance et saturation.
- Histogramme rouge, vert, bleu de l'image rendue, avec indicateurs d'écrêtage allumés quand des pixels sont bouchés ou brûlés ; un clic les montre sur la photo (rouge pour les hautes lumières, bleu pour les ombres).
- Aperçu de 2048 px réels (moyenne de surface, bords fractionnaires), au lieu de 1500 px.
- Chaque curseur a sa propre étape d'historique ; réglages à zéro identiques au rendu neutre calibré.
- Jumeau GPU : au plus un niveau d'écart avec le processeur sur les six formats et trois jeux de réglages ; histogrammes concordants.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises), RTX 3060 Ti, aperçu de 2048 × 1368 : rendu de 4 à 13 ms au GPU (11 à 33 ms au processeur), image affichée 78 à 87 ms après le geste. L'essentiel de ce délai est le transfert des 11 Mo de pixels vers l'interface et leur dessin : c'est le premier point à améliorer pour tenir les 100 ms sur une carte graphique intégrée (envoyer l'image à la taille de l'écran, ou l'afficher directement depuis le GPU). Originaux identiques (SHA-256), réglages retrouvés dans le XMP.

### 2026-10-19 (suite) : V0.2, réglages automatiques

- Bouton Auto de la section Lumière, auto d'un seul curseur (Maj + double-clic ; la luminosité n'en a pas) et balance des blancs automatique (choix « Automatique », ou Maj + double-clic sur la température ou la teinte). Calcul local et déterministe dans le cœur, résultat en valeurs ordinaires, une étape d'historique (« light.auto », « color.whiteBalance.auto »).
- Tonalité : vignette de 512 px rendue par le pipeline lui-même, curseurs cherchés par dichotomie sur l'histogramme de l'image rendue (médiane au gris moyen, au plus 1 % de blanc brûlé, ombres, blancs et noirs bornés à ±50). Scène très contrastée : l'exposition baisse d'au plus 1 IL pour le ciel, les ombres relèvent le reste.
- Balance : monde gris sur les tons moyens proches du neutre du boîtier, affiné sur les pixels rendus neutres ; une estimation trop teintée pour être une lumière réelle (feuillage) ou sans gris crédible garde la balance du boîtier.
- Six formats : 50 à 150 ms pour la lumière, 20 ms pour la balance ; médiane de 84 à 110, moins de 2,1 % de blanc brûlé ; balance retrouvée sur la scène d'intérieur du Nikon (2540 K), gardée sur le feuillage du Panasonic. Vérifié à l'écran sous Windows (données de l'utilisateur mises de côté puis remises).
- Intégration Mac : l'histogramme du processeur graphique virtuel d'Apple était faux (pixels justes) ; il est vérifié et recalculé au processeur si besoin, les avertissements d'écrêtage sont peints après le rendu.

### 2026-10-19 (suite) : V0.2, historique et avant/après

- Panneau Historique dans le Retoucher (à gauche) : chaque traitement en clair (« Contraste +57 », « Lumière automatique », « Balance des blancs : Tungstène »), du plus récent au plus ancien, puis « Original » ; un clic revient à cet état, les étapes annulées restent grisées jusqu'au réglage suivant, qui les remplace.
- Annuler et rétablir : boutons du panneau, Édition > Annuler / Rétablir, Ctrl+Z et Ctrl+Maj+Z (Cmd sur Mac). Un réglage pas encore enregistré devient une étape avant d'être annulé.
- Migration 0005 : position courante de l'historique (`edits.history_position`) ; revenir à une étape réécrit les réglages et le XMP (sans réglages pour l'original).
- Avant/après : bouton à maintenir pour voir l'original, touche « \ » (AltGr+8 en AZERTY) pour basculer, vue scindée (original à gauche, retouche à droite) ; l'original est rendu une fois par photo.
- Corrigé : les raccourcis des menus dessinés (Windows) reconnaissaient la position de la touche et non la lettre ; en AZERTY, Ctrl+Z ne répondait pas (y compris pour annuler un envoi à la corbeille).
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : Auto puis contraste, Ctrl+Z, clic sur « Original », vue scindée, historique retrouvé après redémarrage, XMP sans réglages à l'original, originaux identiques. Non vérifié à l'écran : la touche « \ » et le bouton maintenu (couverts par les tests du magasin).
- Intégration Mac : luminance de l'histogramme calculée en entiers, identique sur tous les processeurs graphiques.

### 2026-10-19 (suite) : V0.2, instantanés et copies virtuelles

- Intégration : l'historique et les correctifs Mac (histogramme vérifié, luminance en entiers) sont verts sur Windows et macOS (run 39318908279).
- Instantanés : panneau du Retoucher, « + » garde les réglages affichés sous un nom libre (« Instantané 1 »…), un clic les applique (une étape d'historique, annulable), double-clic pour renommer, corbeille au survol (avec confirmation). Ils ont leur table (migration 0006) et survivent aux étapes d'historique remplacées après une annulation ; la colonne `snapshot_name` prévue en 0004, jamais utilisée, est retirée.
- Copies virtuelles : panneau Versions (« Originale » puis les copies), « + » crée une copie des réglages affichés et l'affiche ; chaque version a ses réglages, son historique (qui commence par « Copie virtuelle créée ») et ses instantanés ; renommer, supprimer (confirmation, aucun fichier touché).
- XMP : `afterdusk:Snapshots` (instantanés de l'originale) et `afterdusk:Versions` (copies, leurs réglages et instantanés), absents quand ils sont vides, pour une restauration après reconstruction de la base.
- Corrigé : une retouche enregistrée pendant l'écriture du XMP pouvait être retirée de la file sans être écrite (seules la note, le drapeau et la couleur étaient comparés). `media.xmp_pending` devient un compteur de modifications ; le fil d'écriture ne retire la photo que si le compteur n'a pas bougé.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : instantané, copie virtuelle retouchée à part, retour à l'originale, application de l'instantané, XMP complet, originaux identiques. Non vérifié à l'écran : renommer par double-clic et supprimer (couverts par les tests du cœur).
- Reste : les copies virtuelles n'apparaissent pas encore comme vignettes séparées dans Trier et Bibliothèque ; elles viendront avec les aperçus rendus d'après les retouches (étape de l'export).

### 2026-10-19 (suite) : V0.2, réglages intermédiaires (1/3) : courbes, TSL, effets

- Instantanés et copies virtuelles verts sur Windows et macOS.
- Courbe de tonalité par points (RVB, rouge, vert, bleu) : clic pour ajouter un point, glisser pour le déplacer, double-clic pour le retirer, histogramme du canal en fond. Interpolation cubique monotone (Fritsch–Carlson), tables de 1024 entrées lues de la même façon par le processeur et le GPU.
- Couleurs par teinte (TSL) : teinte, saturation et luminance sur huit couleurs, interpolées entre les centres voisins avec un plateau autour de chaque centre ; les gris ne bougent pas.
- Effets : vignetage (intensité, point milieu, progressivité) et grain (quantité, taille, irrégularité), grain par hachage entier placé en pixels de l'original.
- Format : sections `curve`, `hsl`, `effects`, JSON creux toujours en version 1 ; rendu neutre inchangé (six formats, calibration du processus 1).
- Jumeau GPU : au plus un niveau d'écart sur les six formats avec tous ces réglages ; rendu 3 à 10 ms au GPU, 30 à 40 ms au processeur à 2048 px.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : courbe en S et courbe bleue, vignetage, saturation du vert et du jaune, historique en clair, XMP complet, originaux identiques. Le grain, défini en pixels de l'original, est presque invisible dans l'aperçu ajusté à l'écran : il se jugera au zoom 100 % (sous-étape C).
- Reste pour ce critère : netteté et réduction du bruit (sous-étape B), zoom 100 % dans le Retoucher (sous-étape C).

### 2026-10-20 : V0.2, zoom 100 % dans le Retoucher

- Essai sous macOS par l'utilisateur (build universel « Build macOS ») : l'application fonctionne ; le logo touchait les pastilles de la barre de titre. Corrigé : logo décalé, précédé d'un fin séparateur (barre principale et accueil), et en plein écran la place des pastilles disparaît.
- Zoom 100 % : clic sur la photo pour zoomer sur le point visé, glisser pour se déplacer, clic ou touche Z pour revenir à l'image entière ; un pixel de l'original par pixel de l'écran (densité Retina comprise). Seule la région visible est rendue, par le même pipeline (GPU ou processeur), vignetage et grain placés d'après la position dans l'image entière : le zoom montre ce que donnera l'export. La pipette fonctionne aussi au zoom ; l'histogramme reste celui de l'image entière.
- Pleine définition : décodée en arrière-plan après une seconde sur la même photo, partagée avec le premier zoom ; libérée au changement de photo. Mesures (Windows, processeur) : 2,0 à 2,6 s pour les capteurs de 24 Mpx, 7,2 s pour le Panasonic en haute résolution (81 Mpx), 5,3 s pour le X-Trans du Fujifilm dématricé en une passe au zoom (11,4 s en trois passes, gardées pour l'export).
- Rendu d'une région de 1400 × 1100 : 3 à 5 ms au GPU. Jumeau GPU : région identique au processeur à un niveau près ; test : chaque pixel d'une région égale celui de l'image entière.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : zoom, déplacement, grain visible au zoom, originaux identiques. Non vérifiés à l'écran : la préparation en arrière-plan (tests du magasin) et l'avant/après au zoom (il ne s'applique qu'à l'image entière pour l'instant).

### 2026-10-20 (suite) : V0.2, réglages intermédiaires (3/3) : netteté et réduction du bruit

- Section Détail du Retoucher : netteté (quantité 0 à 150, rayon 0,5 à 3 px, détail, masquage) et réduction du bruit (luminance, détail de luminance, couleur), avec le rappel de juger au zoom 100 %.
- Bruit de couleur : écarts de couleur moyennés sur 4 px de l'original, luminance intacte. Bruit de luminance : filtre guidé sur la luminance perçue (rayon 2 px), contours gardés. Netteté : masque flou sur la luminance de l'image rendue, halos retenus (détail), aplats épargnés (masquage). Rayons en pixels de l'original, ramenés à l'échelle : aperçu, zoom et export s'accordent ; la région zoomée est rendue avec une marge, identique à l'image entière (test).
- GPU : passes séparées (moyennes et flou séparables), trois tampons de travail créés au premier besoin (la limite garantie est de huit tampons par étape : la première version en utilisait onze et était refusée). Au plus un niveau d'écart avec le processeur sur les six formats ; à 2048 px, 3 à 30 ms au GPU contre 150 à 190 ms au processeur avec la réduction du bruit.
- Test GPU corrigé : une erreur du processeur graphique autre que son absence faisait sauter le test au lieu de l'échouer ; elle échoue désormais.
- Vérifié au zoom 100 % dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : netteté nettement visible ; la réduction du bruit, trop forte au premier essai (texture de l'écorce effacée, aiguilles grisées), a été adoucie (lissage, rayons) puis revérifiée. Originaux identiques.
- Critère « Tous les réglages essentiels et intermédiaires de la section 4.5 » : tous présents ; le retour en moins de 100 ms reste à mesurer sur une carte graphique intégrée (Mac de l'utilisateur).

### 2026-10-20 (suite) : V0.2, géométrie (recadrage, redressement, ratios)

- Format : section `geometry` (recadrage en fractions de l'image, angle de -45 à +45°, quarts de tour, miroirs, ratio retenu), JSON creux. Ordre : orientation, redressement autour du centre, recadrage ; rééchantillonnage bilinéaire une fois par géométrie, avant le reste du pipeline (réduction du bruit, netteté et GPU inchangés ; vignetage d'après le cadre recadré). Recadrage ramené dans l'image redressée, sans coin vide.
- Outil de recadrage (R) : l'image entière redressée, cadre posé dessus (coins et côtés, déplacement, extérieur assombri, tiers) ; ratios libre, original, 1:1, 4:5, 3:2, 16:9 et bascule portrait/paysage (X) ; quarts de tour, miroirs ; Redresser (±45°) et Auto ; Réinitialiser ; Entrée applique, Échap revient à l'état d'avant l'outil. Le cadre n'est enregistré qu'au relâchement : rien n'est recalculé pendant le glissé.
- Horizon automatique (±10°) : directions des contours francs. Sur les six formats inclinés exprès de +3° et -5°, retrouvé à 0,6° près quand la photo a des lignes (Fujifilm, Nikon, Ricoh), rien proposé sur feuillage, branches ou tissus (Canon, Sony, Panasonic) plutôt qu'un angle faux.
- Zoom 100 % : seul le rectangle utile de l'original est converti puis transformé ; région identique à l'image entière (test).
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : outil ouvert par R, coin glissé, redressement de -20,4° avec cadre resserré, Entrée, zoom sur l'image recadrée, XMP, originaux identiques. Limites connues : l'avant/après ne compare pas une image recadrée (tailles différentes) ; les vignettes de Trier et Bibliothèque ne montrent pas encore le recadrage (aperçus rendus, étape de l'export).

### 2026-10-20 (suite) : V0.2, barre d'outils et correcteur de défauts

- Géométrie verte sur Windows et macOS.
- Barre d'outils verticale au bord droit du Retoucher, sur l'idée de l'utilisateur (comme Lightroom) : Réglages, Recadrer (R), Corriger les défauts (Q) ; le panneau ne montre que l'outil choisi, sous l'histogramme, et un clic sur l'outil actif replie le panneau. Dans Réglages, Lumière et Couleur restent ouvertes, Courbe, Couleurs par teinte, Détail et Effets sont repliées tant qu'on ne les ouvre pas. La galerie de filtres y prendra place (étape des presets).
- Correcteur de défauts : un clic corrige le point visé, la source (disque voisin) étant choisie par le cœur d'après la ressemblance de l'anneau autour du défaut et de la texture, loin des autres corrections ; on peut déplacer la correction ou sa source, la retirer (Suppr) ou tout effacer. Mode correction (la différence de teinte et de lumière des bords est fondue dans le disque par interpolation harmonique) ou clonage ; taille (molette, ou [ et ]) et adoucissement. Visualisation des poussières : luminance moins sa moyenne locale, très accentuée, en gris.
- Format : section `spots` (positions en fractions de l'image avant géométrie, rayon en fraction du côté long, adoucissement et mode absents quand ils valent leur défaut), JSON toujours en version 1. Appliqué à l'image linéaire avant la géométrie, sources lues dans l'image non corrigée : une région zoomée donne exactement l'image entière (test), y compris recadrée et redressée.
- Corrigé au passage : la touche Z (zoom) suivait la position QWERTY de la touche ; les raccourcis du Retoucher suivent désormais la lettre tapée, comme les menus. Une case cochée gardait le clavier pour elle et bloquait les raccourcis.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : petit nuage effacé dans un ciel sans raccord visible, à l'aperçu comme au zoom 100 % ; source déplacée, correction retirée, historique en clair, visualisation des poussières, XMP, originaux identiques.
- Reste pour ce critère : copier les corrections d'une photo à l'autre (les poussières d'une série). Les corrections étant des réglages ordinaires, elles passeront par le copier-coller et la synchronisation des réglages (étape des presets).

### 2026-10-20 (suite) : V0.2, corrections d'objectifs

- Barre d'outils et correcteur de défauts verts sur Windows et macOS.
- Décision 0006 : crate `lensfun` 0.7.0 (portage Rust de lensfun, LGPL 3, base CC BY-SA 3.0 incluse), choisie par l'utilisateur ; elle remplace la reprise de `lensdb` prévue par la décision 0004. Crédits ajoutés dans « À propos ».
- Objectif reconnu d'après les EXIF : Canon RF 70-200, Fujifilm XF35, Nikon Z 24-70 et Sony FE 16-35 parmi les échantillons. Sans nom d'objectif, seul l'objectif d'un compact est retenu, sans complément optique, et une focale hors de l'objectif écarte le profil : le Panasonic (pas de nom d'objectif) et le Ricoh GR III (la base n'a que son profil avec complément GW-4) restent sans correction plutôt qu'avec un profil faux.
- Vignetage corrigé sur l'image linéaire décodée, distorsion et aberrations chromatiques en un seul rééchantillonnage par canal, agrandissement automatique sans bord vide ; 25 ms à 2048 px. Région zoomée identique à l'image entière (test). Les corrections de défauts, l'horizon automatique et la géométrie travaillent sur l'image corrigée.
- Section Objectif dans Réglages (repliée) : objectif reconnu, case « Corriger l'objectif », Distorsion et Vignetage de 0 à 200 %. Actives par défaut (section `lens` absente du JSON).
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : objectif reconnu, coins éclaircis, correction désactivée puis réactivée (historique en clair), zoom 100 % dans un coin sans bord vide, originaux identiques. L'affichage « Objectif non reconnu » n'a pas été vu à l'écran (contrôle interrompu : une autre fenêtre était passée devant l'application) ; il est couvert par les tests.
- Reste : les corrections enregistrées par le boîtier (DNG, Sony, Fujifilm, Panasonic) pour les objectifs absents de lensfun, et le choix manuel de l'objectif.

### 2026-10-20 (suite) : V0.2, presets, galerie de filtres, copier-coller

- Corrections d'objectifs vertes sur Windows et macOS.
- Presets : un preset est un jeu de réglages ordinaires et la liste des réglages qu'il commande. Les presets d'aspect (essentiels, noir et blanc, simulations de film) commandent tous contraste, hautes lumières, ombres, blancs, noirs, vibrance, saturation, courbes, couleurs par teinte et grain : en choisir un autre remplace le précédent, sans toucher à l'exposition, à la balance des blancs, à l'objectif ni à la géométrie. Les groupes Exposition (-1 à +1 IL) et Vignetage ne commandent que leur réglage. Intensité de 0 à 100 % (chaque valeur va de son neutre à celle du preset), notée dans les réglages (`look`) pour être réglée de nouveau plus tard.
- Catalogue fourni : 9 essentiels, 3 noir et blanc, 9 simulations de film (négatifs couleur chaud et doré, inversibles saturé, neutre et classique, cinéma tungstène, noir et blanc 100, 400 et 3200), 4 expositions, 2 vignetages. Noms descriptifs, aucune marque ; boîtes de pellicule dessinées (couleurs évoquant le rendu, sensibilité, type de film).
- Presets personnels : créés depuis la photo, réglages gardés au choix ; fichier `presets.json` à côté de la base (il survit à sa reconstruction), écrit d'un coup ; renommer d'un double-clic, supprimer.
- Galerie : quatrième outil de la barre (Filtres) : Optimisation automatique en tête, Original, groupes repliables en grille de trois colonnes, vignettes de la photo ouverte (les 27 en 30 ms), refaites après chaque réglage ; coche sur le choix actif, intensité. Historique : « Filtre : Négatif couleur chaud », « Intensité du filtre 40 % ».
- Copier-coller : Copier les réglages (Ctrl+Maj+C, barre du Retoucher, menu Édition) avec choix des familles, recadrage et corrections de défauts compris mais décochés par défaut ; Coller (Ctrl+Maj+V) sur la photo ouverte ou sur toutes les photos choisies dans la pellicule, une étape d'historique et le XMP pour chacune. Le filtre appliqué suit quand tout ce qu'il commande est collé. Les corrections de défauts se copient ainsi d'une photo à l'autre (poussières d'une série).
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : galerie, preset appliqué et dosé, simulation de film, Optimisation automatique, copie et collage sur une autre photo, XMP, originaux identiques. Non vérifiés à l'écran : la création d'un preset personnel et le collage sur plusieurs photos (couverts par les tests du cœur et de l'interface).
- Limites : le noir et blanc désature après le mélange des couleurs, un filtre coloré façon « filtre rouge » n'est pas encore possible ; appliquer un preset à une sélection passe par le copier-coller ; les vignettes de Trier et Bibliothèque ne montrent pas encore les retouches (étape de l'export).

### 2026-10-20 (suite) : V0.2, export et écran Livrer

- Décision 0007 (choix de l'utilisateur) : HEIF par l'encodeur du système, « Originaux » en copie vérifiée (DNG après la V0.2), police libre Inter (OFL) pour le filigrane, téléchargée avec l'accord de l'utilisateur et livrée dans `src-tauri/assets/fonts` avec sa licence.
- Rendu d'export : depuis l'original, en demi-définition quand la taille demandée le permet ; objectif, défauts, géométrie, taille exacte (jamais d'agrandissement), pipeline au processeur en virgule flottante (identique au rendu 8 bits, test), accentuation de sortie écran ou papier, conversion sRGB, Display P3 ou Adobe RGB (moxcms) avec profil ICC incorporé. Formats : JPEG, PNG et TIFF 8 ou 16 bits, AVIF (sRGB). Écriture par fichier temporaire, jamais par-dessus un fichier existant.
- Métadonnées recréées pour chaque fichier (EXIF de prise de vue, auteur, copyright, GPS ; XMP note, mots-clés, auteur, droits), selon cinq niveaux ; jamais les réglages de retouche. Filigrane texte ou logo PNG, neuf positions, taille relative à la largeur, opacité, marge.
- Presets d'export : les cinq du cahier des charges, modifiables (Rétablir), dupliquables, dans `export.json` à côté de la base avec le filigrane et la destination. Export de la sélection avec plusieurs presets, un sous-dossier chacun ; modèle de nom ({name}, {original}, {date}, {seq}) ; plusieurs photos à la fois ; arrêt possible ; échecs rapportés sans arrêter l'export (un JPEG n'est pas encore rendu, il se copie avec « Originaux »).
- Mesures (Windows, processeur) : JPEG 2048 px en 0,4 à 0,8 s par photo et par cœur de travail (100 photos en bien moins de 2 minutes avec quatre photos à la fois) ; TIFF 16 bits pleine résolution 24 Mpx en 3,3 s.
- Écran Livrer : la sélection au centre, presets cochés et leurs options dépliables, filigrane, destination et ouverture du dossier à droite, bouton « Exporter N photos », progression puis bilan dans la barre d'outils.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : export Web avec filigrane et Instagram dans deux sous-dossiers, fichiers justes (2048 × 1368, objectif corrigé, filigrane), originaux identiques. Le sélecteur de dossier natif n'a pas été piloté (destination préparée dans `export.json`).
- Reste : HEIF par le système, vignettes rendues d'après les retouches (Trier, Bibliothèque, copies virtuelles), aperçu du résultat dans Livrer, export rendu des JPEG.

### 2026-10-20 (suite) : V0.2, vignettes d'après les retouches

- Une photo retouchée a des aperçus rendus d'après sa retouche (moteur d'export, 2048 px puis 1024 et 256), au lieu de l'aperçu intégré au RAW : grilles de Trier et Bibliothèque, pellicule, visionneuse. Une retouche remise à zéro retrouve l'aperçu du boîtier. Migration 0007 : source d'aperçu « rendered ».
- Refaits en arrière-plan quand on quitte la photo dans le Retoucher (photo suivante, autre écran), après un collage de réglages, et au démarrage pour toute retouche plus récente que ses aperçus (application fermée en pleine retouche). La visionneuse n'affiche plus l'aperçu intégré pleine définition d'une photo retouchée, qui ne montrerait pas la retouche.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : filtre noir et blanc appliqué, vignette de la pellicule puis de la grille de Trier en noir et blanc après le passage à la photo suivante ; originaux identiques.

### 2026-10-20 (suite) : V0.2, copies virtuelles dans les grilles

- Choix de l'utilisateur : chaque copie virtuelle apparaît comme une vignette à part, juste après sa photo, avec son aperçu rendu d'après ses propres réglages et une pastille « Copie » ; elle se sélectionne et s'exporte comme une photo ; note, drapeau et couleur restent ceux de la photo.
- Identifiant de grille négatif (moins l'identifiant de la version) : sélection, pellicule, visionneuse et aperçus fonctionnent sans changement ; le cœur ramène la copie à sa photo pour les marques, la fiche et la couverture d'album. Une copie ne va pas à la corbeille (elle se supprime dans le Retoucher). Ouverte dans le Retoucher, elle s'affiche dans sa version.
- Aperçus des copies : rendus à part (sans ligne dans `previews`, la copie n'étant pas un média), refaits quand on quitte la photo et au démarrage s'ils sont plus anciens que la retouche ou absents.
- Export : la copie est rendue avec ses réglages, nommée « nom - Copie 1.jpg » ; le preset des originaux l'ignore (même fichier). Copier-coller : une copie reçoit le collage dans sa version.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : copie créée et passée en noir et blanc dans le Retoucher, vignette « Copie » en noir et blanc après la photo dans Trier et dans la pellicule, note posée sur la copie visible sur la photo, originaux identiques.

### 2026-10-20 (suite) : V0.2, aperçu du résultat dans Livrer

- Section « Aperçu du résultat » en tête du panneau de Livrer : la photo active telle que le preset choisi l'exportera (retouche, recadrage, objectif, défauts, accentuation, filigrane), réduite à 900 px, et la taille du fichier (« Fichier de 1080 × 721 px ») ; choix du preset quand plusieurs sont cochés ; refait peu après chaque changement de photo, de preset ou de filigrane. Pour « Originaux », l'aperçu en cache et la mention du fichier tel quel.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : preset Instagram avec filigrane, changement de photo, originaux identiques.

### 2026-10-21 : V0.2, base reconstruite depuis les dossiers et les XMP

- Il n'existait aucun moyen de reconstruire la base. Nouvelle réanalyse de la bibliothèque (Fichier > Réanalyser la bibliothèque, et sur l'accueil d'une bibliothèque vide : « Retrouver les photos de la bibliothèque ») : les fichiers présents dans les dossiers mais absents de la base sont inscrits, avec leur album (le dossier), leurs marques et mots-clés, et leurs retouches, instantanés et copies virtuelles repris du XMP (`afterdusk:Develop`, `afterdusk:Snapshots`, `afterdusk:Versions`) ; piles RAW + JPEG reformées ; aperçus refaits (rendus d'après la retouche). Rien n'est écrit sur le disque, ni original ni XMP (test). Bilan à la fin, progression dans la barre d'état.
- Inscription d'un média commune à l'import et à la réanalyse. Historique : « Retouche reprise du XMP ».
- Corrigé avant livraison : la barre d'état lisait la progression par un objet recréé à chaque lecture, ce qui relançait le rendu sans fin et laissait la fenêtre noire pendant la réanalyse (vu à l'écran, test ajouté).
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : bibliothèque importée et retouchée, base supprimée puis recréée vide, réanalyse : « 6 photos ajoutées, retouches de 1 photo reprises du XMP », note de 5 étoiles et vignette noir et blanc retrouvées, originaux identiques. Le bouton de l'accueil n'a pas été vu à l'écran (même commande).
- Limites : l'album prend le nom de son dossier (« 2018-03-13 Six boîtiers ») ; la couverture choisie et les collections, qui ne sont qu'en base, ne sont pas retrouvées.

### 2026-10-21 (suite) : V0.2, export HEIF

- Format HEIF dans Livrer (décision 0007) : encodeur du système, WIC sous Windows avec les extensions HEIF et HEVC de Microsoft, ImageIO sous macOS. Qualité réglable, profil ICC incorporé, extension `.heic`. Sans encodeur, l'option est grisée (« HEIF (indisponible) ») avec la raison.
- L'encodeur de Windows ignore les métadonnées qu'on lui donne : l'EXIF et le XMP sont ajoutés ensuite au fichier (éléments « Exif » et « mime », références vers l'image principale, décalages recalés). Testé sur un fichier de référence (octets de l'image inchangés) et relu par le décodeur de Windows (taille, couleur, XMP).
- Test de bout en bout : un NEF importé puis exporté en HEIF porte le copyright et la marque du boîtier ; original inchangé. Sans encodeur (CI Windows), les tests HEIF sont sautés avec un message.
- Code macOS vérifié par compilation croisée (aarch64-apple-darwin) et clippy ; son exécution sera vérifiée par la CI macOS et sur le Mac de l'utilisateur.
- Non vu à l'écran dans l'application compilée.

### 2026-10-21 (suite) : V0.2, critères et mesures

- Liste des critères de la V0.2 reportée ci-dessus. Cochés : ceux dont les tests passent dans la CI sous Windows et macOS (run 39596900326, HEIF compris ; le test HEIF exige désormais l'encodeur sous macOS pour ne plus pouvoir y être sauté sans bruit).
- Mesures (`tests/perf_v02.rs`, version optimisée, sur les six RAW de test ; `cargo test --release --test perf_v02 -- --ignored --nocapture`), sur la machine de développement Windows (RTX 3060 Ti, 20 fils de calcul) :
  - retour visuel d'un curseur, image de 2048 px avec tous les réglages en jeu : 95 % des rendus en 11 ms au GPU, en 81 ms au processeur (`AFTERDUSK_GPU=0`) ;
  - export de 100 photos en JPEG 2048 px (preset Web) : 14,6 s.
- Les deux critères chiffrés restent ouverts : cette machine est au-dessus du milieu de gamme visé. À mesurer sur le Mac de l'utilisateur avec la même commande. Au processeur, une machine modeste dépasserait les 100 ms : c'est le GPU, même intégré, qui tient ce critère.

### 2026-10-21 (suite) : V0.3, choix de départ et sources de localisation

- Choix de l'utilisateur ([décision 0008](decisions/0008-carte-et-materiel.md)) : tuiles OpenFreeMap, frontières Natural Earth hors ligne, géocodage GeoNames (lieux de plus de 1000 habitants), photo de l'appareil sinon silhouette dessinée (ni photo ni logo de fabricant), lieux et carte avant le matériel.
- Photo > Localiser… : trace GPX (une ou plusieurs ; position interpolée à l'heure de la photo, écart maximal réglable, 10 min par défaut) ou photos de téléphone du même moment (la plus proche dans une fenêtre réglable, 30 min par défaut). Heure du boîtier ramenée en UTC par le fuseau qu'il a écrit, ou par un décalage saisi (fuseau et avance de l'horloge). Une pile RAW + JPEG prend un seul lieu. Le GPS du boîtier n'est jamais remplacé ; un lieu déjà posé seulement sur demande. Bilan dans la boîte (localisées, déjà localisées, sans point proche, sans heure).
- Le lieu va en base (`lat`, `lon`, `location_source`) et dans le XMP (`exif:GPSLatitude`, `exif:GPSLongitude`, `afterdusk:LocationSource`), jamais dans l'original ; la réanalyse le retrouve ; l'export l'écrit dans le fichier sauf preset sans GPS. Traces gardées pour la carte (migration 0008, table `gpx_tracks`). Fiche Informations : ligne « Lieu » avec sa source.
- Corrigé au passage : le GPS des JPEG et HEIF (photos de téléphone, compagnons des RAW) n'était pas lu à l'import.
- Tests : rapprochement (interpolation, trous de la trace, fenêtre), lecture GPX, aller-retour XMP des coordonnées, et de bout en bout sur de vrais RAW : trace GPX puis XMP, réanalyse qui retrouve le lieu, photo de téléphone générée ; originaux, trace et photo de téléphone inchangés. Boîte testée dans l'interface (Vitest).
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : trois RAW de 2018, 2019 et 2021 aux fuseaux différents, Photo > Localiser… avec une trace GPX choisie dans la boîte de Windows : « 3 photos localisées » aux positions attendues, XMP écrits, originaux et trace identiques ; second passage : « 3 déjà localisées ». Corrigé à cette occasion : la ligne « Lieu » affichait sa clé de traduction (variable `ns` réservée par i18next ; test ajouté), la sélection retombait à une photo après la localisation, et la boîte annonçait un seul fuseau quand les photos en portent plusieurs. L'origine du lieu a désormais sa propre ligne, la valeur ne tenant pas dans le panneau.
- Restent pour ce critère : saisie manuelle (après le géocodage et la carte) et correction du fuseau horaire proposée d'après le lieu.

### 2026-10-21 (suite) : Tout sélectionner dans les grilles

- Édition > Tout sélectionner (Ctrl+A, ⌘A) ne sélectionnait que du texte : il sélectionne désormais toutes les photos affichées dans Trier, Bibliothèque, Retoucher (pellicule) et Livrer, la photo active restant la même ; toute la carte dans Importer ; le texte d'un champ quand il a le focus. Rien quand la Bibliothèque montre les albums. Tests ajoutés (Vitest). Non vu à l'écran : l'utilisateur se servait de la machine.
- Reste : « Ne rien sélectionner » est une entrée inactive ; le modèle de sélection garde toujours une photo active, ce qui est à trancher avant de l'activer.

### 2026-10-21 (suite) : V0.3, géocodage hors ligne et saisie d'un lieu

- Données GeoNames (CC BY 4.0, téléchargées le 2026-10-21 avec l'accord de l'utilisateur) converties par `scripts/build-geonames.mjs` en `src-tauri/assets/geo/places.bin.z` (2,7 Mo) : 161 816 lieux de plus de 1000 habitants, 246 pays, régions et fuseaux. Écartés : quartiers et arrondissements (« Marseille 02 », « Paris 16 Passy »), lieux disparus ou abandonnés. Crédit dans « À propos ».
- Lieu le plus proche par une grille d'un degré, la distance pondérée par la population pour qu'une grande ville l'emporte sur la commune voisine (tour Eiffel : Paris ; Empire State Building : New York City) ; rien au-delà de 60 km (en mer). Recherche par nom sans accents ni casse, la plus peuplée d'abord.
- Ville, région, pays (code ISO) et fuseau IANA en base (migration 0009), nommés à la localisation, à l'import (GPS du boîtier ou du téléphone) et au démarrage pour les photos déjà localisées ; le lieu entre dans la recherche (« Cabourg », « France »).
- Fiche Informations : « Lieu » (« Cabourg, France », pays dans la langue de l'interface), « Coordonnées », « Origine du lieu ». Les régions GeoNames sont en anglais : elles servent à la recherche et à distinguer les homonymes dans la saisie, sans être affichées dans la fiche.
- Photo > Localiser…, onglet « Saisie » : recherche d'une ville, choix dans la liste, lieu posé sans condition d'heure. Les quatre sources sont là ; le clic sur la carte viendra avec la carte.
- Tests : lieux connus (deux hémisphères, de part et d'autre de 180°, grandes villes, pleine mer), recherche sans accents, vitesse ; de bout en bout : saisie, lieu gardé ou remplacé, recherche, base antérieure nommée au démarrage, photo de téléphone importée avec sa ville ; boîte testée dans l'interface. Non vu à l'écran.
- Reste pour le premier critère : la correction du fuseau horaire proposée d'après le lieu.

### 2026-10-21 (suite) : V0.3, correction de l'heure d'après le lieu

- Photo > Corriger l'heure… : pour chaque photo localisée et nommée, le fuseau du lieu à sa date (règles IANA par `chrono-tz`, heure d'été comprise) est comparé à celui de l'horloge du boîtier (écrit par lui, ou saisi ; celui de l'ordinateur par défaut s'il n'en a pas écrit). Les écarts sont présentés par groupe (« New York City : horloge UTC +01:00, heure locale UTC -05:00, −6 h (12 photos) »), avec ce qui est déjà juste ou sans lieu ; « Corriger » applique.
- L'heure et le fuseau corrigés vont en base, l'heure du boîtier est gardée (migration 0010, ligne « Heure du boîtier » dans la fiche), le XMP reçoit `exif:DateTimeOriginal` avec son fuseau, la réanalyse le retrouve, l'export l'écrit. L'original n'est pas touché et les fichiers ne changent pas de dossier.
- Tests : fuseaux en été et en hiver, demi-heures ; de bout en bout sur un vrai NEF : écart de −6 h proposé à New York, correction, plus rien à proposer ensuite, XMP, réanalyse, original inchangé ; boîte testée dans l'interface. Non vu à l'écran.

### 2026-10-21 (suite) : V0.3, localisation proposée à la fin de l'import

- Comme le prévoit le parcours du cahier des charges (section 2, étape 3) : à la fin d'un import, si des photos n'ont pas de GPS, la barre d'Importer propose « Localiser les N photos sans lieu… », qui ouvre Photo > Localiser… sur ces photos (trace GPX, photos de téléphone ou saisie). Une photo par pile ; le nombre baisse après chaque localisation et la proposition disparaît quand toutes ont un lieu. Les photos avec GPS (boîtier, téléphone) sont localisées et nommées à l'import sans rien demander.
- Tests : photos sans lieu d'un import (cœur), proposition affichée puis retirée après la localisation (interface). Non vu à l'écran.

### 2026-10-21 (suite) : vérifications à l'écran

- Dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises), quatre photos de test : Ctrl+A et Édition > Tout sélectionner sélectionnent les photos de la grille, Ctrl+A dans la recherche n'en sélectionne que le texte ; onglet « Saisie » : « New York City, États-Unis » trouvé, deux photos localisées ; Corriger l'heure… : −5 h pour mars 2018 (heure d'été américaine déjà en vigueur) et −6 h pour novembre 2019, puis « 2 photos corrigées », fiche « Prise de vue 11:38 » et « Heure du boîtier 16:38 » ; fin d'import de deux RAW sans GPS : « Localiser les 2 photos sans lieu… », qui disparaît une fois les photos placées à Paris. XMP écrits (coordonnées, heure corrigée avec son fuseau), originaux et carte inchangés. Cas réel au passage : le Panasonic écrit +01:00 en mai à Paris, la boîte propose +1 h.
- Corrigé à cette occasion : valeurs longues de la fiche coupées (elles passent désormais à la ligne, l'hémisphère reste avec son nombre), champ de décalage trop large et groupes en gras dans les boîtes, fuseau écrit non relu après une correction, barre d'Importer serrée sur deux lignes (boutons sur une ligne, état abrégé avec infobulle). La barre d'Importer corrigée n'a pas été revue à l'écran.
- Deux premiers critères de la V0.3 cochés : CI verte sous Windows et macOS (run 39618722090). Le clic sur la carte, cinquième façon de saisir un lieu, viendra avec la carte.

### 2026-10-21 (suite) : V0.3, carte du monde

- Choix de l'utilisateur : MapLibre GL JS 6.12.0 (BSD 3), frontières Natural Earth au 1:50m (domaine public, `scripts/build-borders.mjs`, 1,7 Mo chargés à l'ouverture de la Carte), fond en ligne OpenFreeMap activé par défaut et désactivable (Réglages > Général), comme l'exige la règle réseau de CLAUDE.md. La politique de sécurité de l'interface n'ouvre que `tiles.openfreemap.org` ; sans réseau ou fond coupé, la carte n'affiche que les frontières embarquées et ne fait aucune requête. MapLibre n'est chargé qu'à l'ouverture de la Carte.
- Écran Carte : albums localisés en vignettes (point central de leurs photos), groupés en grappes au dézoom (clic : zoom sur la grappe), liste des albums à gauche, aperçu à droite (couverture, dates, lieu, photos localisées, traces GPX, « Ouvrir l'album » ; double clic sur la vignette : l'album). En zoomant, les photos une à une et la trace GPX des jours de l'album. « Tracer une zone » : rectangle, nombre de photos, « Voir les N photos dans la Bibliothèque » (filtre « Zone de la carte » dans la barre de filtres, y compris à cheval sur 180°). « Placer N photos » : un clic pose le lieu sur la sélection, sans remplacer un lieu existant. Échap quitte l'outil.
- Tests : données de la carte, filtre de zone et traces (cœur), zones et longitudes (interface), écran Carte avec la carte remplacée (WebGL absent des tests).
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : quatre albums (Cabourg avec trace GPX, Paris, Marseille, New York), grappe « 3 » sur la France, vignette à New York, aperçu de l'album de Cabourg, trace et photo en zoomant, zone tracée puis « 1 sur 4 photos » dans la Bibliothèque, fond hors ligne après coupure dans les Réglages. Corrigé à cette occasion : carte de hauteur nulle (MapLibre impose sa position), couverture absente de l'aperçu, vignettes d'albums masquant les photos en vue détaillée, album choisi repéré par son nom. Non vu à l'écran : le placement d'un clic (testé dans l'interface et le cœur).
- Les préférences de l'interface (dont l'interrupteur du fond de carte) sont gardées par WebView2 hors du dossier de données : l'interrupteur coupé pendant l'essai a été remis.
- Critères de la carte et du fond hors ligne cochés : CI verte sous Windows et macOS (run 39625230546).

### 2026-10-21 (suite) : V0.3, matériel (inventaire et fiches)

- Migration 0011 : tous les types (boîtier, objectif, flash, filtre, trépied, sac, batterie, autre) et les champs de la fiche (nom choisi, silhouette, numéro de série, date et lieu d'achat, prix et devise, état, garantie, notes, confirmé, photo). La table est recréée pour changer sa contrainte : les migrations coupent désormais les clés étrangères (hors transaction) et vérifient l'intégrité avant de les rétablir ; sans cela, la suppression de l'ancienne table aurait vidé les liens boîtier et objectif des photos (test sur une base arrêtée à la version 10, puis migration réelle d'une base d'essai : les 4 liens gardés).
- Écran Matériel : boîtiers, objectifs et accessoires, avec le nombre de photos ; fiche modifiable (marque, modèle et type pour un accessoire saisi à la main seulement), noms relevés dans les photos, première et dernière photo. Illustration : la photo choisie dans la Bibliothèque ou une image déposée (copiée et vérifiée à côté de la base, la source intacte), sinon une silhouette dessinée pour Afterdusk (hybride, reflex, compact, focale fixe, zoom, trépied ; devinée d'après le modèle, modifiable) et la marque en toutes lettres. Fusion de deux fiches du même type (noms EXIF, photos et champs vides repris), masquage, « C'est mon matériel », suppression d'un accessoire saisi (un appareil relevé dans les photos se masque seulement).
- Tests : silhouettes devinées, migration sans liens rompus, et de bout en bout sur les six RAW (inventaire, fusion et nouvel import retrouvant la fiche fusionnée, masquage, accessoire, fiche, photo déposée puis retirée) ; écran testé dans l'interface.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises) : inventaire de la base d'essai migrée, fiche du Canon EOS R6 avec son numéro de série, photo de la Bibliothèque en illustration, trépied ajouté puis nommé. Corrigé à cette occasion : un accessoire sans marque ni modèle n'avait pas de nom (il prend celui de son type). La fusion et l'image déposée n'ont pas été faites à l'écran (testées dans le cœur et l'interface).
- Restent pour le matériel : statistiques et graphiques, compteur de déclenchements, entretien et rappels, export PDF et CSV.

### 2026-10-21 (suite) : V0.3, statistiques du matériel et compteur de déclenchements

- Compteur de déclenchements lu à l'import dans les notes du fabricant quand la marque l'écrit en clair : Nikon (ShutterCount) et Fujifilm (ImageCount), dans les RAW comme dans les JPEG. Le RAW n'est lu que par petits morceaux, et jamais modifié. Canon et Sony rangent le leur dans des blocs propriétaires ou chiffrés : pour eux, et pour toute photo sans compteur, l'estimation est le nombre de photos importées (« au moins N »). Avec un compteur lu, l'écran donne la dernière valeur et les photos prises depuis. Pour les photos importées avant cette version, l'ouverture des statistiques d'un boîtier Nikon ou Fujifilm relève le compteur dans l'original le plus récent, en lecture seule (testé en effaçant les compteurs de la base).
- Statistiques d'un boîtier ou d'un objectif : focales (regroupées par tranches au-delà de 24 valeurs), ouvertures et ISO les plus fréquents, photos par mois (par année au-delà de trois ans), objectifs utilisés avec un boîtier et l'inverse, albums où il a servi, avec l'objectif favori pour un boîtier. Les JPEG compagnons d'un RAW et la corbeille ne comptent pas. Le tri par type de scène attendra une classification des scènes.
- Graphiques dessinés en SVG dans Afterdusk, sans dépendance : une seule série par graphique, la barre la plus haute indiquée, la valeur exacte au survol, un tableau masqué pour les lecteurs d'écran.
- Tests : lecture des compteurs (notes construites et échantillons : Nikon Z 6 à 739, X-T3 à 3850, rien pour Sony et Canon), statistiques de bout en bout sur les six RAW, tranches et mois dans l'interface, écran testé dans l'interface.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises, originaux d'essai intacts) : le Nikon Z 6 importé avant cette version affiche 739 déclenchements relevés dans l'original, le Canon EOS R6 « au moins 1 » avec son explication, les graphiques, les objectifs et les albums ; la fiche de l'objectif s'ouvre depuis le boîtier. Corrigé à cette occasion : une valeur unique remplissait toute la largeur du graphique (barres limitées à 32 px et centrées), et « nov. 19 » se lisait comme une date (l'année s'écrit en entier).
- Fiche d'un appareil en tableau de bord, à la demande de l'utilisateur (tout s'empilait sur une colonne de 560 px) : une grille de cartes sur toute la largeur. Identité et photo, fiche sur trois rangées (les notes s'allongent jusqu'en bas), puis les quatre graphiques, le matériel associé et les albums. Les graphiques se dessinent à la largeur de leur carte, sans mise à l'échelle du texte. Vérifié à l'écran : en plein écran, tout tient sans défilement ; à 1296 × 829, seules les cartes du matériel associé et des albums demandent de descendre.

### 2026-10-21 (suite) : V0.3, journal d'entretien et rappels

- Migration 0012 : table `maintenance` (appareil, date, intervention, coût et devise, notes, intervalle de rappel en mois, facultatif). Interventions : nettoyage du capteur, révision, réparation, mise à jour du firmware, autre. Une ligne du journal n'a pas de fichier : elle se supprime directement, après confirmation. Le journal suit la fiche lors d'une fusion, et disparaît avec un accessoire supprimé.
- Rappels : pour chaque appareil et chaque type d'intervention, la plus récente décide. Son intervalle fixe l'échéance ; sans intervalle, pas de rappel (fin de mois respectée : 31 mars + 24 mois = 31 mars). Le matériel masqué sort des rappels généraux. Seulement dans l'application : un compteur sur l'onglet Matériel, un repère « entretien » dans la liste, une ligne par rappel échu sur l'accueil, qui ouvre la fiche. Ni notification du système, ni tâche quand l'application est fermée, ni réseau.
- Écran : une carte « Entretien » dans le tableau de bord de l'appareil, avec les échéances (en couleur d'avertissement et une icône quand elles sont passées), le journal et l'ajout d'une intervention. La modification d'une ligne existe dans le cœur, pas encore à l'écran : on la supprime et on la saisit à nouveau.
- Tests : rappels (plus récente seule, sans intervalle, suppression qui fait revenir l'ancienne, modification, matériel masqué, date invalide), journal reporté lors d'une fusion de bout en bout, carte, repère et ligne de l'accueil dans l'interface.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises, originaux d'essai intacts, base d'essai migrée en version 12). Avec un nettoyage du capteur échu sur le Nikon Z 6 et une révision à venir sur l'objectif, la ligne de l'accueil ouvre la fiche du Nikon. Le compteur de l'onglet, le repère de la liste et l'échéance de la carte s'affichent. Un nettoyage ajouté le jour même, sans intervalle, efface le rappel, et le supprimer (après confirmation) le fait revenir. L'objectif affiche « prévu le 1 juin 2027 ». Corrigé à cette occasion : le compteur de l'onglet n'avait pas de style, collé au mot « Matériel », et la phrase de l'accueil portait une majuscule au milieu.

### 2026-10-21 (suite) : V0.3, export de l'inventaire en PDF et CSV

- Décision de l'utilisateur : le PDF est produit par le cœur Rust avec printpdf 0.12.8 (MIT), plutôt que par l'impression du système. La dépendance est prise sans ses options par défaut, donc sans le moteur HTML ; seul le décodage JPEG des images est activé. Toutes les dépendances entraînées sont sous MIT, Apache-2.0, BSD ou Zlib, compatibles GPL v3. Police : Inter, déjà embarquée (OFL). Le gras des titres est simulé par un léger contour, la police étant variable.
- Partage du travail : l'interface prépare le contenu, traduit par i18next, dates et montants mis en forme par la langue. Le cœur met en page et écrit le fichier, à côté puis renommé, pour qu'un export interrompu ne laisse pas de fichier à moitié écrit. Il va chercher la photo de chaque appareil : l'image déposée sur la fiche, sinon l'aperçu en cache de la photo choisie dans la bibliothèque.
- PDF A4 : titre, bibliothèque, date et nombre d'appareils ; tableau récapitulatif (type, appareil, numéro de série, achat, prix, état, photos, dernier entretien), répété en haut de chaque page ; puis une fiche par appareil avec sa photo, ses seuls champs renseignés et son journal d'entretien ; numéro de page en pied. Textes trop longs coupés avec des points de suspension, mesurés sur les largeurs des glyphes.
- CSV : toutes les colonnes de la fiche, plus les photos, les déclenchements et le dernier entretien. Valeurs brutes pour le calcul (dates AAAA-MM-JJ, prix sans symbole). UTF-8 avec BOM et lignes CRLF, pour Excel. Séparateur point-virgule là où la virgule est décimale.
- Deux boutons dans la barre d'outils du Matériel, « Exporter en PDF… » et « Exporter en CSV… », avec la boîte « Enregistrer sous » (permission `dialog:allow-save` ajoutée). Le matériel masqué n'est pas exporté.
- Tests : CSV (BOM, guillemets, séparateur), PDF de plusieurs pages relu par printpdf, écriture sans fichier `.part` restant, photo déposée retrouvée pour le PDF de bout en bout, contenu traduit et bouton dans l'interface. Rendu d'un PDF d'essai contrôlé à l'œil (accents, tableau, fiches avec et sans photo, pied de page). Ajouté au passage : les messages des erreurs « intervention introuvable » et « date invalide », oubliés avec l'entretien.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises, originaux d'essai intacts). Les deux boutons ouvrent « Enregistrer sous » avec le nom et le type proposés. Le PDF de la base d'essai fait trois pages : le Canon avec la photo choisie dans la bibliothèque, le Nikon avec son journal d'entretien et ses 739 déclenchements. Le CSV a son BOM, ses lignes CRLF et le point-virgule, et aucun fichier `.part` ne reste. Corrigé à cette occasion :
  - le numéro de série et les en-têtes étaient coupés dans le tableau (colonnes réparties autrement ; « Entretien » n'y garde que la date de la dernière intervention) ;
  - un compteur estimé s'écrivait comme un compteur lu : « au moins N » dans le PDF, et rien dans le CSV, qui a déjà la colonne Photos.
- Limite connue : Excel retire le zéro initial d'un numéro de série comme « 093022004357 », qu'il prend pour un nombre. Le CSV garde la valeur exacte ; d'autres tableurs, ou l'import de texte d'Excel, la lisent correctement.
- Idée de l'utilisateur : des planches contact en PDF, qui réutiliseraient cette mise en page. Inscrites au cahier des charges en V0.4 (étape Livrer).
- Critères de l'inventaire, des fiches avec statistiques et du journal d'entretien cochés : CI verte sous Windows et macOS (run 39636780190, commit f709996).
- Critère de l'export d'inventaire PDF et CSV coché : CI verte sous Windows et macOS (run 39641509658, commit e57229c). Reste pour la V0.3 : les albums intelligents.

### 2026-10-21 (suite) : V0.3, albums intelligents et collections manuelles

- Décision de l'utilisateur : les collections manuelles (4.2), jamais faites jusqu'ici, arrivent avec les albums intelligents, dans le même panneau.
- Cœur (`library/collections.rs`), sur la table `collections` prévue dès la migration 0001, sans nouvelle migration. Une collection manuelle garde la liste des photos qu'on y ajoute (à la fin, doublons ignorés). Un album intelligent garde un filtre en JSON versionné (`{"version":1,"filter":…}`) et se recalcule à chaque lecture : une photo notée 5 étoiles y entre aussitôt. Un champ absent prend sa valeur par défaut ; une version inconnue ou un texte abîmé donnent un filtre vide plutôt qu'une erreur. Un album intelligent n'enregistre jamais de collection manuelle, qui pourrait disparaître. Comptes hors corbeille, une photo par pile. Supprimer une collection ne touche à aucune photo.
- Les collections vivent dans la base seulement, comme celles de Lightroom : une base perdue puis reconstruite depuis les XMP ne les retrouve pas.
- Filtre : nouvelle pastille « Focale » dans la barre de filtres (focales présentes, arrondies au millimètre, avec leur nombre de photos), nécessaire à l'exemple du cahier des charges « 5 étoiles prises au 23 mm ». Le filtre accepte aussi une collection manuelle, que le magasin pose quand on l'ouvre (la barre de filtres reste libre).
- Panneau de gauche (Bibliothèque et Trier), section Collections :
  - deux boutons : « Nouvelle collection… » et « Enregistrer le filtre comme album intelligent… » (actif quand un filtre est posé). Le nom se saisit dans le panneau : Entrée valide, Échap annule.
  - Les albums intelligents portent la mention « intelligent ». Un clic ouvre une collection manuelle, ou applique le filtre d'un album intelligent à toutes les photos.
  - Sur une collection manuelle, un bouton « + » ajoute les photos sélectionnées.
  - Sous la collection ouverte : Renommer, Remplacer par le filtre actuel (album intelligent dont le filtre a changé), Retirer de la collection (sélection) et Supprimer, après confirmation.
- Tests : filtre enregistré et relu (version, champ absent, version future), de bout en bout sur les six RAW (focales proposées, « 5 étoiles à 52 mm » vide puis rempli après la note, collection manuelle avec doublon et identifiant inconnu, retrait, corbeille puis restauration, refus croisés, nom vide, suppression sans perte de photo). Côté interface : panneau (ouverture, création, ajout, retrait, remplacement du filtre, suppression), comparaison de filtres et pastille Focale.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises, originaux d'essai intacts) :
  - « Sélection client » créée, puis les quatre photos ajoutées (Ctrl+A et « + »). La collection ouverte montre ses quatre photos ; une photo retirée, le compteur passe à 3.
  - La focale « 32 mm (1) » ne garde que la photo du Sony ; enregistrée comme album intelligent « Au 32 mm ». Le filtre réinitialisé, l'album intelligent l'applique de nouveau.
  - Corrigé à cette occasion : les champs vides du filtre revenaient du cœur sous forme de `null`, et l'interface y voyait une zone de carte posée (pastille « Zone de la carte » fantôme). Le cœur ne les écrit plus et l'interface nettoie un filtre relu.
  - Corrigé aussi : le nom lu au clavier d'une ligne de liste collait le libellé à sa précision (« Essai12 mai 2021 »).
- Critère des albums intelligents coché : CI verte sous Windows et macOS (run 39650062200, commit c324240). Tous les critères de la V0.3 sont cochés.

### 2026-10-21 (suite) : préparation de la V0.4

- Décision 0009 complétée avec les réponses de l'utilisateur aux cinq questions ouvertes :
  - import automatique dans l'ordre carte connue, dossier surveillé, FTP ;
  - affichage dans le tri en moins de 5 secondes ;
  - FTP et FTPS acceptés, avec un avertissement en FTP simple ;
  - fiche client avec courriel et téléphone facultatifs ;
  - onze types de séance fournis.
- Planches contact ajoutées au cahier des charges (4.6) et aux critères de la V0.4 : grille de 2 à 8 colonnes au choix, vignettes numérotées avec le nom du fichier, en-tête (album, client et séance, date, photographe), vignettes retouchées, filigrane en option.

## V0.4 : Assistants et partage

Critères (cahier des charges, section 7) :

- [x] Regroupement des rafales en piles
- [ ] Détection locale du flou, des yeux fermés et des problèmes d'exposition, avec filtre « Ratés probables »
- [ ] Analyse d'une journée de 1 000 photos en moins de 10 minutes en arrière-plan, sans ralentir le tri
- [ ] Vidéos : import, rangement, tri, lecture, fichiers annexes
- [ ] Galerie web statique avec sélection client et réimport des choix
- [ ] Planches contact en PDF d'un album ou d'une collection, numérotées, avec en-tête et filigrane en option
- [ ] Clients et types de séance rattachés aux albums, avec filtres, recherche et suppression complète d'un client
- [ ] Import automatique : carte connue, dossier surveillé et réception FTP sur le réseau local, chaque photo affichée dans le tri moins de 5 secondes après son arrivée
- [ ] Installateurs signés pour Windows et macOS, mises à jour automatiques

### 2026-10-21 (suite) : V0.4, regroupement des rafales en piles

- Schéma validé par l'utilisateur. Migration 0013 : table `bursts` (bibliothèque, photo du dessus, automatique ou empilée à la main) ; sur `media`, `burst_id` et `phash`. Les rafales vivent dans la base seulement, comme les collections.
- Détection (`library/bursts.rs`) :
  - Pour chaque boîtier, par ordre de prise de vue, une chaîne de photos séparées de 2 secondes au plus, chacune proche de la précédente.
  - Proximité visuelle : hash perceptuel dHash sur 64 bits, au plus 10 bits différents. Il est calculé sur l'aperçu de 256 px s'il vient du boîtier, sinon sur l'original, lu sans être écrit. Un aperçu retouché ne sert pas.
  - Fractions de seconde (SubSecTimeOriginal) reprises des balises déjà relevées à l'import.
  - Le hash n'est calculé que pour les photos qui ont une voisine dans le temps : une bibliothèque sans rafale ne coûte presque rien.
  - Une paire RAW + JPEG compte pour une photo et reste une paire à l'intérieur de la rafale.
- Recalcul : seules les rafales automatiques sont refaites ; le dessus choisi par l'utilisateur reste au-dessus s'il est toujours dans la rafale. Retirer une photo rend la rafale manuelle, et la détection ne la touche plus. Une rafale réduite à une photo disparaît.
- Photo du dessus : la mieux notée, puis la retenue, puis la première. La plus nette passera devant la première avec la détection des ratés.
- Grille :
  - sans filtre, une rafale est repliée sur sa photo du dessus, avec un badge (icône et nombre de photos) en haut à gauche, le badge RAW + JPEG restant à droite ;
  - un clic sur le badge, ou S, la déplie : ses photos suivent celle du dessus, reliées par un liseré, quel que soit le tri ;
  - un filtre montre chaque photo qui y répond ;
  - jetée, la photo du dessus laisse sa place à la suivante.
- Commandes :
  - menu Photo : Regrouper les rafales (album affiché, sinon bibliothèque), Empiler en rafale (sélection, la photo active au-dessus), Retirer de la rafale ;
  - menu contextuel : Mettre en haut de la rafale, Retirer de la rafale ;
  - « Mettre en haut de la pile » vaut aussi pour une rafale, et le nouveau dessus d'une paire prend sa place dans la rafale.
- Automatique : la détection tourne en arrière-plan à la fin de chaque import réussi. Réglages > Import : « Regrouper les rafales après chaque import » (oui par défaut, préférence locale).
- Les notes posées sur la photo du dessus ne valent que pour elle, contrairement aux paires RAW + JPEG, qui sont une seule prise de vue.
- Tests :
  - chaînes de temps et de ressemblance, fractions de seconde, dHash stable à l'échelle et discriminant ;
  - choix du dessus ;
  - de bout en bout : trois prises du Nikon Z 6 et un Sony. Rafale de trois, dessus le mieux noté, hash calculé pour les trois seulement, originaux et carte intacts, repliée et dépliée, filtre, recalcul qui garde le dessus choisi, corbeille, retrait, empilage à la main ;
  - côté interface : badge, dépliage au clic et avec S, dessus changé, détection, empilage, retrait, préférence.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises, originaux d'essai intacts) :
  - trois prises du Nikon Z 6, copies du RAW d'essai, ajoutées à la bibliothèque d'essai et inscrites par Fichier > Réanalyser la bibliothèque ;
  - Photo > Regrouper les rafales : avec l'original, quatre prises repliées sur une vignette au badge « 4 » ;
  - le badge déplie la rafale, et S la déplie depuis le clavier ;
  - « Mettre en haut de la rafale » fait passer DSC_1002 au-dessus et replie la rafale ;
  - « Retirer de la rafale » sort DSC_1001, et la rafale passe à 3.
- Corrigé à cette occasion : une fois la rafale dépliée, chaque prise répétait le badge ; il ne reste que sur la photo du dessus.
- Le regroupement automatique en fin d'import n'a pas été vu à l'écran ; il est couvert par le test du magasin et le test de bout en bout du cœur.
- Critère du regroupement des rafales coché : CI verte sous Windows et macOS (run 39658755656, commit 9cd7406).

### 2026-10-21 (suite) : V0.4, ratés probables (flou et exposition)

- Ordre choisi par l'utilisateur : flou et exposition d'abord, sans dépendance nouvelle ; le prototype des yeux fermés (ONNX) ensuite.
- Migration 0014 : table `analysis` du cahier des charges (score de netteté, flou, exposition, yeux fermés encore vides, version de l'analyse). Le hash perceptuel reste sur `media` (migration 0013). Une nouvelle version de l'analyse relance le calcul des photos déjà vues.
- Mesures (`analysis/`), sur la luminance ramenée à 1 024 px de côté :
  - netteté : variance du laplacien par tuile, sur une grille de 8 × 8, et moyenne des quatre tuiles les plus nettes. Un sujet net sur un fond flou reste net ; un bougé abaisse toutes les tuiles ;
  - seuil 80, calibré sur les six RAW de test : nets de 290 à 3 400, floutés (rayon 3 px) de 6 à 74. Un test sur ces échantillons garde ce calibrage ;
  - exposition : part de pixels écrêtés (≥ 250) ou bouchés (≤ 5), et luminance moyenne. Résultats : « surexposée probable » ou « sous-exposée probable ».
- Source : l'aperçu en cache s'il vient du boîtier, sinon l'original, lu sans être écrit. Un aperçu retouché ne sert pas, car on juge la prise de vue. Cette lecture est désormais commune avec le hash des rafales.
- Analyse en arrière-plan (`analysis_run`) : photos jamais vues ou vues par une version plus ancienne. La progression s'affiche dans la barre d'état, avec « Arrêter » entre deux photos. Photo > Rechercher les ratés (album affiché, sinon bibliothèque). Après chaque import : ratés, puis rafales, chacun désactivable dans Réglages > Import (préférences locales).
- Interface :
  - une pastille d'avertissement en bas à gauche des vignettes signalées, qui donne les raisons (« Floue ou bougée probable, Surexposée probable ») ;
  - un bouton « Ratés probables » dans la barre de filtres ; on les revoit, puis on les rejette en lot avec Ctrl+A et X. Afterdusk ne rejette rien seul.
- Rafales : à note et drapeau égaux, la plus nette passe au-dessus.
- Tests :
  - mesures : damier net, flouté, sujet net sur fond flou, expositions, échantillons réels ;
  - de bout en bout : les six RAW nets et bien exposés, une prise floutée et une prise brûlée signalées, filtre, seconde analyse sans travail, source intacte ;
  - choix du dessus par la netteté ;
  - côté interface : magasin, ordre ratés puis rafales après l'import, préférences, pastille, bouton du filtre.
- Reste pour le critère : les yeux fermés (prototype ONNX, licences à vérifier, téléchargements à autoriser). Le critère de performance (1 000 photos en moins de 10 minutes) sera mesuré sur l'analyse complète.
- Vérifié dans l'application compilée sous Windows (données de l'utilisateur mises de côté puis remises, originaux d'essai intacts) :
  - une prise floutée (bouge.jpg) et une prise éclaircie à l'extrême (brulee.jpg), tirées du RAW du Nikon, ajoutées à la bibliothèque d'essai et inscrites par la réanalyse ;
  - Photo > Rechercher les ratés : en quelques secondes, seules ces deux photos portent la pastille, avec « Floue ou bougée probable » et « Surexposée probable » ; les RAW nets n'en ont aucune ;
  - « Ratés probables » ne garde qu'elles (« 2 sur 9 photos »), et Ctrl+A puis X les rejette en lot.

### 2026-10-21 (suite) : boîtes de dialogue aux couleurs d'Afterdusk

- Demande de l'utilisateur : les questions et messages (confirmation d'une suppression, bilan de la réanalyse) s'affichaient dans une boîte du système, claire et étrangère à l'application. Ils passent désormais par une boîte d'Afterdusk : fond assombri, panneau sombre, icône d'information ou d'avertissement, boutons en bas à droite.
- Une confirmation destructive met « Annuler » en avant ; le bouton d'action est rouge. Échap annule une question et ferme un message. Les demandes simultanées attendent leur tour.
- La boîte est montée dans la fenêtre principale et dans l'accueil. Les autorisations `dialog:allow-ask` et `dialog:allow-message` ne servent plus et sont retirées.
- Les sélecteurs de fichiers et « Enregistrer sous » restent ceux du système : ce sont des explorateurs complets (favoris, réseau, recherche), qu'il vaut mieux ne pas refaire.
- Tests : focus sur Annuler, Échap, validation, file d'attente, message à un seul bouton. Pas encore vérifié dans l'application compilée.

### 2026-10-21 (suite) : V0.4, yeux fermés (décision 0010)

- Prototype hors du dépôt, sur onze photos libres de Wikimedia Commons (CC0 et domaine public), notées à la main :
  - YuNet trouve les visages et leurs yeux ; Face Mesh V2 place 478 repères ; le rapport d'ouverture de chaque œil (EAR) dit s'il est fermé ;
  - deux yeux fermés trouvés sur trois (le troisième est un profil endormi, sans visage trouvé), aucune fausse alerte, lunettes de soleil, regard baissé et rire compris ;
  - tract et ort donnent les mêmes valeurs ; ort est deux à cinq fois plus rapide. L'utilisateur a choisi ort, malgré la bibliothèque native.
- ONNX Runtime 1.22.0, pour la bibliothèque universelle sur Mac (Intel compris), récupérée par `scripts/fetch-onnxruntime.mjs` depuis la publication officielle de Microsoft, vérifiée par SHA-256 et livrée par Tauri (12,4 Mo sous Windows, 70,8 Mo sous macOS). La CI la récupère et la garde en cache. Les deux modèles (5,1 Mo) sont versionnés avec leurs licences et intégrés à l'exécutable. « À propos » cite ONNX Runtime, YuNet, MediaPipe Face Mesh, et printpdf, oublié lors de l'export d'inventaire.
- Analyse version 2 :
  - l'image est lue à 2 048 px pour bien voir les visages ; le flou et l'exposition restent mesurés à 1 024 px ;
  - une photo est signalée « Yeux fermés probables » si un visage (cinq au plus, les plus grands, d'au moins 48 px) a les deux yeux sous 0,12 ;
  - les photos déjà analysées sont reprises ; sans la bibliothèque, l'analyse reste en version 1 et sera reprise plus tard.
- Échantillons de test : deux portraits CC0 de Wikimedia Commons, yeux ouverts et yeux fermés, ajoutés au manifeste avec leur source et leur empreinte (le script envoie maintenant un User-Agent, que Wikimedia exige).
- Tests : rapport d'ouverture, portraits réels (ouverts, fermés, sans visage), et de bout en bout, avec le portrait aux yeux fermés signalé « eyes » et filtré, l'autre non. Le prototype a montré la vitesse ; le critère des 1 000 photos se mesurera sur l'analyse complète, en version optimisée.
- Vérifié dans l'application compilée sous Windows, avec les boîtes de dialogue (données de l'utilisateur mises de côté puis remises, originaux d'essai intacts) :
  - les deux portraits d'essai ajoutés à la bibliothèque d'essai ; la réanalyse affiche son bilan dans une boîte d'Afterdusk ;
  - Photo > Rechercher les ratés reprend les onze photos (analyse version 2), avec la progression dans la barre d'état. Le portrait aux yeux fermés porte « Yeux fermés probables », l'autre rien ; les ratés déjà connus gardent leur pastille ;
  - la suppression de l'album intelligent demande confirmation dans une boîte d'Afterdusk (avertissement, « Supprimer » en rouge), et Échap annule.
- Corrigé à cette occasion : « Arrêter » passait à la ligne sous la progression, hors de la barre d'état.

### 2026-10-21 (suite) : V0.4, planches contact et logo du studio

- Mise en page PDF commune (`src-tauri/src/pdf.rs`) : formats A4 et Lettre, portrait ou paysage, texte coupé à la largeur, images cadrées, opacité, pied de page numéroté, écriture atomique. L'inventaire du matériel l'utilise aussi.
- Livrer > Planche contact… :
  - les photos de la grille, ou la sélection quand plusieurs photos sont choisies ; les fichiers du dessous d'une paire RAW + JPEG sont écartés, les copies virtuelles portent leur nom ;
  - titre de l'album ou de la collection, puis dates, photographe et nombre de photos (client et séance viendront avec les clients) ;
  - de 2 à 8 colonnes, A4 ou Lettre, portrait ou paysage, logo en tête, filigrane aucun, texte ou logo : réglages gardés d'une planche à l'autre ;
  - vignettes tirées des aperçus retouchés (600 px), progression dans la boîte ; 20 photos par page en 4 colonnes A4, 72 en 8 colonnes.
- Logo du studio (idée de l'utilisateur), dans Réglages > Identité : copié et vérifié dans les données d'Afterdusk, aperçu sur damier, remplaçable et retirable. Il sert en tête des planches contact et de l'inventaire du matériel, et comme filigrane des exports (« Logo du studio », à côté de texte et image) et des planches.
- Tests : pagination et filigranes de la planche, formats de page, copie et retrait du logo, boîte de dialogue (photos retenues, sélection, réglages envoyés). Une planche d'exemple de 26 photos, rendue puis regardée page par page, avec filigrane texte puis logo.

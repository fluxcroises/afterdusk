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
- [ ] Carte du monde des albums avec grappes, aperçu et filtre par zone
- [ ] Fond de carte simplifié hors ligne
- [ ] Inventaire alimenté automatiquement, fusion des doublons, masquage
- [ ] Fiches complètes, accessoires manuels, statistiques d'usage avec graphiques
- [ ] Journal d'entretien et rappels
- [ ] Export d'inventaire PDF et CSV
- [ ] Albums intelligents

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

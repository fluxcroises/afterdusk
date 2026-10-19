# Avancement

Un critère n'est coché qu'une fois testé sur Windows et macOS par GitHub Actions (CLAUDE.md).

## V0.1 : Importer et trier

- [x] Projet Tauri 2 qui se compile et se lance sur Windows et macOS, avec intégration continue GitHub Actions sur les deux systèmes
- [x] Première ouverture guidée (bibliothèque, sauvegarde, rangement, identité)
- [ ] Une carte SD insérée est détectée en moins de 5 secondes et l'import proposé
- [ ] Import de 500 RAW avec rangement, renommage, sauvegarde et vérification par somme de contrôle, sans aucune perte, en cas de coupure comprise (reprise propre au redémarrage)
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

# Avancement

Un critère n'est coché qu'une fois testé sur Windows et macOS par GitHub Actions (CLAUDE.md).

## V0.1 : Importer et trier

- [ ] Projet Tauri 2 qui se compile et se lance sur Windows et macOS, avec intégration continue GitHub Actions sur les deux systèmes
- [ ] Première ouverture guidée (bibliothèque, sauvegarde, rangement, identité)
- [ ] Une carte SD insérée est détectée en moins de 5 secondes et l'import proposé
- [ ] Import de 500 RAW avec rangement, renommage, sauvegarde et vérification par somme de contrôle, sans aucune perte, en cas de coupure comprise (reprise propre au redémarrage)
- [ ] Les photos déjà importées sont reconnues et grisées lors d'un second import
- [ ] Aperçus affichés dès la fin de la copie de chaque fichier, sans attendre la fin de l'import
- [ ] Passage d'une photo à la suivante en moins de 100 ms dans la visionneuse
- [x] Notes, drapeaux, couleurs au clavier, écrits en base et dans le XMP
- [x] Comparaison côte à côte de 2 à 4 photos avec zoom synchronisé
- [ ] Corbeille interne avec restauration et vidage après 30 jours
- [ ] Vue Bibliothèque avec albums, filtres de base (date, note, drapeau, boîtier, objectif) et recherche
- [ ] Bibliothèque sur disque externe débranché consultable via le cache
- [ ] RAW testés au minimum : Fujifilm (RAF X-Trans), Canon (CR3), Nikon (NEF), Sony (ARW), Panasonic (RW2), DNG
- [ ] Interface en français et en anglais

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

# Vérification sur Mac

Session à faire une fois sur le Mac de l'utilisateur (décidé le 2026-10-19) : les mesures que la CI ne peut pas faire et les vérifications à l'écran reportées. Les résultats vont dans `docs/avancement.md` ; un critère se coche quand sa mesure Mac est sous le seuil.

Noter d'abord la machine : modèle, puce (Apple Silicon ou Intel), mémoire, processeur graphique, version de macOS.

Machine de la session (2026-10-23) : MacBook Pro 16 pouces (Mac15,7), puce Apple M3 Pro (Apple Silicon, 12 cœurs : 6 performance et 6 efficacité), 18 Go, processeur graphique M3 Pro à 18 cœurs (Metal 4), écran Liquid Retina XDR 3456 × 2234, macOS 26.6.2 (25G83).

## 1. Installation

Dans le Terminal :

```bash
xcode-select --install
```

```bash
curl https://sh.rustup.rs -sSf | sh
```

Installer Node.js (paquet LTS de nodejs.org), puis :

```bash
git clone --recursive https://github.com/lewisbedar/afterdusk-app.git && cd afterdusk-app && npm ci
```

```bash
node scripts/fetch-test-assets.mjs && node scripts/fetch-ffmpeg.mjs && node scripts/fetch-onnxruntime.mjs
```

Les trois scripts vérifient les empreintes SHA-256 de ce qu'ils téléchargent (RAW de raw.pixls.us, FFmpeg 9.0.2, ONNX Runtime 1.23.2).

Contrôle : `npm test` et `cargo test` (dans `src-tauri`) passent.

## 2. Mesures automatiques

```bash
cd src-tauri && cargo test --release --test perf_v02 --test perf_v04 -- --ignored --nocapture
```

| Critère                                       | Version | Seuil         | Windows (2026-10-23)                    | Mac (2026-10-23)                            |
| --------------------------------------------- | ------- | ------------- | --------------------------------------- | ------------------------------------------- |
| Retour visuel d'un réglage (p95)              | V0.2    | 100 ms        | 8 ms (RTX 3060 Ti), 64 ms au processeur | 6 ms (M3 Pro, Metal), 59 ms au processeur   |
| Export de 100 JPEG en 2048 px                 | V0.2    | 2 min         | 16 s                                    | 8,3 s (12 fils de calcul)                   |
| Analyse de 1 000 photos, depuis les aperçus   | V0.4    | 10 min        | 1,4 min                                 | 0,9 min                                     |
| Analyse de 1 000 photos, depuis les originaux | V0.4    | 10 min        | 2,2 min                                 | 1,4 min                                     |
| Navigation pendant l'analyse (p95)            | V0.4    | sans ralentir | 0,8 ms                                  | 0,3 ms (1,6 ms sans analyse, mesurée avant) |

La ligne « rendu : » en tête de la mesure dit quel processeur graphique est utilisé. Pour comparer, relancer la première mesure au processeur seul : `AFTERDUSK_GPU=0 cargo test --release --test perf_v02 retour_visuel -- --ignored --nocapture`.

## 3. Mesures à l'écran

Lancer l'application avec le compteur de la visionneuse :

```bash
VITE_PERF_OVERLAY=1 npx tauri dev --release
```

- **Visionneuse (V0.1, seuil 100 ms)** : dans Trier, ouvrir une photo en loupe, passer d'une photo à la suivante avec les flèches une vingtaine de fois, à un rythme normal puis vite. Relever le p95 du compteur en bas de la fenêtre. Windows : 21 ms (compilation de débogage). Mac (2026-10-23, application compilée avec le compteur, 8 photos de test-assets) : p95 33 ms, max 37 ms sur 23 passages ; 37 ms au rythme normal (11 passages).
- **Carte SD (V0.1, seuil 5 s)** : Afterdusk ouvert, insérer une carte d'appareil photo. Chronométrer jusqu'au bandeau « Carte … » qui propose l'import. Windows : 372 ms. Annuler sans importer, éjecter la carte : Afterdusk ne doit rien y avoir écrit. Mac (2026-10-23, carte d'un Sony, 738 fichiers dans DCIM) : environ 2 s de l'insertion au bandeau « Carte SONY (Untitled) détectée », chronométrés à la main (le montage par macOS en prend l'essentiel, le relevé des volumes est fait chaque seconde) ; deux insertions, annulées sans importer ; carte relevée avant et après (1 097 entrées, mêmes dates et tailles), rien d'écrit.

## 4. Vérifications à l'écran reportées

Cocher chaque point vu sur le Mac.

- [x] Lancement de l'application compilée (premier critère de la V0.1) : `npx tauri build`, puis ouvrir `src-tauri/target/release/bundle/macos/Afterdusk.app`. Mac : paquet de 414 Mo (FFmpeg 307 Mo, arm64 et Intel ; ONNX Runtime 72 Mo) et .dmg de 159 Mo, compilés avec `VITE_PERF_OVERLAY=1` pour la section 3 ; lancé et utilisé toute la session.
- [ ] Barre de titre transparente avec les pastilles, logo décalé, sans chevauchement (fenêtre principale et accueil). Mac : transparente et sans chevauchement sur les deux fenêtres ; **défaut** relevé par l'utilisateur sur l'accueil : le logo et « Afterdusk » sont centrés environ 5 points plus bas que les pastilles.
- [ ] Menu système : menu Afterdusk (À propos, Réglages ⌘,), Fichier, Édition, Photo, Affichage, Fenêtre, Aide, traduits ; Affichage > Interface suit le profil. Mac : menus et entrées d'Afterdusk en français, ⌘, ouvre les Réglages, Interface > Simplifiée masque modules, filtres et entrées du menu Photo. **Défaut** : les entrées ajoutées par macOS restent en anglais (« Quit and Keep Windows », « Close All », Writing Tools, AutoFill, Start Dictation, Emoji & Symbols), comme les boutons des fenêtres de choix de dossier (Cancel, Open, New Folder) : `Info.plist` déclare `CFBundleDevelopmentRegion = English` et aucune `CFBundleLocalizations`.
- [ ] Raccourcis ⌘ : ⌘Z dans le Retoucher (y compris case à cocher ou curseur ayant le focus), ⌘⌫ (corbeille), ⌘F (recherche). Mac : ⌘Z annule avec le focus sur le curseur Exposition et sur la case « Corriger l'objectif » ; ⌘⌫ envoie dans la corbeille interne (annulé ensuite). **Défaut** : ⌘F ne donne pas le focus au champ de recherche dans Trier (confirmé au clavier par l'utilisateur), alors qu'Édition > Rechercher… le fait.
- [ ] Second écran : Fenêtre > Second écran s'ouvre en plein écran sur le moniteur externe et suit la navigation. Mac (TV LG en AirPlay, écran étendu 1920 × 1080, recopie désactivée) : **défaut**, la fenêtre s'ouvre sur l'écran du Mac, à x = 864 (moitié de 1728, abscisse logique de la TV). `secondary_toggle` (`src-tauri/src/commands/windows.rs`) passe une `PhysicalPosition` relevée sur la TV (échelle 1), interprétée avec l'échelle 2 de l'écran Retina où la fenêtre est créée ; `set_fullscreen` agit ensuite sur l'écran du Mac. Suivi de la navigation non vérifié.
- [x] Sauvegarde vers iCloud Drive (ou OneDrive) : dossier proposé dans l'assistant, note « copie locale vérifiée, pas l'envoi en ligne ». Mac : iCloud Drive et OneDrive proposés, note affichée (choix retiré ensuite). Défaut : une fois choisi, le bouton « iCloud Drive » paraît grisé, comme désactivé.
- [x] Fenêtre « À propos » : licence, composants, copie des informations système.
- [x] Six RAW de test décodés en pleine définition (zoom 100 % dans le Retoucher) : RAF X-Trans, CR3, NEF, ARW, RW2, DNG. Mac : rendu à 100 % en 3 à 15 ms, sans artefact de dématriçage sur le RAF.
- [x] Retoucher : rendu par le processeur graphique d'Apple (Metal), histogramme cohérent avec l'image. Mac : « rendu … (Apple M3 Pro (Metal)) ». Défaut : dans Trier, le panneau Histogramme reste vide pour toutes les photos.
- [x] Export HEIF par l'encodeur du système (ImageIO), relu dans Aperçu avec ses métadonnées. Mac : ARW exporté en HEIC (HEVC, 2048 × 1368, sRGB, tuiles 512), EXIF complet (boîtier, objectif, exposition, date et fuseau, « Software = Afterdusk »).
- [x] Vidéos : HEVC lu sans version de lecture (lecteur natif), H.264, image par image ; dérushage et export sans réencodage. Mac : clip H.264 4K d'un Sony (carte) et 10 s réencodées en HEVC par VideoToolbox ; lecture native, aucune version de lecture créée ; extrait 8,72 s–18,24 s exporté en copie de flux (H.264 + PCM, 9,77 s), originaux identiques (SHA-256).
- [ ] HEIC d'un iPhone : vignette, ouverture dans le Retoucher, orientation correcte (portrait), export. Mac (iPhone 17 Pro Max, 5712 × 4284, Orientation 6) : vignette, Retoucher (original décodé en 4,4 s) et export fonctionnent, mais **défaut** : la photo est tournée d'un quart de tour (paysage) partout, export compris (JPEG 2048 × 1536, Orientation 1). FFmpeg 9.0.2 rend déjà le HEIF dans le bon sens (4284 × 5712), même avec `-noautorotate` ; `rendered::develop` applique ensuite `orient(img, 6)` une seconde fois. Fichier d'origine intact.
- [x] Dossier surveillé (FSEvents) : une photo copiée par le Finder apparaît dans Trier en moins de 5 s. Mac : HEIC collé par le Finder, en base 1,3 s après la copie, album « Séance en cours ».
- [ ] Réception FTP : macOS demande d'autoriser les connexions entrantes ; envoi depuis un boîtier ou un client FTP du réseau local. Mac : réception activée (écoute sur 2121), envoi FTPS par `curl` depuis le Mac lui-même, photo rangée dans l'album « Réception FTP » ; FTP en clair refusé (534) ; réception désactivée ensuite, port fermé. Restent : la demande de macOS (pare-feu désactivé sur ce Mac) et un envoi depuis un autre appareil du réseau.
- [x] Analyse des ratés (ONNX Runtime universel) : yeux fermés repérés sur les portraits d'essai. Mac : `visage_fermes.jpg` seul dans « Ratés probables », `visage_ouverts.jpg` non.
- [x] Polices de l'ordinateur dans l'éditeur de compositions (liste de macOS, texte rendu à l'export). Mac : Polaroid, légende en American Typewriter, exportée en 1040 × 1264 avec la bonne police.
- [x] Assistant de bienvenue : étape Usage sans barre de défilement.

## 5. Après la session

Consigner machine et résultats dans `docs/avancement.md`, cocher les critères tenus (cahier des charges et journal), et noter les défauts trouvés pour les corriger depuis Windows ou le Mac.

# Vérification sur Mac

Session à faire une fois sur le Mac de l'utilisateur (décidé le 2026-10-19) : les mesures que la CI ne peut pas faire et les vérifications à l'écran reportées. Les résultats vont dans `docs/avancement.md` ; un critère se coche quand sa mesure Mac est sous le seuil.

Noter d'abord la machine : modèle, puce (Apple Silicon ou Intel), mémoire, processeur graphique, version de macOS.

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
git clone https://github.com/lewisbedar/afterdusk-app.git && cd afterdusk-app && npm ci
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

| Critère                                       | Version | Seuil         | Windows (2026-10-23)                    | Mac |
| --------------------------------------------- | ------- | ------------- | --------------------------------------- | --- |
| Retour visuel d'un réglage (p95)              | V0.2    | 100 ms        | 8 ms (RTX 3060 Ti), 64 ms au processeur |     |
| Export de 100 JPEG en 2048 px                 | V0.2    | 2 min         | 16 s                                    |     |
| Analyse de 1 000 photos, depuis les aperçus   | V0.4    | 10 min        | 1,4 min                                 |     |
| Analyse de 1 000 photos, depuis les originaux | V0.4    | 10 min        | 2,2 min                                 |     |
| Navigation pendant l'analyse (p95)            | V0.4    | sans ralentir | 0,8 ms                                  |     |

La ligne « rendu : » en tête de la mesure dit quel processeur graphique est utilisé. Pour comparer, relancer la première mesure au processeur seul : `AFTERDUSK_GPU=0 cargo test --release --test perf_v02 retour_visuel -- --ignored --nocapture`.

## 3. Mesures à l'écran

Lancer l'application avec le compteur de la visionneuse :

```bash
VITE_PERF_OVERLAY=1 npx tauri dev --release
```

- **Visionneuse (V0.1, seuil 100 ms)** : dans Trier, ouvrir une photo en loupe, passer d'une photo à la suivante avec les flèches une vingtaine de fois, à un rythme normal puis vite. Relever le p95 du compteur en bas de la fenêtre. Windows : 21 ms (compilation de débogage).
- **Carte SD (V0.1, seuil 5 s)** : Afterdusk ouvert, insérer une carte d'appareil photo. Chronométrer jusqu'au bandeau « Carte … » qui propose l'import. Windows : 372 ms. Annuler sans importer, éjecter la carte : Afterdusk ne doit rien y avoir écrit.

## 4. Vérifications à l'écran reportées

Cocher chaque point vu sur le Mac.

- [ ] Lancement de l'application compilée (premier critère de la V0.1) : `npx tauri build`, puis ouvrir `src-tauri/target/release/bundle/macos/Afterdusk.app`.
- [ ] Barre de titre transparente avec les pastilles, logo décalé, sans chevauchement (fenêtre principale et accueil).
- [ ] Menu système : menu Afterdusk (À propos, Réglages ⌘,), Fichier, Édition, Photo, Affichage, Fenêtre, Aide, traduits ; Affichage > Interface suit le profil.
- [ ] Raccourcis ⌘ : ⌘Z dans le Retoucher (y compris case à cocher ou curseur ayant le focus), ⌘⌫ (corbeille), ⌘F (recherche).
- [ ] Second écran : Fenêtre > Second écran s'ouvre en plein écran sur le moniteur externe et suit la navigation.
- [ ] Sauvegarde vers iCloud Drive (ou OneDrive) : dossier proposé dans l'assistant, note « copie locale vérifiée, pas l'envoi en ligne ».
- [ ] Fenêtre « À propos » : licence, composants, copie des informations système.
- [ ] Six RAW de test décodés en pleine définition (zoom 100 % dans le Retoucher) : RAF X-Trans, CR3, NEF, ARW, RW2, DNG.
- [ ] Retoucher : rendu par le processeur graphique d'Apple (Metal), histogramme cohérent avec l'image.
- [ ] Export HEIF par l'encodeur du système (ImageIO), relu dans Aperçu avec ses métadonnées.
- [ ] Vidéos : HEVC lu sans version de lecture (lecteur natif), H.264, image par image ; dérushage et export sans réencodage.
- [ ] HEIC d'un iPhone : vignette, ouverture dans le Retoucher, orientation correcte (portrait), export.
- [ ] Dossier surveillé (FSEvents) : une photo copiée par le Finder apparaît dans Trier en moins de 5 s.
- [ ] Réception FTP : macOS demande d'autoriser les connexions entrantes ; envoi depuis un boîtier ou un client FTP du réseau local.
- [ ] Analyse des ratés (ONNX Runtime universel) : yeux fermés repérés sur les portraits d'essai.
- [ ] Polices de l'ordinateur dans l'éditeur de compositions (liste de macOS, texte rendu à l'export).
- [ ] Assistant de bienvenue : étape Usage sans barre de défilement.

## 5. Après la session

Consigner machine et résultats dans `docs/avancement.md`, cocher les critères tenus (cahier des charges et journal), et noter les défauts trouvés pour les corriger depuis Windows ou le Mac.

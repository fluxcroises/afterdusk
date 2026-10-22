# 0012. Vidéos : FFmpeg livré avec Afterdusk

Date : 2026-10-22 · Statut : **adoptée** le 2026-10-22, au choix de l'utilisateur (option A)

## Contexte

Le critère de la V0.4 demande pour les vidéos l'import, le rangement, le tri, la lecture et les fichiers annexes (cahier des charges, 4.9) : miniature, durée, résolution, cadence et codec affichés, lecture intégrée avec défilement image par image. Le dérushage de la V0.5 demandera en plus de découper et d'exporter.

Deux voies ont été comparées :

- **A. FFmpeg livré** : ffprobe lit les métadonnées et ffmpeg calcule les miniatures pour tous les formats des boîtiers (H.264, HEVC, ProRes, AVCHD), et produit une version de lecture H.264 quand le lecteur intégré ne sait pas lire l'original (HEVC sous Windows sans l'extension de Microsoft, ProRes, AVCHD). C'est aussi la base du dérushage.
- **B. Rust seul et lecteur du système** : métadonnées MP4 et MOV en Rust, miniatures du boîtier ou du lecteur intégré, aucune dépendance de plus ; mais pas de HEVC sous Windows sans extension payante, pas de ProRes ni d'AVCHD selon le système, et FFmpeg nécessaire en V0.5 de toute façon.

## Décision

**Option A.** FFmpeg **9.0.2**, ffmpeg et ffprobe utilisés comme programmes séparés, appelés par le cœur en arrière-plan.

- **Windows** : construction GPL en bibliothèques partagées de BtbN/FFmpeg-Builds, publication mensuelle conservée `autobuild-2026-09-30-13-08`, archive `ffmpeg-n9.0.2-17-g2a571b6068-win64-gpl-shared-9.0.zip`, SHA-256 `3da6c7b6…c022696` publié par GitHub.
- **macOS** : constructions statiques de Martin Riedl (ffmpeg.martin-riedl.de), Apple Silicon et Intel, réunies en binaires universels par `lipo` ; SHA-256 publiés à côté de chaque archive. Les constructions Intel régulières de ce site s'arrêtent en janvier 2027 : la version retenue est épinglée.
- **Récupération** : `scripts/fetch-ffmpeg.mjs`, comme pour ONNX Runtime (décision 0010) : archives vérifiées, binaires extraits dans `src-tauri/resources/ffmpeg/` (non versionné), livrés par Tauri ; la CI les récupère et les garde en cache. L'application ne télécharge rien.
- **Licence** : FFmpeg sous GPL version 2 ou ultérieure dans ces constructions (x264 compris, pour les versions de lecture) : compatible avec la GPL v3 d'Afterdusk. `NOTICE.txt` donne l'adresse du code source correspondant ; « À propos » cite FFmpeg.
- **Poids** : 183 Mo une fois installé sous Windows (dont 119 Mo pour avcodec), environ 60 à 70 Mo dans l'installateur. Une construction réduite aux décodeurs utiles pourra alléger plus tard.

## Conséquences

- Migration 0019 : le type de média vidéo, durée, cadence, codec, et les fichiers annexes des boîtiers.
- Sans FFmpeg (fichiers absents ou illisibles), les vidéos s'importent quand même (copie vérifiée) ; leurs informations et miniatures attendent que FFmpeg soit disponible.
- Échantillons de test : courtes vidéos générées par FFmpeg lui-même (mire de test), donc sans question de droits, versionnées avec les tests.

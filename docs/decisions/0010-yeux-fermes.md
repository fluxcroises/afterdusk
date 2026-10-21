# 0010. Yeux fermés : modèles et moteur ONNX

Date : 2026-10-21 · Statut : **adoptée** le 2026-10-21 : ort et ONNX Runtime, au choix de l'utilisateur

## Contexte

Le cahier des charges (4.3) demande de signaler les yeux fermés, entièrement en local. Il prévoyait « des modèles ONNX locaux (crate ort) » et laissait ouverts le choix des modèles et la vérification de leur licence (section 9). L'utilisateur a demandé de comparer deux moteurs, `ort` (ONNX Runtime) et `tract` (Rust pur), et autorisé les téléchargements du prototype.

## Méthode retenue

1. **YuNet** trouve les visages et place les deux yeux. Fichier `face_detection_yunet_2023mar.onnx`, 232 589 octets, licence MIT, dépôt officiel `opencv/opencv_zoo`, issu de libfacedetection (Shiqi Yu). Empreinte SHA-256 conforme au dépôt.
2. Chaque visage est recadré droit, les yeux à l'horizontale, en 256 × 256, carré de 1,5 fois la boîte du visage.
3. **Face Mesh V2** place 478 repères. Fichier `face_landmarks.onnx`, 4 920 979 octets, licence Apache 2.0, poids compris : conversion directe du modèle MediaPipe de Google, avec script de conversion et preuve de parité numérique. Il vient du dépôt Hugging Face `fernandotonon/QtMeshEditor-facemesh-onnx`, une conversion communautaire et non une publication de Google. Empreinte SHA-256 conforme à l'identifiant LFS publié.
4. Pour chaque œil, on calcule le rapport d'ouverture (EAR) : hauteur des paupières sur largeur de l'œil, sur six repères. **Yeux fermés probables si les deux yeux sont sous 0,12.**

## Prototype

Onze photos libres de Wikimedia Commons (CC0 et domaine public), notées à la main :

| Photo              | Attendu                           | EAR (droit / gauche)      | Résultat                                                     |
| ------------------ | --------------------------------- | ------------------------- | ------------------------------------------------------------ |
| p01, p02, p06, p12 | ouverts                           | 0,24 à 0,38               | ouverts                                                      |
| p08                | lunettes de soleil                | 0,26 / 0,30               | ouverts, pas de faux signal                                  |
| p11                | regard baissé                     | 0,25 / 0,56               | ouverts                                                      |
| p10                | rire, petit visage (194 px)       | 0,13 / 0,17               | ouverts au seuil de 0,12 (un seuil à 0,18 l'aurait signalée) |
| p04, p05           | fermés                            | 0,10 / 0,05 ; 0,03 / 0,03 | fermés                                                       |
| p07                | endormie de profil, noir et blanc | aucun visage trouvé       | manquée                                                      |
| p03                | pas de visage                     | aucun visage              | rien                                                         |

Deux yeux fermés trouvés sur trois, aucune fausse alerte. Les deux moteurs donnent les mêmes valeurs au millième près.

| Moteur                                       | Détection        | Repères          | 1 000 photos (un visage) | Ajout au logiciel                                                                                                              |
| -------------------------------------------- | ---------------- | ---------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------------------------ |
| tract 0.23.9 (Rust pur, MIT ou Apache 2.0)   | 105 ms par photo | 34 ms par visage | environ 2 min 20         | environ 23 Mo dans l'exécutable, rien à livrer à côté                                                                          |
| ort 2.0.0-rc.15 et ONNX Runtime 1.31.0 (MIT) | 49 ms par photo  | 6 ms par visage  | environ 1 min            | une bibliothèque native par système : 17 Mo sous Windows ; la publication 1.31.0 ne propose que macOS arm64 (pas de Mac Intel) |

Mesures faites sur un seul fil, en version optimisée, sur la machine de développement, à partir de vignettes JPEG de 2 000 px.

## Décision

La proposition était tract : rien de natif à livrer, assez rapide. **L'utilisateur a retenu ort**, plus rapide, « malgré la bibliothèque native à livrer par système ».

- **Version** : ONNX Runtime **1.23.2**, et non 1.31.0, parce que la 1.23.2 existe en bibliothèque universelle pour Mac (Apple Silicon et Intel), la 1.31.0 en Apple Silicon seulement. Même version sous Windows. D'abord 1.22.0 : sous macOS 26, elle plantait (SIGSEGV) à l'ouverture de la première session ; la 1.23.2 passe sur la CI macOS. L'utilisateur accepterait d'abandonner les Mac Intel si besoin ; ce n'est pas nécessaire pour l'instant. Le prototype donne les mêmes valeurs (49 ms par photo, 8 ms par visage). `ort` 2.0.0-rc.15, fonctions `std`, `load-dynamic` et `api-22`, sans téléchargement par la crate.
- **Poids** : `onnxruntime.dll`, 14,2 Mo sous Windows ; `libonnxruntime.dylib` universelle, 74,9 Mo sous macOS. C'est le prix des Mac Intel. Si on les abandonne un jour, la version arm64 seule pèse environ la moitié.
- **Récupération** : `scripts/fetch-onnxruntime.mjs` télécharge l'archive officielle de Microsoft (publication GitHub v1.23.2), vérifie son SHA-256 et en extrait la bibliothèque, `LICENSE` et `ThirdPartyNotices.txt` dans `src-tauri/resources/onnxruntime/`. Ce dossier n'est pas versionné ; Tauri le livre avec l'application. Empreintes publiées par GitHub pour cette version : Windows x64 `0b38df9a…9f3cd`, macOS universel `49ae8e3a…85809`. La CI récupère la bibliothèque et la garde en cache. L'application ne télécharge rien.
- **Modèles** : les deux fichiers ONNX (5,1 Mo) sont versionnés dans `src-tauri/assets/models/` avec leurs licences et une notice, et intégrés à l'exécutable.
- **Sans la bibliothèque** (fichier absent ou illisible) : les yeux ne sont pas analysés. Le flou et l'exposition le sont, en version 1 de l'analyse, reprise quand la bibliothèque sera là.
- Les deux moteurs ayant donné les mêmes valeurs, passer à tract resterait possible sans toucher aux modèles.

## Limites connues

- Un visage de profil, très petit ou dans l'ombre peut ne pas être trouvé : la photo n'est alors pas signalée, jamais signalée à tort.
- Un rire franc plisse les yeux. Le seuil de 0,12 sur les deux yeux l'évite sur l'échantillon, mais un œil mi-clos de près pourra passer pour fermé. C'est un raté « probable », jamais rejeté seul.
- Face Mesh est une conversion communautaire. Si une version officielle en ONNX paraît, elle la remplacera.

# 0001 — Liaison LibRaw

Date : 2026-10-18 · Statut : adoptée

## Contexte

Le cahier des charges retient LibRaw pour décoder les RAW de tous les boîtiers (section 3). Il faut une liaison Rust qui compile sans dépendance système sur Windows et macOS, en CI comme sur le poste du développeur.

## Décision

- **LibRaw 0.22.2** en sous-module git (`src-tauri/libraw-sys/LibRaw`), compilé depuis les sources par la crate `cc` dans `src-tauri/libraw-sys/build.rs`. La liste des sources reprend `LIB_OBJECTS` de `Makefile.dist`. Pas de vcpkg, Homebrew ni bindgen (pas de libclang à installer).
- **Une couche C++ minimale** (`shim/afterdusk_raw.cpp`) expose une API C à disposition fixe : métadonnées, balises EXIF déjà mises en texte, liste des aperçus intégrés, extraction d'un aperçu. Rust ne dépend donc pas de la disposition de `libraw_data_t`, qui change entre versions.
- **Module sûr** `src-tauri/src/raw/` (`RawFile`) au-dessus de la FFI, erreurs `thiserror`.
- Les EXIF sont lus par LibRaw (rappel `exif_parser_callback`) : pas de seconde bibliothèque pour lire les RAW.
- Sous Windows, les chemins Unicode passent par `open_file(const wchar_t*)`.
- Licence : LibRaw est sous LGPL 2.1 ou CDDL 1.0 ; on l'utilise sous LGPL 2.1, compatible GPL v3.

## Résultats du prototype

`cargo test --test raw_samples` sur les échantillons de `test-assets/` (Windows 11, MSVC, profil release pour les temps) :

| Format           | Boîtier         | Aperçu retenu    | EXIF relevées | Remarques                                              |
| ---------------- | --------------- | ---------------- | ------------- | ------------------------------------------------------ |
| RAF (X-Trans IV) | Fujifilm X-T3   | JPEG 4416 × 2944 | 42            | X-Trans détecté, objectif XF35mmF2 R WR                |
| CR3              | Canon EOS R6    | JPEG 5472 × 3648 | 52            | dimensions non déclarées par LibRaw, lues dans le JPEG |
| NEF              | Nikon Z 6       | JPEG 6048 × 4024 | 66            | 4 aperçus intégrés                                     |
| ARW              | Sony ILCE-7M3   | JPEG 1616 × 1080 | 60            | pas de numéro de série dans l'EXIF                     |
| RW2              | Panasonic DC-G9 | JPEG 1920 × 1440 | 146           | fichier en mode haute résolution (80 Mpx)              |
| DNG              | Ricoh GR III    | JPEG 6000 × 4000 | 60            | DNG natif du boîtier                                   |

Ouverture et extraction de l'aperçu : 1 à 3 ms par fichier (fichier en cache disque). Tous donnent marque, modèle, ISO, vitesse, ouverture, focale, date de prise de vue ; le décalage horaire (`OffsetTimeOriginal`) est présent sur CR3, NEF, ARW et RW2.

Tests de règle absolue : la lecture ne change ni la somme SHA-256 ni la date de modification des originaux ; un chemin avec accents, tiret long et idéogrammes s'ouvre sous Windows.

## Conséquences

- **Aperçus plus petits que 2048 px** (ARW 1616 px, RW2 1920 px) : le palier 2048 du cache demandera un développement réduit en arrière-plan (`dcraw_process` en demi-taille), à prévoir avec le cache d'aperçus.
- Le choix de l'aperçu se fait par la taille en octets parmi les JPEG, car les dimensions déclarées manquent parfois (CR3).
- La compilation de LibRaw ajoute environ une minute au premier build ; elle est mise en cache ensuite (`rust-cache` en CI).
- Support JPEG XL, zlib (DNG compressés en deflate) et Adobe DNG SDK désactivés pour l'instant ; à réévaluer si un échantillon les exige.
- Les EXIF des JPEG, HEIF et TIFF (non RAW) ne passent pas par LibRaw : choix à faire au moment de l'import de ces formats (par exemple `kamadak-exif`, BSD-2).

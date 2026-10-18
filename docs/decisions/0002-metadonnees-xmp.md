# 0002 — Écriture XMP : xmp-toolkit-rs plutôt qu'Exiv2

Date : 2026-10-18 · Statut : adoptée pour la bibliothèque ; nommage et espace de noms à valider

## Contexte

Le cahier des charges laisse le choix entre Exiv2 et xmp-toolkit-rs (sections 3 et 9). Les notes, drapeaux, couleurs, mots-clés et retouches doivent être écrits dans un XMP **à côté** de l'original (jamais dans l'original), lisible par les autres logiciels.

## Options

| Critère                 | Exiv2                                                                         | xmp-toolkit-rs (`xmp_toolkit`)                                 |
| ----------------------- | ----------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Licence                 | GPL 2 ou ultérieure                                                           | MIT ou Apache 2.0 ; XMP Toolkit SDK d'Adobe en BSD 3           |
| Liaison Rust            | `rexiv2`, via gexiv2 et GLib                                                  | crate maintenue par Adobe (Content Authenticity Initiative)    |
| Compilation             | CMake + expat + zlib (+ brotli, inih) ; GLib à embarquer sur Windows et macOS | `cc` seul, sources incluses dans la crate                      |
| Lecture EXIF des RAW    | Très complète (y compris CR3 en BMFF)                                         | Non prévue                                                     |
| Écriture de XMP annexes | Oui                                                                           | Oui, c'est le cœur de la bibliothèque, par l'éditeur du format |

## Décision

**xmp-toolkit-rs** pour lire et écrire les XMP annexes. La lecture des EXIF des RAW reste confiée à LibRaw (décision 0001), ce qui retire le principal avantage d'Exiv2. On évite ainsi GLib et CMake, et le prototype a compilé du premier coup sous Windows.

Exiv2 n'a pas été prototypé : la chaîne gexiv2 + GLib sous Windows suffisait à l'écarter. On y reviendra s'il faut un jour lire des métadonnées que ni LibRaw ni une crate EXIF ne savent lire.

## Prototype

`src-tauri/src/metadata/xmp.rs`, tests unitaires et `tests/xmp_sidecar.rs` :

- écriture atomique (fichier `.tmp` puis renommage) ; refus d'écrire tout chemin qui n'a pas l'extension `.xmp` ;
- un XMP existant est relu et complété : les propriétés des autres logiciels (par exemple `crs:Exposure2012` de Lightroom) sont conservées ;
- aller-retour vérifié pour la note, le drapeau, la couleur, les mots-clés, l'auteur, le copyright et le nom d'origine ;
- écrire le XMP à côté d'un vrai RAF ne change ni sa somme SHA-256 ni sa date de modification.

Correspondance des champs :

| Afterdusk               | XMP                                                      | Lu par                                             |
| ----------------------- | -------------------------------------------------------- | -------------------------------------------------- |
| Note 0 à 5              | `xmp:Rating`                                             | Lightroom, Bridge, Capture One, darktable, digiKam |
| Couleur                 | `xmp:Label` (`Red`, `Yellow`, `Green`, `Blue`, `Purple`) | Lightroom, Bridge, darktable                       |
| Mots-clés               | `dc:subject`                                             | tous                                               |
| Auteur, copyright       | `dc:creator`, `dc:rights`                                | tous                                               |
| Nom sur la carte        | `xmpMM:PreservedFileName`                                | Lightroom, Bridge                                  |
| Drapeau retenue/rejetée | `afterdusk:Pick` (1 ou -1)                               | Afterdusk ; aucun champ standard n'existe          |

On n'écrit pas `xmp:Rating = -1` pour un rejet (convention Bridge et darktable) pour ne pas perdre la note ; à la lecture, un `-1` venant d'un autre logiciel est compris comme un rejet.

## Points à valider (difficiles à défaire)

1. **Nommage des annexes**. Les deux conventions sont codées (`SidecarNaming`) :
   - `DSCF0001.xmp` : Lightroom, Bridge, Capture One. Collision quand un RAW et un JPEG partagent le même nom.
   - `DSCF0001.RAF.xmp` : darktable, digiKam. Pas de collision, mais Lightroom ne le lit pas.

   Proposition : `DSCF0001.xmp` pour les RAW (compatibilité avec Lightroom, d'où viennent la plupart des utilisateurs visés) et `DSCF0001.JPG.xmp` pour les autres formats, ce qui évite la collision RAW + JPEG.

2. **Espace de noms** `https://afterdusk.app/ns/xmp/1.0/` (préfixe `afterdusk`). L'adresse n'a pas besoin d'exister, mais une fois écrite dans des fichiers on ne la change plus ; à confirmer avec le nom de domaine retenu (section 9 du cahier des charges).

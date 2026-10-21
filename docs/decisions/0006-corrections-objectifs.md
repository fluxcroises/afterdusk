# 0006 — Corrections d'objectifs

Date : 2026-10-20 · Statut : **adoptée** (choix de l'utilisateur le 2026-10-20)

## Contexte

Le cahier des charges demande en V0.2 des « corrections d'objectifs automatiques via lensfun » (distorsion, vignetage, aberrations chromatiques), appliquées par défaut quand l'objectif est reconnu. La décision 0004 avait retenu la base lensfun lue en Rust (`lensdb` de LightCraft), plutôt que la bibliothèque C++ lensfun, qui dépend de GLib (compilation lourde sous Windows et macOS).

Entre-temps, un portage Rust complet de lensfun a été publié : la crate `lensfun` 0.7.0 (avril 2026).

## Options examinées

| Option                              | Pour                                                                                                                                                                          | Contre                                                                         |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| **Crate `lensfun` 0.7.0** (retenue) | Rust pur, sans dépendance C ; même calcul que lensfun (vérifié sur 1 640 cas contre l'original C++) ; base incluse ; dépendances permissives (`flate2`, `regex`, `roxmltree`) | Jeune (version bêta, un seul auteur)                                           |
| Notre portage de la base (`lensdb`) | Contrôle complet                                                                                                                                                              | Plusieurs jours de travail, erreurs possibles là où la crate est déjà vérifiée |
| Bibliothèque C++ lensfun            | Référence                                                                                                                                                                     | GLib : compilation fragile sous Windows et macOS, binaires plus gros           |

## Décision

- **Crate `lensfun`, version figée (`=0.7.0`)**. Code LGPL-3.0-or-later, compatible avec la GPL v3 d'Afterdusk. Le code est petit et lisible : si la crate était abandonnée, on la reprendrait dans le dépôt.
- **Base lensfun** (CC BY-SA 3.0, contributeurs lensfun) incluse dans l'application, jamais téléchargée ; créditée dans « À propos » avec le portage.
- **Reconnaissance** : boîtier puis objectif d'après les EXIF. Sans nom d'objectif, seul l'objectif propre à un compact (sans complément optique « + GW-4 ») est retenu ; une focale hors de celles de l'objectif écarte le profil. Pas de profil : pas de correction (jamais un profil approchant). Les six échantillons : Canon, Fujifilm, Nikon et Sony reconnus ; Panasonic (fichier sans nom d'objectif) et Ricoh GR III (la base n'a que le profil avec complément GW-4) sans correction.
- **Pipeline** : vignetage sur l'image linéaire décodée ; distorsion et aberrations chromatiques en un seul rééchantillonnage, par canal ; agrandissement automatique juste suffisant pour qu'aucun bord ne reste vide (fonction des seules proportions, donc identique à l'aperçu, au zoom et à l'export). L'image corrigée est le repère des corrections de défauts et de la géométrie.
- **Réglages** : section `lens` (`enabled`, `distortion` et `vignetting` de 0 à 200 % du profil), absente quand elle vaut ses défauts : corrections actives à 100 %. Les photos déjà retouchées en V0.2 (aucune n'est publiée) voient leur rendu changer une fois ; à partir de la publication de la V0.2, ce comportement est figé dans le processus 1.

## À reprendre plus tard

- Corrections enregistrées par le boîtier (opcodes DNG, notes du fabricant Sony, Fujifilm, Panasonic), quand lensfun ne connaît pas l'objectif : c'est le cas du Ricoh GR III et des objectifs Micro 4/3.
- Choix manuel de l'objectif dans la base.

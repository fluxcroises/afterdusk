# 0005 — Format des réglages de retouche et pipeline de développement

Date : 2026-10-19 · Statut : **adoptée** (les cinq recommandations validées par l'utilisateur le 2026-10-19)

## Contexte

La V0.2 commence par deux fondations dont tout le reste dépend :

- le **format des réglages** : il est écrit dans la base et dans le XMP de chaque photo (§2 et §5 du cahier des charges, règle « réglages en JSON versionné ; toute évolution du format prévoit une migration »). Une fois des milliers de XMP écrits, on ne le change plus sans migration : c'est une décision difficile à défaire ;
- le **pipeline de développement** : il transforme le RAW en image, d'abord avec un **rendu neutre fidèle au boîtier** (premier critère de la V0.2), puis avec tous les réglages de la section 4.5, en moins de 100 ms sur un aperçu de 2048 px.

Décision 0004 : on écrit notre pipeline (LightCraft sert d'inspiration), on reprend `color` et `lensdb` avec attribution, on ajoute un champ « processus » distinct de `version`, LibRaw reste le seul décodeur.

## 1. Format des réglages

### Un objet JSON, creux, par sections

```json
{
  "version": 1,
  "process": 1,
  "light": { "exposure": 0.35, "contrast": 10, "highlights": -40, "shadows": 25 },
  "color": {
    "whiteBalance": { "mode": "custom", "temperature": 5600, "tint": 4 },
    "vibrance": 15
  },
  "geometry": { "crop": { "x": 0.05, "y": 0, "w": 0.9, "h": 1 }, "angle": -1.2 }
}
```

- **`version`** décrit la forme du JSON. Elle change quand un champ change de nom, d'unité ou de structure, et chaque changement s'accompagne d'une migration (lecture d'une ancienne version → forme actuelle), testée.
- **`process`** fige le rendu : le moteur garde le calcul de chaque processus. Une photo retouchée en processus 1 garde exactement son aspect même après une évolution du moteur ; les nouvelles photos prennent le dernier processus ; l'utilisateur peut passer une photo au processus récent, explicitement (comme Lightroom et Capture One).
- **Creux** : seuls les réglages modifiés sont écrits ; une valeur absente vaut sa valeur par défaut, fixée par le processus. Conséquences utiles : un preset ne contient que ce qu'il règle ; copier-coller et synchronisation transmettent des sections choisies ; l'intensité d'un preset (0 à 100 %) s'applique valeur par valeur ; le XMP reste court.
- **Sections** dans l'ordre de l'interface : `light`, `color`, `curve`, `hsl`, `detail`, `effects`, `geometry`, `lens`. La V0.2 les remplit au fil des critères ; la première étape ne livre que `light.exposure` et la balance des blancs « telle que prise », pour éprouver la chaîne complète.

### Unités : celles des logiciels habituels

| Réglage                                                                 | Unité, plage                                                              | Défaut        |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------- | ------------- |
| Exposition                                                              | IL, -5 à +5                                                               | 0             |
| Contraste, hautes lumières, ombres, blancs, noirs, vibrance, saturation | -100 à +100                                                               | 0             |
| Balance des blancs                                                      | `asShot`, préréglage, ou kelvins (2 000 à 50 000) et teinte (-150 à +150) | `asShot`      |
| Recadrage                                                               | fractions de l'image (0 à 1), après redressement                          | image entière |
| Angle                                                                   | degrés, -45 à +45                                                         | 0             |

Des plages identiques à celles de Lightroom rendent les presets et les habitudes transposables, sans promettre un rendu identique.

### Où il est stocké

- **Base** (migration 0004) : tables `edits` (une version par photo : l'originale et les copies virtuelles), `edit_history` (chaque étape, instantanés nommés) et `presets`, comme prévu au §5 du cahier des charges.
- **XMP** : le JSON de chaque version, dans notre espace de noms (`afterdusk:Develop`, une entrée par copie virtuelle), à côté des notes déjà écrites. C'est ce qui permet de reconstruire les retouches si la base est perdue (critère de la V0.2).
- **Pas d'écriture des réglages Lightroom (`crs:`)** dans le XMP : Lightroom les appliquerait avec son propre moteur, et la photo n'y aurait pas le même aspect qu'ici, sans que rien ne le signale. La lecture des réglages Lightroom, pour reprendre un catalogue, pourra venir plus tard.

## 2. Pipeline de développement

### Chaîne

1. **Décodage** par LibRaw : données du capteur, dématriçage (AHD pour les capteurs Bayer, l'algorithme propre au X-Trans pour Fujifilm), sans aucune retouche de LibRaw (ni courbe, ni luminosité automatique). La couche C actuelle n'expose que les aperçus et les métadonnées : on y ajoute l'accès à l'image dématriçée et à la matrice couleur du boîtier.
2. **Balance des blancs** « telle que prise » : coefficients du boîtier, lus par LibRaw.
3. **Couleur** : matrice du boîtier vers XYZ, puis espace de travail **linéaire Rec. 2020 en virgule flottante** (large gamut, comme le cahier des charges le prévoit). Code `color` de LightCraft repris ici, avec attribution.
4. **Réglages** dans l'espace linéaire (exposition, balance, puis le reste au fil de la V0.2).
5. **Rendu neutre** : courbe de tonalité de base douce, qui donne une luminosité proche du JPEG du boîtier, sans saturation ajoutée ; mesurée sur nos six échantillons en comparant au JPEG intégré.
6. **Sortie** : sRGB pour l'écran et l'export courant, Display P3 pour les écrans qui l'affichent.

### Processeur et carte graphique

- **Une référence processeur en Rust** pour chaque étape, et son **jumeau WGSL** exécuté par `wgpu` (Vulkan, DirectX 12, Metal). Des tests comparent les deux sur nos échantillons, avec une tolérance chiffrée. Sans carte graphique utilisable, on retombe sur le processeur.
- **Aperçu de retouche** : l'image dématriçée est gardée en mémoire graphique à la taille de l'écran (2048 px au plus) ; chaque mouvement de curseur ne relance que les étapes 4 à 6. L'export recalcule à pleine définition.

### Affichage dans l'interface

L'interface est une page web (Tauri). Deux façons de lui montrer l'image :

| Option                                             | Principe                                                                                                                         | Pour                                                                                         | Contre                                                                                            |
| -------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| **A. Le cœur rend, l'interface affiche** (proposé) | `wgpu` calcule l'image ; ses pixels sont lus et envoyés à l'interface par notre protocole `preview://`, affichés dans un canevas | Un seul moteur, le même pour l'aperçu et l'export : ce que l'on voit est ce que l'on exporte | Un aller-retour de quelques mégaoctets par image ; à mesurer (objectif : moins de 100 ms en tout) |
| B. L'interface rend                                | Les mêmes shaders exécutés par WebGPU dans la page                                                                               | Pas de transfert d'image                                                                     | Deux moteurs à tenir identiques ; WebGPU inégal selon les versions de macOS                       |

On commence par A et on mesure. Si le transfert est trop lent, on réduit la taille envoyée (taille réelle de la vue) avant d'envisager B.

### Mesure de la performance

La machine de développement a une carte graphique NVIDIA RTX 3060 Ti, plus rapide que la « carte intégrée récente » du cahier des charges. On mesurera aussi en forçant le processeur (pire cas), et la mesure sur une carte intégrée (un portable, le Mac) restera à faire avant de cocher le critère des 100 ms.

## Première étape livrée

- Migration 0004 (`edits`, `edit_history`, `presets`), type `DevelopSettings` (versions, processus, migrations, valeurs par défaut), lecture et écriture dans la base et le XMP, tests d'aller-retour.
- Décodage complet par LibRaw ; pipeline processeur et `wgpu` jusqu'au rendu neutre, avec exposition et balance « telle que prise ».
- Écran Retoucher : la photo active en rendu neutre, un curseur d'exposition pour éprouver la chaîne de bout en bout, mesure du temps de rendu.
- Tests : rendu neutre comparé au JPEG intégré sur les six formats, référence processeur et GPU dans la tolérance, aucun original modifié.

## Questions à trancher

1. **Format creux par sections**, avec `version` et `process` : d'accord ?
2. **Unités et plages de Lightroom** pour les curseurs : d'accord ?
3. **Pas de réglages `crs:` écrits dans le XMP** (Lightroom ne verra que notes, drapeaux, couleurs et mots-clés, pas les retouches) : d'accord ?
4. **Affichage par le cœur (option A)**, avec mesure avant toute autre piste : d'accord ?
5. **Espace de travail linéaire Rec. 2020** : d'accord ? (ProPhoto, plus large, est l'autre choix courant ; Rec. 2020 suffit pour les couleurs des capteurs et se convertit plus simplement vers les écrans.)

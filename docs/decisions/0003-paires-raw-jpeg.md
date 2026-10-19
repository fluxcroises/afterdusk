# 0003 — Paires RAW + JPEG regroupées en une seule photo

Date : 2026-10-19 · Statut : **proposée**, à valider avant tout code

## Contexte

Beaucoup de photographes déclenchent en RAW + JPEG (ou RAW + HEIF) : le boîtier écrit `DSCF0001.RAF` et `DSCF0001.JPG` pour le même déclenchement. Aujourd'hui Afterdusk importe deux photos indépendantes : deux miniatures côte à côte, deux notes à poser, et une suppression qui peut séparer les deux fichiers.

Les logiciels concurrents :

- **Lightroom** traite la paire comme une seule photo : le JPEG est un fichier compagnon, invisible dans la grille. Une option permet de les importer séparément.
- **Capture One** les présente comme deux variantes empilées, que l'on peut déplier.

Le cahier des charges ne mentionne pas les paires. Il prévoit en revanche des **rafales empilées** en V0.4 (§4.3), qui regroupent plusieurs déclenchements. Une paire, elle, est un seul déclenchement. Les deux notions restent donc distinctes, même si l'interface peut les afficher de la même façon.

## Proposition

### Une photo, deux fichiers

Le RAW est la photo. Le JPEG (ou HEIF) devient son **compagnon** :

- il est importé, copié, vérifié par somme de contrôle et sauvegardé comme tout fichier ;
- il n'apparaît pas comme une photo distincte dans les grilles ni dans les compteurs ;
- une pastille « RAW+JPEG » sur la miniature signale la paire, et la fiche d'informations nomme les deux fichiers.

**Modèle de données.** On ajoute une colonne `companion_of INTEGER REFERENCES media(id)` sur `media`. La ligne du JPEG pointe vers celle du RAW, et les grilles listent `companion_of IS NULL`.

Option écartée : une seule ligne `media` avec une table `media_files` (un fichier par rôle). Ce modèle est plus juste, mais il oblige à reprendre tout ce qui manipule un chemin : import, aperçus, XMP, corbeille, protocole `preview://`. Avec la colonne, chaque fichier garde sa ligne et son traitement actuel, déjà testé.

### Reconnaissance des paires, à l'import

Deux fichiers forment une paire si :

1. ils sont dans le **même dossier** de la source ;
2. ils ont le **même nom sans extension**, sans tenir compte de la casse ;
3. l'un est un RAW, l'autre un JPEG ou un HEIF (`jpg`, `jpeg`, `heic`, `heif`, `hif`) ;
4. leurs **heures de prise de vue** sont égales à 2 secondes près.

La règle 4 évite une fausse paire quand le compteur du boîtier est repassé par le même numéro. Si un critère manque, les fichiers restent deux photos indépendantes, comme aujourd'hui.

### Ce qui s'applique à la paire

| Action                              | Effet                                                                                                                                                                                                                                                        |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Note, drapeau, couleur, mots-clés   | Écrits en base sur le RAW et **dans les deux XMP** (`DSCF0001.xmp` et `DSCF0001.JPG.xmp`, qui ne se gênent pas : décision 0002), pour que tout autre logiciel voie la même note.                                                                             |
| Corbeille, restauration, vidage     | Toujours les deux fichiers ensemble.                                                                                                                                                                                                                         |
| Renommage à l'import                | Un seul numéro `{seq}` pour la paire : `20261019_…_0042.RAF` et `20261019_…_0042.JPG`. Aujourd'hui chaque fichier prend un numéro, ce qui séparerait la paire.                                                                                               |
| Aperçus                             | Construits à partir du JPEG du boîtier quand il est plus grand que l'aperçu intégré au RAW. C'est le cas sur Sony (aperçu de 1616 px) et Panasonic (1920 px) : l'aperçu 2048 px n'aurait plus besoin d'un développement réduit, et le zoom 100 % serait net. |
| Filtres et recherche                | La paire compte pour une photo ; la recherche par nom de fichier trouve l'un ou l'autre.                                                                                                                                                                     |
| Doublons                            | Inchangé : chaque fichier a son empreinte. Une paire déjà importée est grisée entière.                                                                                                                                                                       |
| Retouche (V0.2) et livraison (V0.3) | Partent du RAW. Le JPEG du boîtier n'est jamais retouché.                                                                                                                                                                                                    |

### Écran d'import

- La source affiche **une vignette par paire**, avec sa pastille, et le compte « 120 photos (240 fichiers) ».
- Les cases à cocher sélectionnent la paire entière.

### Bibliothèques existantes

Une migration numérotée (`0003`) ajoute la colonne, puis relie les paires déjà importées selon les mêmes critères (même dossier d'album, même nom, même heure). Aucun fichier n'est déplacé ni modifié : seule la base change, et une nouvelle migration pourrait annuler l'association.

### Tests prévus

- Reconnaissance sur des paires synthétiques : même nom avec une casse différente, heures écartées de plus de 2 s, JPEG sans RAW, RAW + HEIF.
- Règle absolue : la corbeille déplace les deux fichiers et les restaure à l'identique (empreintes).
- Les deux XMP reçoivent la même note.
- Renommage : un seul numéro pour la paire.
- Migration sur une base contenant des paires déjà importées.

## Questions à trancher

1. **Le principe** : le JPEG compagnon caché derrière le RAW, comme Lightroom (proposé), ou une pile dépliable qui montre les deux, comme Capture One ? La pile ajoute un clic pour voir le JPEG. Elle serait utile surtout pour comparer le rendu du boîtier au RAW, ce que la comparaison (C) ferait aussi si l'on ajoute « Comparer avec le JPEG du boîtier ».
2. **Option d'import** « JPEG des paires : importer (par défaut) / ne pas importer ». Certains déclenchent en RAW + JPEG uniquement pour la carte de secours ou pour l'envoi depuis le boîtier, et ne veulent pas des JPEG sur le disque. Proposé : oui, et le choix est mémorisé.
3. **Supprimer le JPEG d'une paire en gardant le RAW** (Photo > « Envoyer le JPEG du boîtier dans la corbeille ») : proposé pour plus tard, quand le besoin se présentera ; cette commande passerait par la corbeille comme toute suppression.
4. **Bibliothèques existantes** : relier automatiquement les paires déjà importées par la migration (proposé), ou seulement les nouveaux imports ?
5. **Aperçus** tirés du JPEG du boîtier quand il est plus grand que l'aperçu intégré : proposé. L'inconvénient, que l'on a déjà avec l'aperçu intégré, est que l'aperçu montre le style d'image du boîtier et non le développement du RAW.

## Conséquences

- Une colonne et une migration ; aucun changement de format XMP.
- Les règles absolues restent vraies : le JPEG est copié, vérifié et supprimé comme tout original, toujours par la corbeille.
- Les rafales empilées de la V0.4 réutiliseront l'affichage en pile (pastille, ouverture d'un clic), mais pas ce modèle de données.

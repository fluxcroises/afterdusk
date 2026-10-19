# 0003 — Paires RAW + JPEG en pile

Date : 2026-10-19 · Statut : **adoptée** (réponses de l'utilisateur du 2026-10-19)

## Contexte

Beaucoup de photographes déclenchent en RAW + JPEG (ou RAW + HEIF) : le boîtier écrit `DSC_0001.NEF` et `DSC_0001.JPG` pour le même déclenchement. Jusqu'ici, Afterdusk importait deux photos indépendantes : deux miniatures côte à côte, deux notes à poser, et une suppression qui pouvait séparer les deux fichiers.

Les logiciels concurrents :

- **Lightroom** traite la paire comme une seule photo : le JPEG est un fichier compagnon, invisible dans la grille.
- **Capture One** présente les deux fichiers comme des variantes empilées, que l'on peut déplier.

Le cahier des charges ne mentionne pas les paires. Il prévoit en revanche des **rafales empilées** en V0.4 (§4.3), qui regroupent plusieurs déclenchements, alors qu'une paire n'en est qu'un. Les deux notions restent distinctes, même si l'interface les affiche de la même façon.

## Décision

### Une pile par déclenchement

Les deux fichiers forment une **pile**, comme dans Capture One : l'utilisateur voit d'emblée les deux formats et choisit celui qu'il garde ou met en avant.

- **Repliée** (par défaut), la pile occupe une case de la grille : le fichier du dessus, avec une pastille qui indique le nombre de fichiers. Elle compte pour une photo dans les compteurs et les filtres.
- **Dépliée** (touche S, comme dans Lightroom, ou clic sur la pastille), les deux fichiers s'affichent côte à côte. On peut alors les comparer, en choisir un ou en supprimer un.
- **Dessus de la pile** : le RAW par défaut ; Photo > « Mettre en haut de la pile » donne la place au JPEG.
- **Marques** (note, drapeau, couleur) : elles appartiennent au déclenchement. Posées sur la pile ou sur l'un de ses fichiers, elles valent pour les deux, et sont écrites **dans les deux XMP** (`DSC_0001.xmp` et `DSC_0001.JPG.xmp`, qui coexistent : décision 0002).
- **Corbeille, au choix de l'utilisateur** :
  - Suppr sur une pile repliée envoie les deux fichiers ;
  - sur une pile dépliée, seulement les fichiers sélectionnés ;
  - Photo > « Envoyer le JPEG dans la corbeille » ou « Envoyer le RAW dans la corbeille » garde l'autre fichier.

  Si le fichier du dessus part seul, l'autre prend sa place. Un fichier restauré retrouve sa pile.

**Modèle de données.** Une colonne `companion_of INTEGER REFERENCES media(id) ON DELETE SET NULL` sur `media` : le fichier du dessous pointe vers celui du dessus. Changer le dessus de la pile revient à inverser le lien. Chaque fichier garde sa ligne et tout son traitement actuel (copie, empreinte, aperçus, XMP, corbeille).

Option écartée : une seule ligne `media` avec une table `media_files`. Ce modèle est plus juste, mais il oblige à reprendre tout ce qui manipule un chemin.

### Reconnaissance des paires

Deux fichiers forment une paire s'ils remplissent les quatre conditions :

1. ils sont dans le **même dossier** ;
2. ils ont le **même nom sans extension**, sans tenir compte de la casse (`DSC_0001.NEF` et `DSC_0001.JPG`, le cas habituel) ;
3. l'un est un RAW, l'autre un JPEG ou un HEIF (`jpg`, `jpeg`, `heic`, `heif`, `hif`) ;
4. leurs **heures de prise de vue** sont égales à 2 secondes près, ce qui évite une fausse paire quand le compteur du boîtier repasse par le même numéro.

Sinon, les fichiers restent deux photos indépendantes.

### Import

- La source affiche une case par paire, avec sa pastille ; la case à cocher prend la paire entière. Le compte indique « 120 photos (240 fichiers) ».
- **Option « JPEG des paires : importer / ne pas importer »**, importer par défaut, choix mémorisé. Elle sert à ceux qui ne déclenchent en RAW + JPEG que pour la carte de secours.
- **Renommage** : un seul numéro `{seq}` par paire, pour que les deux fichiers gardent le même nom.
- **Doublons** : inchangé, chaque fichier a son empreinte.

### Aperçus

Quand le JPEG du boîtier est plus grand que l'aperçu intégré au RAW (Sony : 1616 px, Panasonic : 1920 px), les aperçus et le zoom 100 % du RAW sont tirés du JPEG. L'aperçu montre alors le style d'image du boîtier, comme l'aperçu intégré, et non le développement du RAW.

### Bibliothèques existantes

La migration `0003` ajoute la colonne et relie les paires déjà importées selon les mêmes critères. Aucun fichier n'est déplacé ni modifié : seule la base change.

## Conséquences

- Une colonne et une migration ; aucun changement de format XMP.
- Les règles absolues restent vraies : chaque fichier de la pile est copié, vérifié et supprimé comme tout original, toujours par la corbeille.
- Les rafales empilées de la V0.4 réutiliseront l'affichage en pile (pastille, S pour déplier), avec leur propre modèle.
- Retouche (V0.2) et livraison (V0.3) partiront du fichier du dessus.

# 0015. Pinceau et gomme

Date : 2026-10-29 · Statut : **adoptée**, au choix de l'utilisateur

## Contexte

La décision 0014 a livré les dégradés linéaire et radial en V0.6 et reporté le pinceau, avec sa gomme, à la version suivante. Elle fixe aussi que les masques sont recalculés depuis leurs formes à la définition de chaque rendu, jamais depuis une image réduite.

## Décision

1. **Tracés, pas d'image** : un coup de pinceau s'enregistre comme une suite de points (fractions de la photo source, avant géométrie), avec sa taille (fraction du côté long), son adoucissement et sa densité. Il est redessiné à chaque rendu, à sa définition, à travers la géométrie, comme les dégradés.
2. **Pinceau, nouvelle forme** : « Pinceau 1 », un masque fait uniquement de tracés en ajout.
3. **Gomme sur tous les masques** : un tracé en retrait retire l'effet de n'importe quel masque, dégradés compris (exemple : un arbre qui dépasse dans un ciel assombri). Alt maintenu pendant le tracé, comme dans Lightroom.
4. **Densité** : chaque tracé ajoute (ou retire) l'effet à hauteur de sa densité ; repasser au même endroit monte l'effet peu à peu, sans dépasser le plein effet.
5. **Format des réglages en version 3** : la version 0.6 ignorerait des tracés qu'elle ne connaît pas et les effacerait en enregistrant ; un document avec tracés est donc écrit en version 3, qu'elle refuse. Un document sans tracé reste écrit dans la plus ancienne version qui suffit (1 sans masque, 2 avec masques sans tracé).

## Conséquences

- Migration v2 → v3 dans `develop/settings.rs`, testée sur un JSON v2 ; bornes des tracés (nombre de points, taille, densité).
- La couverture des tracés est calculée par rendu dans un tampon à la taille de la sortie, sur le processeur comme sur la carte graphique, avec un test d'égalité.
- Hors de cette étape : masque automatique qui suit les bords, masques du sujet ou du ciel par détection.

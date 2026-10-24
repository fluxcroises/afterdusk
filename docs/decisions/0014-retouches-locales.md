# 0014. Retouches locales

Date : 2026-10-24 · Statut : **adoptée**, au choix de l'utilisateur

## Contexte

Première piste de la V1 : des réglages qui ne s'appliquent qu'à une partie de la photo (un ciel, un visage, le sol), comme les masques de Lightroom. Le moteur applique aujourd'hui les mêmes réglages à tous les pixels, sauf deux exceptions qui servent de modèle : les corrections de défauts, rangées en fractions de la photo source et suivies par le recadrage, et le vignetage, calculé selon la position du pixel.

## Décision

1. **Format des réglages en version 2** : une liste `masks`, chaque masque portant sa forme (dégradé linéaire, dégradé radial, puis pinceau) et ses réglages. Positions en fractions de la photo source, avant recadrage et redressement, comme les corrections de défauts. Un document en version 1 se lit comme un document sans masque ; une version d'Afterdusk qui ne connaît que la version 1 refuse un document en version 2 (« format plus récent ») au lieu d'effacer les masques en l'enregistrant.
2. **Sept réglages par masque** : exposition, contraste, hautes lumières, ombres, température, teinte, saturation. Tous se calculent pixel par pixel. Clarté, netteté et bruit locaux, qui regardent les pixels voisins, viendront plus tard.
3. **Livraison en deux temps** : V0.6 avec les dégradés linéaire et radial, puis le pinceau (avec gomme) dans la version suivante.
4. **Masques recalculés depuis leurs formes** à la définition de chaque rendu (aperçu, zoom, export), jamais depuis une image réduite.

## Conséquences

- Migration v1 → v2 dans `develop/settings.rs`, testée sur un JSON v1.
- Les masques sont dessinés à la taille de l'image de travail, puis transformés avec elle par la géométrie ; la carte graphique les reçoit en textures.
- Hors de cette étape : masques automatiques du sujet ou du ciel (détection, sans rien inventer), réglages locaux de détail.

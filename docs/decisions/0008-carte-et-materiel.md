# 0008 — V0.3 : fond de carte, géocodage, illustration du matériel

Date : 2026-10-21 · Statut : **adoptée** (choix de l'utilisateur le 2026-10-21)

## Contexte

La V0.3 (cahier des charges, 4.7 et 4.8) apporte la carte du monde et la gestion du matériel. La carte est le seul appel réseau permis avec la vérification des mises à jour ; le fournisseur de tuiles était un point ouvert (section 9). Le géocodage inverse doit marcher hors ligne. L'illustration du matériel était à trancher au début de la V0.3.

## Décisions

- **Tuiles en ligne : OpenFreeMap** (instance publique). Gratuit, sans clé ni inscription, sans cookies, usage commercial permis ; attribution OpenStreetMap et OpenMapTiles exigée, affichée par MapLibre GL JS (BSD 3). Aucune autre requête réseau ; la carte n'en fait que lorsqu'elle est affichée.
- **Fond hors ligne : Natural Earth** (domaine public), frontières des pays simplifiées, embarquées dans l'application.
- **Géocodage inverse hors ligne : GeoNames, lieux de plus de 1000 habitants** (environ 150 000 lieux, CC BY 4.0, crédité dans « À propos »), avec les noms des régions ; données compactées et embarquées.
- **Illustration du matériel** : la photo de l'utilisateur (choisie dans la bibliothèque ou déposée sur la fiche), sinon une silhouette dessinée pour Afterdusk selon le type (hybride, reflex, compact, objectif fixe ou zoom), et la marque en toutes lettres. Ni photo ni logo de fabricant : les photos sont protégées par le droit d'auteur, les logos sont des marques, et les télécharger serait un appel réseau interdit. Le nom de la marque en texte reste un usage descriptif permis.
- **Ordre** : lieux et carte d'abord (sources de localisation et fuseaux, géocodage, carte, filtre par zone), puis le matériel, puis les albums intelligents.

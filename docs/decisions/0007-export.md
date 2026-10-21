# 0007 — Export : formats, encodeurs, originaux, filigrane

Date : 2026-10-20 · Statut : **adoptée** (trois choix de l'utilisateur le 2026-10-20)

## Contexte

La V0.2 se termine par l'export (cahier des charges, 4.6) : cinq presets, formats JPEG, HEIF, AVIF, TIFF 8 ou 16 bits, PNG et DNG pour les originaux, espace colorimétrique, filigrane, nettoyage des métadonnées. Plusieurs formats demandent des dépendances difficiles à défaire.

## Décisions

- **Encodeurs en Rust**, déjà dans l'arbre des dépendances ou sans code C : `jpeg-encoder` (MIT/Apache, IJG), `png`, `tiff`, `ravif` pour l'AVIF (BSD, sans assembleur pour ne demander aucun outil de compilation ; plus lent, sans importance pour l'export), `moxcms` pour les profils ICC et les conversions (sRGB, Display P3, Adobe RGB). Profil ICC incorporé à chaque fichier ; l'AVIF reste en sRGB.
- **HEIF par l'encodeur du système** (choix de l'utilisateur) : natif sous macOS ; sous Windows par les extensions HEIF/HEVC de Microsoft quand elles sont installées, sinon le format est grisé avec une explication. Aucune bibliothèque C (libheif, x265) et pas d'encodeur HEVC distribué par Afterdusk.
- **Originaux** (choix de l'utilisateur) : le preset « Originaux » copie le fichier d'origine tel quel, vérifié par somme de contrôle comme à l'import, avec son XMP. La conversion en DNG est reportée après la V0.2.
- **Filigrane texte** (choix de l'utilisateur) : une police libre (licence OFL) livrée avec Afterdusk, pour un rendu identique sous Windows et macOS, créditée dans « À propos ».
- **Rendu d'export** : processeur, en virgule flottante (16 bits sans perte), demi-définition quand la taille demandée le permet ; jamais d'agrandissement.

## Complément du 2026-10-21 : métadonnées du HEIF

L'encodeur HEIF de Windows (WIC) accepte l'EXIF et le XMP sans erreur mais ne les écrit pas, même une seule propriété. Afterdusk ajoute donc lui-même deux éléments au fichier produit par le système, « Exif » et « mime » (XMP), décrivant l'image principale (ISO/IEC 23008-12), et recale les décalages de la table iloc. Le même code sert sous macOS, où ImageIO n'encode que l'image et son profil. Les tests vérifient que les octets de l'image restent identiques et que le décodeur de Windows relit l'image et le XMP.

## Limite connue

Le rendu du processus 1 passe par l'espace sRGB avant les couleurs par teinte et les courbes : un export Display P3 ou Adobe RGB décrit exactement les couleurs dans ces espaces, sans couleurs hors du gamut sRGB. Un espace de travail plus large demandera un nouveau processus.

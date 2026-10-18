Ceci est une proposition de charte graphique pour Afterdusk

---

## 1. Direction artistique

Adopter un design professionnel, minimaliste et fonctionnel, reposant sur les principes suivants :

- Interface sombre par défaut.
- Palette principalement composée de gris neutres légèrement froids.
- Contrastes maîtrisés.
- Typographie lisible et discrète.
- Composants compacts mais confortables à utiliser.
- Icônes monochromes.
- Bordures fines et angles légèrement arrondis.
- Absence d'effets décoratifs inutiles.
- Priorité absolue à l'affichage des photographies.

L'interface doit rester visuellement discrète afin de ne pas détourner l'attention de l'image en cours de retouche.

Éviter les dégradés colorés, les ombres importantes, les effets *glassmorphism*, les bordures lumineuses, les éléments surdimensionnés et les couleurs trop saturées.

## 2. Palette de couleurs officielle

Utiliser les couleurs suivantes pour les différents éléments de l'interface.

| Élément | Couleur |
|---|---|
| Fond principal | `#17191C` |
| Espace de travail photographique | `#111214` |
| Panneaux latéraux | `#22252A` |
| Surfaces secondaires | `#2C3036` |
| Survol des éléments | `#383D44` |
| Bordures et séparateurs | `#383D44` |
| Texte principal | `#F0F2F5` |
| Texte secondaire | `#AAB0B9` |
| Texte désactivé | `#737983` |
| Accent principal | `#7598E8` |
| Accent au survol | `#92ACF0` |
| Succès | `#6EBC94` |
| Avertissement | `#DBAD69` |
| Erreur | `#D97878` |

### Règles d'utilisation

Le bleu d'accent doit rester discret. Il sert uniquement à identifier les interactions et les éléments actifs :

- Boutons principaux.
- Outils sélectionnés.
- Curseurs de réglage.
- Cases cochées.
- Sélections.
- Champs actifs.
- Indicateurs de progression lorsque nécessaire.

Ne pas utiliser le bleu comme couleur décorative dans les panneaux ou les arrière-plans principaux.

L'espace d'affichage des photographies doit rester particulièrement neutre, afin de limiter les interférences visuelles pendant la correction colorimétrique.

Prévoir une gestion cohérente des contrastes pour tous les textes, icônes et contrôles.

---

## 3. Typographie

Police à définir : Inter, Manrope, Sora, IBM Plex Sans ou Geist

Définir la hiérarchie suivante :

| Élément | Taille | Graisse |
|---|---|---|
| Titres des panneaux | 14 px | 600 |
| Sections d'outils | 12 px | 600 |
| Libellés des contrôles | 12–13 px | 400 |
| Valeurs numériques | 12 px | 500 |
| Métadonnées | 11 px | 400 |

Principes :

- Éviter les caractères trop gras.
- Éviter les majuscules systématiques.
- Utiliser des chiffres tabulaires pour les valeurs numériques.
- Maintenir une bonne lisibilité même dans les panneaux denses.
- Conserver une cohérence typographique sur l'ensemble du logiciel.

---

## 4. Structure générale de l'interface

Organiser visuellement l'application selon les besoins du flux de travail photographique.

### Barre supérieure

Hauteur indicative : 48 px.

Elle peut contenir, selon les fonctionnalités existantes :

- Logo Afterdusk.
- Navigation entre les espaces de travail.
- Commandes globales.
- Annulation et rétablissement.
- Paramètres.
- Exportation.

Limiter les éléments décoratifs et privilégier un alignement régulier.

### Panneau latéral gauche

Largeur indicative : 260 à 300 px.

Destiné aux éléments de navigation et d'organisation, selon les fonctionnalités disponibles :

- Bibliothèque.
- Dossiers.
- Albums.
- Historique des modifications.
- Informations sur les fichiers.

### Espace de travail central

Il doit occuper la majorité de l'écran.

Utiliser le fond `#111214`.

Les photographies doivent être affichées sans bordure décorative ni ombre excessive.

Préserver les fonctionnalités de zoom, déplacement, comparaison et affichage plein écran lorsqu'elles sont disponibles.

### Panneau latéral droit

Largeur indicative : 260 à 300 px.

Il regroupe les outils de développement photographique :

- Lumière.
- Couleur.
- Balance des blancs.
- Courbes.
- Netteté.
- Corrections optiques.
- Recadrage.
- Autres outils existants.

Les groupes d'outils doivent être clairement identifiés et pouvoir être repliés lorsque cela est pertinent.

### Bande inférieure

Si l'application dispose d'un film fixe, harmoniser son apparence avec les panneaux latéraux.

Les miniatures doivent être lisibles, avec une sélection clairement identifiable et des bordures discrètes.

Permettre de masquer les panneaux lorsque cette fonctionnalité est disponible ou facile à intégrer sans modifier l'architecture.

---

## 5. Composants graphiques

Uniformiser les composants existants.

### Boutons

- Hauteur standard : 32 à 36 px.
- Rayon des angles : 6 px.
- Espacement horizontal : 12 px.
- Fond neutre pour les actions secondaires.
- Fond bleu `#7598E8` pour les actions principales.
- Texte sombre sur les boutons bleus.
- États hover, active, focus et disabled clairement définis.

Éviter d'avoir plusieurs boutons d'action principale concurrents dans un même panneau.

### Curseurs de réglage

Les sliders constituent un élément central d'Afterdusk et doivent recevoir une attention particulière.

- Barre fine et discrète.
- Portion active bleue.
- Curseur facilement manipulable.
- Valeur numérique visible et alignée.
- Libellé clairement associé au réglage.
- Affichage des valeurs négatives et positives.
- Possibilité de réinitialiser le réglage si la fonctionnalité existe.
- Réactivité immédiate lors des ajustements.

Préserver intégralement le comportement des sliders existants.

### Champs numériques

Utiliser :

- Fond `#2C3036`.
- Texte `#F0F2F5`.
- Bordure discrète.
- Focus bleu.
- Alignement des valeurs à droite lorsque pertinent.

### Menus et listes déroulantes

- Fond `#2C3036`.
- Survol `#383D44`.
- Sélection visible.
- Espacement régulier.
- Séparateurs fins.

### Panneaux accordéon

Les sections de réglages doivent utiliser :

- Un titre discret mais lisible.
- Un chevron d'ouverture ou de fermeture.
- Une séparation fine.
- Un espacement vertical régulier.
- Aucun effet d'ombre prononcé.

---

## 6. Iconographie

Adopter une iconographie linéaire, homogène et minimaliste.

Privilégier **Lucide Icons** si aucune bibliothèque cohérente n'est déjà intégrée au logiciel.

Caractéristiques :

- Taille habituelle : 18 à 20 px.
- Épaisseur régulière.
- Couleur blanche ou gris clair.
- Bleu réservé à l'état actif.
- Aucun effet 3D.
- Pas d'icônes multicolores.

Harmoniser les icônes des commandes photographiques, de navigation et des paramètres.

### Identité d'Afterdusk

Utiliser le logo SVG pour représenter l'application.

- Conserver la forme originale du symbole.
- Utiliser une version monochrome blanche.
- Préserver la transparence du fond.
- Ne pas déformer le SVG.
- Ne pas ajouter d'effets de lumière, d'ombre ou de dégradé.
- Prévoir une version adaptée aux petites tailles.

Pour l'icône de l'application, utiliser le symbole blanc sur un carré anthracite `#17191C`, aux coins légèrement arrondis.

Ne pas utiliser un autre symbole générique de photographie en remplacement du logo officiel.

---

## 7. Espacements et géométrie

Mettre en place une échelle d'espacement cohérente :

- 4 px : espacement très réduit.
- 8 px : espacement entre éléments liés.
- 12 px : espacement standard.
- 16 px : espacement entre groupes.
- 24 px : séparation entre sections importantes.

Rayons recommandés :

- Petits éléments : 4 px.
- Boutons, champs et menus : 6 px.
- Fenêtres et grandes surfaces : 10 px.

Éviter les arrondis excessifs donnant une apparence d'application mobile.

L'interface doit conserver une densité adaptée à un logiciel de bureau professionnel.

---

## 8. Animations et interactions

Utiliser des transitions discrètes de 120 à 180 ms pour :

- Les survols.
- Les ouvertures de panneaux.
- Les changements d'onglets.
- Les menus déroulants.

Ne pas animer artificiellement les curseurs photographiques : les réglages doivent rester précis et immédiatement réactifs.

Limiter les animations susceptibles de ralentir l'affichage ou le traitement des photographies.

Respecter les préférences système concernant la réduction des animations.

---

## 9. Responsive et adaptabilité

Afterdusk étant destiné à un usage photographique, l'interface doit bien fonctionner sur différents écrans d'ordinateur.

Prévoir notamment :

- Une résolution Full HD.
- Des écrans QHD et 4K.
- La mise à l'échelle Windows/Mac OS.
- Le redimensionnement de la fenêtre.
- Le comportement des panneaux lorsque l'espace horizontal diminue.
- Une taille minimale exploitable pour les contrôles.

Ne pas réduire excessivement les textes et les boutons afin de faire tenir tous les outils à l'écran.

---

## 10. Accessibilité

Appliquer les bonnes pratiques suivantes :

- Contrastes suffisants selon WCAG AA.
- Navigation clavier lorsque pertinente.
- Indicateur de focus visible.
- Libellés explicites.
- États actifs distinguables autrement que par la couleur.
- Zones cliquables suffisamment grandes.
- Infobulles pour les commandes représentées uniquement par une icône.

La densité de l'interface ne doit pas compromettre son accessibilité.

---

## 11. Architecture CSS et composants

Créer une structure centralisée pour les variables de design.

Exemple de variables à intégrer :

```css
:root {
  --ad-bg: #17191C;
  --ad-workspace: #111214;
  --ad-panel: #22252A;
  --ad-surface: #2C3036;
  --ad-hover: #383D44;

  --ad-border: #383D44;

  --ad-text: #F0F2F5;
  --ad-text-secondary: #AAB0B9;
  --ad-text-muted: #737983;

  --ad-accent: #7598E8;
  --ad-accent-hover: #92ACF0;

  --ad-success: #6EBC94;
  --ad-warning: #DBAD69;
  --ad-danger: #D97878;

  --ad-radius-sm: 4px;
  --ad-radius-md: 6px;
  --ad-radius-lg: 10px;

  --ad-topbar-height: 48px;
  --ad-sidebar-width: 280px;
  --ad-control-height: 34px;

  --ad-transition: 150ms ease;
}
```

Ces variables constituent une référence. Les adapter à la technologie et à l'organisation du projet.

Éviter la multiplication de couleurs et dimensions codées en dur dans les composants.

Si le logiciel utilise React, centraliser autant que possible les styles au sein de composants UI réutilisables.

---

## 12. Final

Contrôler :

- La cohérence visuelle.
- La lisibilité.
- Le fonctionnement des outils.
- La fluidité de l'interface.
- L'absence de régressions.
- Le comportement à différentes résolutions.

## Résultat attendu

Une interface sombre, moderne et professionnelle, qui rappelle la qualité ergonomique des logiciels photographiques de référence tout en donnant à Afterdusk sa propre identité.

Le résultat doit être fonctionnel, cohérent, sobre, agréable à utiliser pendant de longues sessions de retouche et suffisamment discret pour laisser toute la place aux photographies.

**Ne pas se limiter à une maquette : appliquer réellement la charte au projet existant, dans le respect de son architecture et de ses fonctionnalités.**
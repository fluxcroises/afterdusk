# Afterdusk

Logiciel de bureau libre et gratuit pour les photographes semi-pro : importer, trier, retoucher, livrer. Windows et macOS, licence GPL v3. Publié par [Flux Croisés](https://flux-croises.fr), association à but non lucratif qui veut rendre la photographie, la vidéo et les arts visuels accessibles à tous, gratuitement.

## Ce que fait Afterdusk

- **Importer** depuis une carte, un dossier, un dossier surveillé ou un boîtier en FTP sur le réseau local. Chaque copie est vérifiée, et peut être doublée sur un disque de sauvegarde. Les originaux et la carte ne sont jamais modifiés.
- **Trier** à la grille et à la visionneuse : notes, drapeaux, couleurs, rafales regroupées, photos floues ou yeux fermés repérés.
- **Retoucher** les RAW de la plupart des boîtiers (LibRaw), les JPEG, TIFF, PNG et HEIF, sans toucher au fichier : lumière, couleur, courbes, recadrage, corrections d'objectif, petites imperfections.
- **Livrer** en JPEG, PNG, TIFF, AVIF ou HEIF, avec des modèles prêts à l'emploi (réseaux sociaux, tirages, web), en galerie web, en planche contact PDF ou en composition (texte, logo, collage).
- Et aussi : carte des prises de vue, suivi du matériel, fiches clients, vidéos avec dérushage sans réencodage.

## Télécharger

La dernière version est sur la page [Releases](https://github.com/fluxcroises/afterdusk/releases/latest) :

- **Windows 10 et 11 (64 bits)** : `Afterdusk_<version>_x64-setup.exe`
- **macOS (Apple Silicon et Intel)** : `Afterdusk_<version>_universal.dmg`

### Installer sous Windows

Lancez l'installateur. Windows demande les droits d'administrateur : Afterdusk s'installe dans `Program Files\Flux Croisés\Afterdusk`, pour tous les utilisateurs de l'ordinateur.

Tant que les installateurs ne sont pas signés (voir plus bas), Windows affiche « Windows a protégé votre ordinateur ». Cliquez sur « Informations complémentaires », puis sur « Exécuter quand même ».

### Installer sur Mac

Ouvrez le `.dmg` et glissez Afterdusk dans le dossier Applications.

L'application n'est pas encore signée par Apple. À la première ouverture, macOS la bloque :

1. Ouvrez Afterdusk une première fois, puis fermez le message.
2. Allez dans Réglages Système > Confidentialité et sécurité.
3. En bas, à côté du message sur Afterdusk, cliquez sur « Ouvrir quand même », puis confirmez.

Les ouvertures suivantes se font normalement.

### Mises à jour

Au démarrage, au plus une fois par jour, Afterdusk regarde si une nouvelle version est publiée ici. Si c'est le cas, un bandeau la propose ; rien ne s'installe sans votre clic. La vérification se désactive dans Réglages > Général.

## Confidentialité

Afterdusk ne collecte aucune donnée et n'envoie aucune photo. Il fait trois sortes de requêtes, chacune désactivable dans les Réglages :

- les tuiles du fond de carte (OpenFreeMap), quand la carte est affichée ;
- la vérification des mises à jour (GitHub), au plus une fois par jour ;
- la réception FTP, sur le réseau local seulement, désactivée par défaut.

Vos photos, vos réglages et votre bibliothèque restent sur votre ordinateur et vos disques.

## Politique de signature du code

Les installateurs sont construits par GitHub Actions, depuis ce dépôt public, par l'action [Publication](.github/workflows/release.yml) ; aucun fichier construit ailleurs n'est publié. Les archives de mise à jour sont signées avec la clé du projet, et Afterdusk refuse toute mise à jour qui n'en porte pas la signature.

Pour l'instant, les installateurs Windows et macOS ne sont pas signés. Une demande de signature gratuite est prévue auprès de [SignPath Foundation](https://signpath.org) pour Windows ; cette section donnera alors la mention et les règles exigées.

Rôles :

- auteurs (code publié dans ce dépôt) : [Lewis Bedar](https://github.com/lewisbedar) ;
- relecteurs (contributions extérieures) : [Lewis Bedar](https://github.com/lewisbedar) ;
- approbateurs (chaque publication) : [Lewis Bedar](https://github.com/lewisbedar).

## Développer

Prérequis : Node.js 24, Rust stable, et sous Windows Visual Studio Build Tools (charge « Développement Desktop en C++ »).

```bash
git clone --recurse-submodules https://github.com/fluxcroises/afterdusk.git
npm ci
node scripts/fetch-test-assets.mjs   # échantillons RAW de test (≈ 320 Mo, raw.pixls.us)
node scripts/fetch-ffmpeg.mjs        # FFmpeg, vérifié par son empreinte
node scripts/fetch-onnxruntime.mjs   # ONNX Runtime, vérifié par son empreinte
npm run tauri dev
```

Tests : `npm test` pour l'interface, `cargo test --workspace` dans `src-tauri/` pour le cœur. Pour juger des temps de rendu, lancez plutôt `npm run tauri dev -- --release` : en développement, le cœur n'est pas optimisé.

Documents : [cahier des charges](docs/cahier-des-charges.md), [avancement](docs/avancement.md), [charte graphique](docs/charte-graphique.md), [décisions](docs/decisions/), [guide d'utilisation](docs/guide-utilisateur.md), [distribution](docs/distribution.md).

## Licence

© 2026 Lewis Bedar. GPL v3 ou ultérieure ([LICENSE](LICENSE)). Les composants livrés avec Afterdusk et leur licence sont listés dans Aide > À propos (LibRaw sous LGPL 2.1, XMP Toolkit d'Adobe sous BSD 3, FFmpeg sous GPL 2.0 ou ultérieure, ONNX Runtime sous MIT, entre autres).

La photo de l'écran d'accueil (`src/assets/accueil-cabourg.jpg`, Cabourg) est © Lewis Bedar, tous droits réservés : elle n'est pas couverte par la licence GPL du code. Le logo de Flux Croisés (`src/assets/logo_horizontal_blanc.png`) appartient à l'association et n'est pas non plus couvert par la GPL.

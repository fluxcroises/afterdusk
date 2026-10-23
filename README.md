# Afterdusk

Logiciel de bureau libre et gratuit pour les photographes semi-pro : importer, trier, retoucher, livrer. Windows et macOS, licence GPL v3. Publié par [Flux Croisés](https://flux-croises.fr), association à but non lucratif qui veut rendre la photographie, la vidéo et les arts visuels accessibles à tous, gratuitement.

> En développement : version 0.1 (Importer et trier). Voir [docs/avancement.md](docs/avancement.md).

## Développer

Prérequis : Node.js 24, Rust stable, et sous Windows Visual Studio Build Tools (charge « Développement Desktop en C++ »).

```bash
git clone --recurse-submodules https://github.com/fluxcroises/afterdusk.git
npm ci
node scripts/fetch-test-assets.mjs   # échantillons RAW de test (≈ 320 Mo, raw.pixls.us)
npm run tauri dev
```

Tests : `npm test` pour l'interface, `cargo test --workspace` dans `src-tauri/` pour le cœur.

Documents : [cahier des charges](docs/cahier-des-charges.md), [charte graphique](docs/charte-graphique.md), [décisions](docs/decisions/).

## Licence

© 2026 Lewis Bedar. GPL v3 ou ultérieure ([LICENSE](LICENSE)). LibRaw est utilisé sous LGPL 2.1, le XMP Toolkit d'Adobe sous BSD 3.

La photo de l'écran d'accueil (`src/assets/accueil-cabourg.jpg`, Cabourg) est © Lewis Bedar, tous droits réservés : elle n'est pas couverte par la licence GPL du code. Le logo de Flux Croisés (`src/assets/logo_horizontal_blanc.png`) appartient à l'association et n'est pas non plus couvert par la GPL.

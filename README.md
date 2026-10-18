# Afterdusk

Logiciel de bureau libre et gratuit pour les photographes semi-pro : importer, trier, retoucher, livrer. Windows et macOS, licence GPL v3.

> En développement : version 0.1 (Importer et trier). Voir [docs/avancement.md](docs/avancement.md).

## Développer

Prérequis : Node.js 24, Rust stable, et sous Windows Visual Studio Build Tools (charge « Développement Desktop en C++ »).

```bash
git clone --recurse-submodules https://github.com/lewisbedar/afterdusk-app.git
npm ci
node scripts/fetch-test-assets.mjs   # échantillons RAW de test (≈ 320 Mo, raw.pixls.us)
npm run tauri dev
```

Tests : `npm test` pour l'interface, `cargo test --workspace` dans `src-tauri/` pour le cœur.

Documents : [cahier des charges](docs/cahier-des-charges.md), [charte graphique](docs/charte-graphique.md), [décisions](docs/decisions/).

## Licence

GPL v3 ou ultérieure ([LICENSE](LICENSE)). LibRaw est utilisé sous LGPL 2.1, le XMP Toolkit d'Adobe sous BSD 3.

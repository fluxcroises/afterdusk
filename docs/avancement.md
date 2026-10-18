# Avancement

Un critère n'est coché qu'une fois testé sur Windows et macOS par GitHub Actions (CLAUDE.md).

## V0.1 : Importer et trier

- [ ] Projet Tauri 2 qui se compile et se lance sur Windows et macOS, avec intégration continue GitHub Actions sur les deux systèmes
- [ ] Première ouverture guidée (bibliothèque, sauvegarde, rangement, identité)
- [ ] Une carte SD insérée est détectée en moins de 5 secondes et l'import proposé
- [ ] Import de 500 RAW avec rangement, renommage, sauvegarde et vérification par somme de contrôle, sans aucune perte, en cas de coupure comprise (reprise propre au redémarrage)
- [ ] Les photos déjà importées sont reconnues et grisées lors d'un second import
- [ ] Aperçus affichés dès la fin de la copie de chaque fichier, sans attendre la fin de l'import
- [ ] Passage d'une photo à la suivante en moins de 100 ms dans la visionneuse
- [ ] Notes, drapeaux, couleurs au clavier, écrits en base et dans le XMP
- [ ] Comparaison côte à côte de 2 à 4 photos avec zoom synchronisé
- [ ] Corbeille interne avec restauration et vidage après 30 jours
- [ ] Vue Bibliothèque avec albums, filtres de base (date, note, drapeau, boîtier, objectif) et recherche
- [ ] Bibliothèque sur disque externe débranché consultable via le cache
- [ ] RAW testés au minimum : Fujifilm (RAF X-Trans), Canon (CR3), Nikon (NEF), Sony (ARW), Panasonic (RW2), DNG
- [ ] Interface en français et en anglais

## Journal

### 2026-10-18

- Projet initialisé : Tauri 2.12, React 19, TypeScript strict, Vite, ESLint, Prettier, Vitest, i18next, Zustand. Barre d'étapes et barre latérale aux couleurs de la charte graphique (`src/theme.css`), icônes Lucide, logo et icône d'application.
- CI GitHub Actions (`.github/workflows/ci.yml`) sur `windows-latest` et `macos-latest` : format, lint, types, Vitest, cargo fmt, clippy, cargo test, compilation Tauri. Le premier critère sera coché quand la CI passera sur les deux systèmes. Mac Intel non couvert par la CI pour l'instant.
- Prototype LibRaw validé localement (Windows) sur les six formats : voir [décision 0001](decisions/0001-liaison-libraw.md). Le critère « RAW testés » reste ouvert tant que le décodage complet (pas seulement l'aperçu) et macOS ne sont pas vérifiés.
- Prototype XMP avec xmp-toolkit-rs : voir [décision 0002](decisions/0002-metadonnees-xmp.md). Nommage des annexes et espace de noms à valider.
- Schéma SQLite V0.1 proposé : [schema-v0.1.md](schema-v0.1.md), en attente de validation avant de coder l'import.

Tests en place : 14 tests Rust (dont deux de règle absolue : lecture et écriture XMP sans modifier l'original) et 4 tests Vitest.

# Afterdusk

Logiciel open source (GPL v3) de gestion photo pour photographes semi-pro.
Tauri 2 + Rust (src-tauri/) et React + TypeScript (src/).
Cahier des charges : docs/cahier-des-charges.md. Avancement : docs/avancement.md.

## Règles absolues
- Ne jamais modifier, déplacer ou supprimer un original sans action explicite de l'utilisateur.
- Ne jamais écrire sur la carte SD ou le boîtier source.
- Toute copie est vérifiée par somme de contrôle avant d'être considérée comme faite.
- Toute suppression passe par la corbeille interne.
- Aucun appel réseau hors tuiles de carte et vérification des mises à jour.

## Conventions
- Rust : cargo fmt et cargo clippy sans avertissement ; erreurs avec thiserror, jamais de unwrap hors tests.
- TypeScript strict, ESLint et Prettier ; composants fonctionnels React.
- Aucun texte d'interface en dur : tout passe par i18next (fr.json et en.json à jour ensemble).
- Traitements longs dans des tâches Rust en arrière-plan, progression envoyée par événements Tauri.
- Chemins de médias stockés relativement à la racine de la bibliothèque.
- Réglages de retouche en JSON versionné ; toute évolution du format prévoit une migration.
- Migrations SQLite numérotées dans src-tauri/src/db/migrations/.
- Dépendances compatibles GPL v3 uniquement.

## Tests
- cargo test pour le cœur, Vitest pour l'interface.
- Tests d'import et de décodage sur les échantillons de test-assets/ (RAW libres de droits de raw.pixls.us).
- Un test par règle absolue (aucun original modifié, aucune écriture sur la source).
- Tester sur Windows et macOS via GitHub Actions avant de cocher un critère.

## Façon de travailler
- Une version à la fois, dans l'ordre de docs/cahier-des-charges.md section 7.
- Plan court avant chaque fonctionnalité, commit par fonctionnalité, messages en français.
- Demander avant une décision difficile à défaire.

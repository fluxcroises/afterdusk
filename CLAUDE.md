# Afterdusk

Logiciel open source (GPL v3) de gestion photo pour photographes semi-pro.
Tauri 2 + Rust (src-tauri/) et React + TypeScript (src/).
Cahier des charges : docs/cahier-des-charges.md. Avancement : docs/avancement.md.

## Règles absolues
- Ne jamais modifier, déplacer ou supprimer un original sans action explicite de l'utilisateur.
- Ne jamais écrire sur la carte SD ou le boîtier source.
- Toute copie est vérifiée par somme de contrôle avant d'être considérée comme faite.
- Toute suppression passe par la corbeille interne.
- Aucun appel réseau hors des exceptions listées ici. Chaque exception se désactive dans les Réglages et n'envoie ni photo ni donnée personnelle à un tiers sans action de l'utilisateur. Exceptions : tuiles de carte, vérification des mises à jour, réception FTP sur le réseau local (désactivée par défaut, décision 0009). Toute nouvelle exception est ajoutée ici après accord de l'utilisateur.

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

<!-- antislop:start -->
## antislop
For UI, copy, people, mobile layout, or code comments work, read these installed skill files directly (use these paths even if a same-named global skill exists):
- Core filter, always on: `antislop`: `.claude/skills/antislop/SKILL.md`
- UI / visual: `antislop-ui`: `.claude/skills/antislop-ui/SKILL.md`
- Copy & text: `antislop-copywriting`: `.claude/skills/antislop-copywriting/SKILL.md`
- People: `antislop-human`: `.claude/skills/antislop-human/SKILL.md`
- Mobile / responsive: `antislop-layoutmobile`: `.claude/skills/antislop-layoutmobile/SKILL.md`
- Code comments: `antislop-code`: `.claude/skills/antislop-code/SKILL.md`
Before starting, follow the core's "Two Usage Modes" section in strict order: explicit session instruction first, then global preference, then ask. A session instruction always wins. For a resolved mode, say `antislop active: <mode> (session override).` or `antislop active: <mode> (global preference).` once before presenting findings or making edits, using the actual mode and source. Acknowledging the user's request without naming the source does not replace this notice.
Only an explicit choice of antislop during or after selects a session mode. A request to review, audit, or avoid file edits does not select a mode; read the global preference in that case. Another skill's mode does not select antislop's mode.
If the mode is unresolved, ask during/after and end the response; wait for the answer before any UI review, planning, or concept. For read-only tasks, put the active-mode notice only at the start of the final answer, never in progress messages. For editing tasks, announce before the first edit and omit it from the final answer.
To update antislop later: `npx antislop-ai --update`, or run `npx antislop-ai` and pick Overwrite them.
<!-- antislop:end -->

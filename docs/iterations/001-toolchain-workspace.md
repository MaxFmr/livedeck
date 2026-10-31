# 001 — Toolchain et workspace

- Date de début : 2026-10-31
- Phase : 0
- Statut : terminée

## Objectif
Avoir un workspace Rust qui compile, avec les lints et profils du projet, prêt pour le sniffer MIDI.

## Périmètre
- Dedans : rustup, workspace, crates vides mais câblées, `git init`, presets de style dans la doc.
- Dehors : toute dépendance externe (midir arrive en 002), tout code audio.

## Critère de fin (mesurable)
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` verts.

## Résultat
- Fait :
  - rustup, profil minimal + rustfmt + clippy, toolchain épinglée `1.99.1` (`rust-toolchain.toml`) et définie par défaut ; doublon `stable` supprimé.
  - Workspace édition 2024, resolver 3 : `crates/engine` (lints anti-panic : `unwrap_used`, `expect_used`, `panic`, `indexing_slicing`), `crates/io`, `crates/app`, `tools/midi-sniff`. Versions des dépendances déclarées dans `[workspace.dependencies]`, pas encore utilisées.
  - Profils : release `lto=fat`, `codegen-units=1`, `panic=abort`, `debug=line-tables-only` ; dépendances et `engine` en `opt-level=2` en debug.
  - `git init` (rien de commité).
  - Doc : section « Presets de style » dans `docs/music/live-engine.md`, ROADMAP mise à jour (presets, phase 3b samples), plan copié dans `docs/plan-iterations.md`.
- Mesures : toolchain 470 Mo (`~/.rustup`), `target/` 10 Mo, 24 Go libres.
- Écarts par rapport au plan : installation avec `--no-modify-path` → `cargo` n'est pas dans le PATH du terminal de l'utilisateur (voir questions ouvertes).

## Appris / à reporter dans la doc
- Un `rust-toolchain.toml` avec une version exacte réinstalle une toolchain nommée `1.99.1` à côté de `stable` (+470 Mo) : garder une seule toolchain, celle épinglée.
- Un crate nommé `io` ne gêne pas `std::io` (toujours écrit en chemin complet), mais à surveiller.

## Questions ouvertes
- Ajouter `. "$HOME/.cargo/env"` au `~/.zshrc` de l'utilisateur pour qu'il puisse lancer `cargo` lui-même.
- `.claude/settings.json` toujours à activer par l'utilisateur.

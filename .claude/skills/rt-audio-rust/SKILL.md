---
name: rt-audio-rust
description: Expert Rust audio temps réel pour LiveDeck — callback cpal, MIDI midir, files rtrb, triple_buffer, horloge et placement à l'échantillon, DSP f32 sans allocation (oscillateurs PolyBLEP, SVF, enveloppes, batterie, effets), gestion mémoire, profils Cargo, tests hors ligne et benchs. Charger avant d'écrire ou modifier du code dans crates/engine, crates/io, un callback audio/MIDI, ou un DSP ; et pour toute question de performance, latence, xrun, mémoire.
---

# Rust audio temps réel

Lire selon le besoin :
- Règles, architecture des threads, horodatage, mémoire, tests : [docs/rust/realtime-audio.md](../../../docs/rust/realtime-audio.md)
- Crates, versions, pièges d'API : [docs/rust/stack.md](../../../docs/rust/stack.md)
- Recettes DSP : [docs/rust/dsp-cookbook.md](../../../docs/rust/dsp-cookbook.md)

## Avant d'écrire du code

1. **Vérifier l'API** de la crate pour la version du `Cargo.lock` (docs.rs ou
   `research/upstream/*-CHANGELOG.md`). cpal 0.18 / egui 0.37 / midir 0.11 ne correspondent pas à la
   mémoire d'entraînement.
2. Identifier sur quel thread le code s'exécute : **audio**, **contrôle**, **MIDI**, **UI**. Le noter en
   commentaire de module (`//! Runs on: audio thread`).
3. Pour le thread audio : concevoir l'état pré-alloué d'abord (capacités, pools), le traitement ensuite.

## Patrons à utiliser

- `Command` : `enum` `Copy`, ≤ 32 octets (`const _: () = assert!(size_of::<Command>() <= 32);`).
- Ressources lourdes : construites sur le thread de contrôle, envoyées par pointeur, l'ancienne renvoyée
  par la file `Garbage`.
- Paramètres continus : cible atomique ou commande + lissage one-pole côté audio.
- `EngineSnapshot` `Copy` écrit une fois par bloc dans le triple buffer.
- Traitement en sous-blocs découpés aux offsets d'événements.
- Fréquence d'échantillonnage passée partout (`prepare(sr, max_block)`), aucun `48000` en dur.

## Définition de « terminé » pour du code audio

- [ ] `cargo clippy -D warnings` (lints de `engine` actifs) et `cargo test` verts.
- [ ] Test de rendu hors ligne couvrant le nouveau code (pas de NaN, crête bornée, comportement attendu).
- [ ] Le test sous `assert_no_alloc` couvre le nouveau chemin.
- [ ] `/rt-audit` passé sur le diff.
- [ ] Si chemin chaud modifié : bench criterion avant/après noté dans le journal d'itération.

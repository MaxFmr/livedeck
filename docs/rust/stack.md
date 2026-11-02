# Pile technique — crates retenues et pièges d'API

Versions relevées sur crates.io le **2026-10-31** (rafraîchir avec `tools/refresh-knowledge.sh`, résultat dans
`research/crates-versions.txt`). **Règle : ne jamais écrire du code `cpal`/`egui`/`eframe`/`midir` de
mémoire.** Ces API changent à chaque mineure ; vérifier sur docs.rs pour la version épinglée dans
`Cargo.lock` ou dans `research/upstream/*-CHANGELOG.md`.

## Retenues

| Rôle | Crate | Version | Pourquoi |
|---|---|---|---|
| Sortie audio | `cpal` | 0.18.2 | standard de fait, CoreAudio direct |
| MIDI | `midir` | 0.11.1 | CoreMIDI, timestamps µs |
| Messages MIDI typés | `wmidi` | 4.0.x | parsing sans allocation (`MidiMessage::try_from(&[u8])`) |
| File SPSC sans verrou | `rtrb` | 0.4.1 | la plus simple et rapide, `Copy` friendly |
| État audio → UI | `triple_buffer` | 9.0.0 | dernier état sans blocage |
| UI | `eframe` / `egui` | 0.37.0 | mode immédiat, léger, rendu GPU, peinture custom facile |
| Décodage samples | `symphonia` | 0.6.1 | WAV/FLAC/MP3/OGG pur Rust (features minimales : `wav`, `flac`) |
| Rééchantillonnage | `rubato` | 5.0.1 | au chargement uniquement |
| Config / mapping | `serde` + `toml` | 1.0 / 1.1 | mapping et presets lisibles |
| Erreurs | `thiserror` (libs), `anyhow` (app) | 2.0 / 1.0 | |
| Logs (hors audio) | `tracing` + `tracing-subscriber` | 0.1 | |
| Debug no-alloc | `assert_no_alloc` | 1.1.2 | ancienne mais stable, debug seulement |
| Bench | `criterion` | 0.8 | |
| WAV de test | `hound` | 3.5 | |

## Écartées (et pourquoi)

- **`fundsp`** (0.23) : excellent pour prototyper un son, mais graphe générique, allocations à la
  construction, contrôle fin des voix moins direct. On écrit notre DSP (simple, voir `dsp-cookbook.md`) ;
  fundsp reste un banc d'essai sonore hors moteur.
- **`kira`, `firewheel`** : moteurs audio de jeu, pas de séquençage musical à l'échantillon sous notre contrôle.
- **`nih-plug`** : framework de plugins (VST3/CLAP) ; inutile pour une app autonome (on pourrait plus tard
  exporter le moteur en plugin).
- **`iced`, `vizia`** : plus de structure, mais plus lourds/moins mûrs pour des widgets peints à la main à 60 fps.
- **`basedrop`** : pratique mais peu maintenu → file poubelle explicite.
- **JUCE / Electron / Tauri** : voir `docs/decisions/0001-stack.md`.

## Pièges connus par crate

### cpal 0.18
- `build_output_stream(config, data_cb, err_cb, timeout)` : `StreamConfig` **par valeur** (0.17+).
- `StreamTrait::play` (renommé `start` dans la version suivante, `play` déprécié) — se référer au lock.
- Feature `realtime` (renommée depuis `audio_thread_priority`) : sans effet utile sur macOS.
- Le format par défaut peut être `I32`/`I24` sur certains périphériques : demander explicitement `F32`
  quand supporté, sinon convertir.
- Xruns signalés via `ErrorKind::Xrun` dans le callback d'erreur (0.18.2 CoreAudio).
- `DeviceTrait`/`StreamTrait` : `Send + Sync` dans la prochaine version ; en 0.18 le `Stream` peut ne pas
  être `Send` → le créer et le garder sur le thread qui le possède.

### midir 0.11
- Le callback reçoit `&mut T` (données utilisateur) : y placer le `rtrb::Producer`.
- `connect` consomme `MidiInput` ; garder la `MidiInputConnection` vivante (sinon déconnexion).
- Les noms de ports peuvent contenir l'index ; matcher par sous-chaîne `DJControl MIX`, insensible à la casse.

### egui / eframe 0.37
- Lire `research/upstream/egui-CHANGELOG.md` avant d'écrire du code d'UI (API `Context`, `Ui`,
  `ViewportBuilder`, visuels changent souvent).
- Ne pas laisser l'UI se redessiner en continu sans raison : `ctx.request_repaint_after(…)` cadencé par le
  transport (≈ 60 fps en lecture, rien à l'arrêt hors événement).
- Comparer RSS/CPU des rendus `glow` et `wgpu` (feature d'eframe) en phase 5 ; garder le plus sobre.
- Polices : en embarquer une seule (sous-ensemble Latin) via `FontDefinitions`.

### assert_no_alloc 1.1.2
- Feature `disable_release` par défaut : `AllocDisabler` **n'existe pas en release**. Déclarer l'allocateur
  global et les tests qui l'utilisent sous `#[cfg(debug_assertions)]` ; `assert_no_alloc(|| …)` reste
  disponible (no-op) en release.

### criterion 0.8
- `criterion::black_box` déprécié → `std::hint::black_box`.
- Comparer les benchs à alimentation égale (secteur/batterie : +48 % mesuré sur M1).

### symphonia 0.6
- Activer uniquement les codecs nécessaires (features) pour réduire binaire et temps de compilation.
- 015 : **pas utilisé** (ni `rubato`) — les kits sont convertis une fois en WAV 48 kHz par `afconvert`
  (`tools/import-kit-909.sh`), lus par `hound` 3.5 ; la lecture interpolée couvre 44,1 ↔ 48 kHz.

## Outillage

- Toolchain : `rustup` profil `minimal` + `rustfmt` + `clippy` (≈ 1 Go). Édition 2024.
- `cargo-bundle` 🔍 ou script maison pour produire `LiveDeck.app` (phase 6).
- Espace disque serré (cf. mémoire utilisateur) : `target/` peut dépasser 2–3 Go → `cargo clean` régulier,
  `CARGO_INCREMENTAL=0` en release, surveiller `df -h /`.

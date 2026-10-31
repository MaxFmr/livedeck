# Plan de développement LiveDeck

## Contexte

Le harnais est en place dans `~/dev/livedeck` (CLAUDE.md, ROADMAP.md, docs/, skills, tools/). Aucun code
n'existe encore et Rust n'est pas installé. 25 Go sont libres. Ce plan découpe la roadmap en **itérations
concrètes**.

Choix de l'utilisateur :
- **Tous les styles** : l'outil reste ouvert. Les styles deviennent des **presets** (kit, patterns, plage de
  BPM, swing, gamme) et non des constantes. Exemples : dub/steppa, afrobeat/reggae, UK garage, lo-fi/hip-hop,
  house/techno. BPM de 60 à 200, swing de 50 à 75 %.
- **Sons** : batterie synthétisée d'abord, puis lecteur de samples (packs de l'utilisateur, ex.
  `/Volumes/PHILIPS SOL/Samples/Roland TR-909`, **copiés** dans `assets/` et non lus sur le disque FAT32
  en jeu).
- **Validation à chaque itération** : je livre une étape jouable ou mesurable, l'utilisateur teste au
  contrôleur, puis on continue. Chaque itération suit le skill `/iteration` (journal
  `docs/iterations/NNN-*.md`, puis ROADMAP cochée).

Prérequis côté utilisateur, avant la première itération (le garde-fou m'empêche de le faire) :
`mv ~/dev/livedeck/.claude/settings.proposed.json ~/dev/livedeck/.claude/settings.json`, puis ouvrir une
session Claude dans `~/dev/livedeck` (chargement de CLAUDE.md et des skills).

## Mises à jour de la doc au démarrage (itération 001)

- `docs/music/live-engine.md` : ajouter une section « Presets de style » (structure `StylePreset` : kit,
  banque de patterns, plage de BPM, swing, gamme et progression par défaut) avec 5 presets de départ.
- `ROADMAP.md` : intégrer les presets de style en phase 3/4 et le lecteur de samples en phase 3b.

## Itérations

### Phase 0 — Fondations et vérité matérielle

| # | Incrément | Livrable / critère de fin | Test utilisateur |
|---|---|---|---|
| 001 | Toolchain + workspace | `rustup` profil minimal + rustfmt + clippy (après `df -h /`). Workspace édition 2024 : `crates/engine`, `crates/io`, `crates/app`, `tools/midi-sniff`. Profils et lints de `docs/rust/realtime-audio.md` §6, `git init`. `cargo clippy -D warnings` et `cargo test` verts. | — |
| 002 | `midi-sniff` | Liste les ports, se connecte à `DJControl`, affiche `t(ms) │ hex │ décodage` ; options `--send`, `--leds`, `--json`. Le décodeur `ControllerEvent` dans `crates/io/src/controller.rs` est testé sur les octets de la fiche. | Brancher le contrôleur en USB et voir défiler les messages |
| 003 | Session `/midi-sniff` | Checklist de `docs/hardware/djcontrol-mix.md` déroulée geste par geste ; fiche passée en 🧪 ; décodeur corrigé si besoin. | Guidé geste par geste (≈ 15 min) |

### Phase 1 — Moteur audio squelette

| # | Incrément | Livrable / critère de fin |
|---|---|---|
| 004 | Sortie audio + horloge | Stream cpal F32 48 kHz, buffer 128 (repli 256), horloge en échantillons, transport, métronome synthétique, limiteur master + garde NaN. Binaire de démo `cargo run -p app`. |
| 005 | Plomberie temps réel | `Command` (`Copy`, ≤ 32 o), files rtrb `Command`/`Garbage`/`EngineEvent`, `EngineSnapshot` en triple buffer, thread de contrôle, reconstruction du stream sur changement de périphérique, App Nap désactivé. |
| 006 | Filets de sécurité | Tests de rendu hors ligne, test `assert_no_alloc`, bench criterion, compteurs de charge et de xruns ; `/rt-audit` + `/perf-check`. **Sortie de phase** : 10 min, 0 xrun, charge < 10 %. |

### Phase 2 — Contrôleur → commandes

| # | Incrément | Livrable |
|---|---|---|
| 007 | Connexion live | midir dans `io`, hot-plug (reconnexion, dump `B0 7F 7F`, LEDs), état « contrôleur absent ». |
| 008 | Mapping | `config/mapping.toml` → table de dispatch ; Tempo 14 bits, jogs lissés, soft-takeover. Pads → métronome / transport pour tester. |
| 009 | LEDs + latence | Diff de LEDs sur le thread de contrôle ; latence geste→son mesurée (micro), < 10 ms. |

À partir d'ici, la **piste D** démarre en parallèle : D1 (intention, références visuelles) et D2
(inventaire de l'état), sous forme d'échanges courts entre deux itérations de code.

### Phase 3 — Deck rythme (puis 3b samples)

- 010 : voix de batterie synthétiques (kick, snare, hats + choke, clap, perc), rendues en WAV pour écoute.
- 011 : séquenceur 16 pas (vélocité, probabilité, swing) et 4 patterns quantifiés sur les pads Hot Cue.
- 012 : mutes (FX), beat-repeat (Loop), fills (Sampler), densité au fader Tempo.
- 013 : basse mono en degrés de gamme, glide, filtre.
- 014 : `StylePreset` + 5 presets (patterns et kits synthétiques par style).
- 015 (3b) : lecteur de samples (symphonia + rubato au chargement, pool de voix), import d'un kit TR-909 dans `assets/`.

### Phase 4 — Deck mélodie
- 016 : poly synthé 16 voix · 017 : gammes, accords par degrés, voicings · 018 : arpégiateur, phrases,
  transposition au jog · 019 : delay et réverb pré-alloués · **sortie** : morceau complet jouable.

### Piste D → Phase 5 (UI) → Phase 6 (scène) → Phase 7 (extensions)
Comme dans `ROADMAP.md` : maquettes D3/D4 en artifacts HTML validées par l'utilisateur **avant** l'UI egui ;
puis scènes et morph au crossfader, robustesse, endurance 1 h, `.app` ; puis MIDI out (FM-1), Link,
enregistrement.

## Fichiers critiques
- `~/dev/livedeck/ROADMAP.md`, `docs/iterations/` (journal), `docs/music/live-engine.md` (presets).
- À créer : `Cargo.toml` (workspace), `crates/{engine,io,app}/`, `tools/midi-sniff/`, `config/mapping.toml`.
- Références à suivre : `docs/rust/realtime-audio.md`, `docs/rust/stack.md` (versions épinglées :
  cpal 0.18.2, midir 0.11.1, egui/eframe 0.37.0, rtrb 0.4.1, triple_buffer 9.0.0), `docs/hardware/djcontrol-mix.md`.

## Vérification (à chaque itération)
- `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.
- `/rt-audit` dès que `crates/engine` ou un callback change ; `/perf-check` dès que le chemin audio ou l'UI change.
- Démonstration : sniffer ou app lancée dans un onglet Terminal, ou rendu WAV dans `target/renders/` ; test au contrôleur par l'utilisateur ; mesures consignées dans le journal.
- Surveiller `df -h /` et lancer `cargo clean` si `target/` dépasse ~3 Go.

## Première action après validation du plan
Itération 001 : vérifier l'espace disque, installer rustup (installeur officiel `https://sh.rustup.rs`,
profil minimal), créer le workspace, puis mettre à jour la doc (presets de style).

# LiveDeck

Instrument de musique live pour macOS, écrit en Rust et joué entièrement depuis un contrôleur DJ
**Hercules DJControl Mix**. Les deux decks ne mixent pas deux morceaux : ils forment les deux moitiés
d'un même groupe sur une horloge commune.

- **Deck gauche — rythme** : batterie (synthétisée ou samples TR-909) et basse mono.
- **Deck droit — mélodie** : synthé polyphonique, accords par degrés, voicings.

Tout reste dans la tonalité : patterns, lignes de basse et accords sont écrits en **degrés de gamme**,
donc changer de tonique ou de gamme transpose tout sans fausse note. Les changements musicaux (patterns,
styles, tonalité, gamme) tombent **à la mesure suivante**.

> État : phases 0 à 3 terminées, phase 4 (mélodie) en cours — itération 017 sur 019. Pas encore d'interface
> graphique : l'app tourne en console. Voir [ROADMAP.md](ROADMAP.md).

## Objectifs

| Exigence | Cible | Mesuré |
|---|---|---|
| Latence geste → son (USB) | < 10 ms | médiane 8,1–8,5 ms, max 10,3 ms ([009](docs/iterations/009-leds-latency.md)) |
| Décrochages audio (xruns) | 0 sur 1 h | 0 sur toutes les sessions d'essai ; endurance 1 h en phase 6 |
| Charge du callback (bloc de 128) | < 30 % toutes voix actives | 2,3 % au bench (16 voix de synthé + batterie) |
| Mémoire (RSS) | < 120 Mo | 11,6 Mo en phase 1 |

## Prérequis

- Mac Apple Silicon, macOS 15 (testé sur MacBook Air M1).
- Un Hercules DJControl Mix en USB. Sans contrôleur, l'app joue quand même et se pilote au clavier ;
  le contrôleur est détecté à chaud.
- Rust 1.99.1 (édition 2024), installé par `rustup` ; la version est épinglée dans
  [rust-toolchain.toml](rust-toolchain.toml).

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --profile minimal
```

## Démarrage

```bash
cargo run -p app --release
```

L'app ouvre la sortie audio par défaut (48 kHz, buffer de 128 trames, repli à 256), charge
[config/mapping.toml](config/mapping.toml) et le kit [assets/kits/tr909](assets/kits/tr909), puis démarre
la lecture en style « Hybride », pattern A, la mineur, 120 BPM.

| Option | Effet |
|---|---|
| `--bpm <60-200>` | tempo de départ |
| `--buffer <frames>` | taille de buffer audio demandée |
| `--mapping <fichier>` | autre mapping (défaut : `config/mapping.toml`, sinon celui intégré au binaire) |
| `--kit <dossier>` | autre kit de samples (`kick.wav`, `snare.wav`, `closed_hat.wav`, `open_hat.wav`, `clap.wav`, `perc.wav`) |
| `--no-metronome` | démarre métronome coupé |
| `--quiet` | n'affiche que les bilans (charge, xruns) et les changements, pas chaque temps |
| `--seconds <n>` | s'arrête après n secondes (essais d'endurance) |
| `--record <wav>` | enregistre la sortie (60 s au plus) |

Commandes clavier (taper puis Entrée) : `p` lecture/arrêt, `+` / `-` tempo ±1 BPM, `t <bpm>` tempo exact,
`m` métronome, `h` kick, `q` quitter.

## Jouer au contrôleur

Le contrôleur mémorise le mode de ses pads (Hot Cue, Sampler, FX, Loop) : choisir le mode avec les boutons
au-dessus des pads. **Attention** : Shift + bouton Hot Cue passe en mode FX, Shift + bouton Sampler en mode
Loop. Pour la couche Shift des pads, choisir le mode **sans** Shift, puis tenir Shift et taper le pad.

Les faders à double rôle (Volume A, Volume B) utilisent le **soft-takeover** : après un changement de couche,
le fader n'agit qu'une fois revenu à la valeur actuelle du réglage, pour éviter les sauts de niveau.

### Deck gauche — rythme

| Contrôle | Seul | Avec Shift |
|---|---|---|
| Pads **Hot Cue** 1–4 | patterns de batterie A–D avec leur ligne de basse, à la mesure | ligne de basse seule, à la mesure |
| Pads **Sampler** 1–4 | fills : roll de caisse claire, roll de perc, break de kick, montée de hats (de la noire suivante à la fin de la mesure) | sons seuls : kick, snare, hat fermé, hat ouvert |
| Pads **FX** 1–4 | mutes : kick · snare + clap · hats · perc (LED allumée = joue) | — |
| Pads **Loop** 1–4 (tenus) | beat-repeat de la batterie 1/4, 1/8, 1/16, 1/32 ; un pad plus court subdivise, relâcher revient au pad tenu en phase | — |
| Jog — bord | swing (50 à 75 %) | style suivant / précédent (un demi-tour par style), à la mesure |
| Jog — dessus | tempo | style |
| Fader **Tempo** | densité de la batterie (0 = kicks sur les temps → 1 = tout) | — |
| Potard **Bass** | filtre de la basse (60 Hz → 4 kHz) ; la bascule globale Filter/Bass doit être sur Bass | — |
| **Volume** | niveau de la batterie | niveau de la basse |
| **Cue** | mute de la basse (LED allumée = joue) | — |
| **PFL** | métronome on/off | batterie : samples 909 ↔ synthé, à la mesure |
| **Sync** | tap tempo | — |
| **Play** | lecture / arrêt | reprise à la mesure 1 |

### Deck droit — mélodie

| Contrôle | Seul | Avec Shift |
|---|---|---|
| Pads **Hot Cue** 1–4 | accords I, IV, V, vi, tenus tant que le pad l'est, joués tout de suite | ii, iii, vii, I à l'octave |
| Pads **Loop** 1 | latch : l'accord tient après le relâchement, jusqu'au suivant (LED allumée = actif) | — |
| Pads **Loop** 2 | renversement + | renversement − |
| Pads **Loop** 3 | octave + | octave − |
| Pads **Loop** 4 | extension suivante : triade → 7e → sus4 → add9 | retour à la triade |
| Jog | — | gamme (un demi-tour par gamme), à la mesure |
| Fader **Tempo** | brillance du synthé (filtre ±2,5 octaves autour du patch) | — |
| **Volume** | niveau de la mélodie | niveau du métronome |
| **Cue** | tonique −1 demi-ton, à la mesure | tonique +1 |
| **Play** | lecture / arrêt | — |

Un changement de voicing rejoue tout de suite les accords qui sonnent. Les accords sont joués une octave
au-dessus de la basse, dans sa tonalité.

### Global

| Contrôle | Fonction |
|---|---|
| **Master** | volume général (avant le limiteur de sécurité) |

Crossfader, Filter, Headphone et Vinyl ne sont pas encore affectés (morph de scènes en phase 6).

### Styles

Un style change d'un coup, à la mesure : les 4 patterns, les fills, les 4 lignes de basse, le son du kit
(accordage, longueur, niveau de chaque voix), le caractère de la basse, le patch du synthé et le swing.
Le tempo et la tonalité restent ceux du joueur ; la console affiche le tempo conseillé.

| Style | Tempo conseillé | Swing | Synthé |
|---|---|---|---|
| Hybride | 120 | 54 % sur 1/8 | pad chaud |
| Dub / steppa | 140 (half-time) | droit | skank court |
| Afrobeat / reggae | 108 | 58 % sur 1/16 | orgue |
| UK garage | 132 | 62 % sur 1/16 | stab résonant |
| Lo-fi / hip-hop | 86 | 60 % sur 1/16 | keys doux |
| House / techno | 124 | 52 % sur 1/16 | stab house |

Gammes disponibles : mineur, majeur, dorien, phrygien, mixolydien, penta mineure.

### Personnaliser le mapping

[config/mapping.toml](config/mapping.toml) relie chaque contrôle nommé (`a.play`, `b.pad.loop.3`,
`a.jog.edge`, `mixer.master`…) à une action, avec une couche `shift = true` optionnelle. La liste des
actions et de leurs paramètres est commentée en tête du fichier. Relancer l'app pour appliquer ; une erreur
de mapping est signalée avec le contrôle fautif.

## Architecture

```
crates/engine/   DSP, séquenceurs, horloge. Pur Rust, aucune E/S : testable hors ligne.
crates/io/       sortie audio (cpal), MIDI (midir), décodage du contrôleur, mapping, LEDs, kits WAV.
crates/app/      câblage des threads, console (UI egui en phase 5).
tools/midi-sniff affiche les messages du contrôleur (vérité matérielle).
config/          mapping du contrôleur.
assets/kits/     kits de samples (WAV 48 kHz mono).
docs/            connaissances de référence et journal des itérations.
```

Trois threads :

- **audio** (callback cpal) : rend les blocs ; ni allocation, ni verrou, ni E/S, ni panique ;
- **contrôle** : MIDI → mapping → commandes, LEDs, console, reconstruction du flux si la sortie change ;
- **MIDI** (CoreMIDI) : reçoit les messages horodatés.

Le contrôle parle au moteur par une file sans verrou de `Command` (`rtrb`) ; le moteur publie son état dans
un `EngineSnapshot` en triple buffer. Tout ce que le thread audio utilise est alloué avant le démarrage du
flux. Les règles complètes sont dans [docs/rust/realtime-audio.md](docs/rust/realtime-audio.md).

Chaîne audio : batterie → beat-repeat → niveau ; basse → niveau ; synthé → niveau ; métronome → somme →
volume master → limiteur (plafond −0,3 dBFS, garde NaN) → sortie stéréo.

## Développement

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                                    # 126 tests, dont un test sans allocation
cargo bench -p engine --bench process                     # coût d'un bloc de 128 trames
cargo run -p engine --example render_kit --release        # rendus WAV dans target/renders/
cargo run -p io --example render_samples --release        # synthé contre samples, par style
cargo run -p midi-sniff --release                         # messages bruts du contrôleur
cargo run -p io --example replay -- 90037F 97397F         # rejoue des octets MIDI dans le mapping
```

En debug, le callback audio est sous `assert_no_alloc` : toute allocation y fait échouer le programme.

Outils (`tools/`) :

| Script | Rôle |
|---|---|
| `try-NNN.sh` | essai guidé à la voix (`say`) de l'itération NNN : WAV d'écoute, puis étapes au contrôleur ; journal dans `target/NNN-test.txt`. `SKIP_WAV=1 try-017.sh 2 3` rejoue certaines étapes, `QUIET=` journalise chaque geste |
| `import-kit-909.sh` | importe un kit dans `assets/kits/tr909/` (conversion `afconvert` en 48 kHz mono 24 bits) |
| `perf.sh <binaire>` | RSS max et CPU d'un binaire |
| `refresh-knowledge.sh` | rafraîchit les sources de référence (mapping Mixxx, changelogs, versions des crates) |

Le projet avance par **itérations courtes** : chacune a un critère de fin mesurable, un essai au contrôleur
validé par le musicien, et un journal dans [docs/iterations/](docs/iterations/).

## Documentation

| Sujet | Fichier |
|---|---|
| Feuille de route et critères de sortie | [ROADMAP.md](ROADMAP.md) |
| Journal des itérations | [docs/iterations/](docs/iterations/) |
| Concept musical, modèle de données, mapping | [docs/music/live-engine.md](docs/music/live-engine.md) |
| Messages MIDI et LEDs du DJControl Mix | [docs/hardware/djcontrol-mix.md](docs/hardware/djcontrol-mix.md) |
| CoreMIDI / CoreAudio sur macOS | [docs/hardware/midi-macos.md](docs/hardware/midi-macos.md) |
| Règles temps réel | [docs/rust/realtime-audio.md](docs/rust/realtime-audio.md) |
| Crates et versions | [docs/rust/stack.md](docs/rust/stack.md) |
| Recettes DSP | [docs/rust/dsp-cookbook.md](docs/rust/dsp-cookbook.md) |
| Démarche de design et maquettes | [docs/design/](docs/design/), [design/mockups/](design/mockups/) |
| Décisions d'architecture | [docs/decisions/](docs/decisions/) |

## Licence

Code : MIT ou Apache-2.0, au choix.

Les samples de [assets/kits/tr909](assets/kits/tr909) proviennent de la bibliothèque **Battery 4 Factory
Library** de Native Instruments et restent soumis à sa licence : ils ne doivent pas être redistribués. C'est
pourquoi ce dépôt est privé.

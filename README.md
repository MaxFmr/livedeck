# LiveDeck

**[English](#english) · [Français](#français)**

---

## English

A live music instrument for macOS, written in Rust and played entirely from a **Hercules DJControl Mix**
DJ controller. The two decks do not mix two tracks: they are the two halves of one band, on a shared
clock.

- **Left deck — rhythm**: drums and a mono bass.
- **Right deck — melody**: polyphonic synth, chords by scale degree, voicings.

Everything stays in key: drum patterns, bass lines and chords are written in **scale degrees**, so changing
the tonic or the scale transposes everything without a wrong note. Musical changes (patterns, styles, key,
scale) land **on the next bar**.

> Status: phases 0–3 done, phase 4 (melody) in progress — iteration 017 of 019. No graphical interface
> yet: the app runs in the terminal. See [ROADMAP.md](ROADMAP.md) (in French).

### Targets

| Requirement | Target | Measured |
|---|---|---|
| Gesture → sound latency (USB) | < 10 ms | median 8.1–8.5 ms, max 10.3 ms ([009](docs/iterations/009-leds-latency.md)) |
| Audio dropouts (xruns) | 0 in 1 h | 0 in every test session; 1 h endurance run in phase 6 |
| Callback load (128-frame block) | < 30 % with every voice on | 2.3 % in the bench (16 synth voices + drums) |
| Memory (RSS) | < 120 MB | 11.6 MB in phase 1 |

### Requirements

- Apple Silicon Mac, macOS 15 (tested on a MacBook Air M1).
- A Hercules DJControl Mix over USB. Without it, the app still plays and takes keyboard commands; the
  controller is picked up when plugged in.
- Rust 1.99.1 (edition 2024) through `rustup`; the version is pinned in
  [rust-toolchain.toml](rust-toolchain.toml).

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --profile minimal
```

### Getting started

```bash
cargo run -p app --release
```

The app opens the default audio output (48 kHz, 128-frame buffer, 256 as a fallback), loads
[config/mapping.toml](config/mapping.toml), then starts playing: style “Hybrid”, pattern A, A minor,
120 BPM.

| Option | Effect |
|---|---|
| `--bpm <60-200>` | starting tempo |
| `--buffer <frames>` | requested audio buffer size |
| `--mapping <file>` | another mapping (default: `config/mapping.toml`, else the one built into the binary) |
| `--kit <folder>` | another drum kit (one WAV per voice) |
| `--no-metronome` | start with the metronome off |
| `--quiet` | print only reports (load, xruns) and changes, not every beat |
| `--seconds <n>` | stop after n seconds (endurance runs) |
| `--record <wav>` | record the output (60 s max) |

Keyboard commands (type, then Enter): `p` play/stop, `+` / `-` tempo ±1 BPM, `t <bpm>` exact tempo,
`m` metronome, `h` kick, `q` quit.

### Playing the controller

The controller remembers its pad mode (Hot Cue, Sampler, FX, Loop): pick it with the buttons above the
pads. **Careful**: Shift + Hot Cue mode button switches to FX mode, Shift + Sampler to Loop. For the Shift
layer of the pads, pick the mode **without** Shift, then hold Shift and hit the pad.

The dual-role faders (Volume A, Volume B) use **soft takeover**: after switching layer, the fader only acts
once it has come back to the setting's current value, so levels never jump.

#### Left deck — rhythm

| Control | Alone | With Shift |
|---|---|---|
| **Hot Cue** pads 1–4 | drum patterns A–D with their bass line, on the next bar | bass line alone, on the next bar |
| **Sampler** pads 1–4 | fills: snare roll, perc roll, kick break, hats rise (from the next beat to the end of the bar) | one-shots: kick, snare, closed hat, open hat |
| **FX** pads 1–4 | mutes: kick · snare + clap · hats · perc (LED on = playing) | — |
| **Loop** pads 1–4 (held) | drum beat-repeat 1/4, 1/8, 1/16, 1/32; a shorter pad subdivides, releasing goes back to the held pad in phase | — |
| Jog — edge | swing (50 to 75 %) | next / previous style (half a turn per style), on the next bar |
| Jog — top | tempo | style |
| **Tempo** fader | drum density (0 = kicks on the beat → 1 = everything) | — |
| **Bass** knob | bass filter (60 Hz → 4 kHz); the global Filter/Bass switch must be on Bass | — |
| **Volume** | drum level | bass level |
| **Cue** | bass mute (LED on = playing) | — |
| **PFL** | metronome on/off | switch drum kit, on the next bar |
| **Sync** | tap tempo | — |
| **Play** | play / stop | restart from bar 1 |

#### Right deck — melody

| Control | Alone | With Shift |
|---|---|---|
| **Hot Cue** pads 1–4 | chords I, IV, V, vi, held while the pad is, played right away | ii, iii, vii, I an octave up |
| **Sampler** pads 1–4 | phrases of the style, looped, started and stopped on the next bar | variation: second half a third higher |
| **FX** pads 1–4 | arpeggio: off · up · up-down · random (plays the held or latched chords while playing) | arpeggio over 1 to 4 octaves |
| **Loop** pad 1 | latch: the chord keeps sounding after release, until the next one (LED on = active) | — |
| **Loop** pad 2 | inversion + | inversion − |
| **Loop** pad 3 | octave + | octave − |
| **Loop** pad 4 | next extension: triad → 7th → sus4 → add9 | back to the triad |
| Jog — edge | arpeggio rate: 1/4, 1/8, 1/8T, 1/16, 1/16T, 1/32 (a quarter turn per rate) | scale (half a turn per scale), on the next bar |
| Jog — top | transpose chords, arpeggio and phrases within the scale (a quarter turn per degree, ±7), right away | scale |
| **Tempo** fader | synth brightness (filter ±2.5 octaves around the patch) | — |
| **Volume** | melody level | metronome level |
| **Cue** | tonic −1 semitone, on the next bar | tonic +1 |
| **Play** | play / stop | — |

A voicing change replays the chords that are sounding right away. Chords play an octave above the bass, in
its key.

#### Global

| Control | Function |
|---|---|
| **Master** | overall volume (before the safety limiter) |

Crossfader, Filter, Headphone and Vinyl are not assigned yet (scene morphing comes in phase 6).

#### Styles

A style switches everything at once, on the next bar: the 4 patterns, the fills, the 4 bass lines, the
drum sound (tuning, length, level of each voice), the bass character, the synth patch and the swing. Tempo
and key stay the player's; the terminal shows the suggested tempo.

| Style | Suggested tempo | Swing | Synth |
|---|---|---|---|
| Hybrid | 120 | 54 % on 1/8 | warm pad |
| Dub / steppa | 140 (half-time) | straight | short skank |
| Afrobeat / reggae | 108 | 58 % on 1/16 | organ |
| UK garage | 132 | 62 % on 1/16 | resonant stab |
| Lo-fi / hip-hop | 86 | 60 % on 1/16 | soft keys |
| House / techno | 124 | 52 % on 1/16 | house stab |

Scales: minor, major, dorian, phrygian, mixolydian, minor pentatonic.

#### Custom mapping

[config/mapping.toml](config/mapping.toml) binds each named control (`a.play`, `b.pad.loop.3`,
`a.jog.edge`, `mixer.master`…) to an action, with an optional `shift = true` layer. Actions and their
parameters are listed in comments at the top of the file. Restart the app to apply; a mapping error names
the faulty control.

### Architecture

```
crates/engine/   DSP, sequencers, clock. Pure Rust, no I/O: testable offline.
crates/io/       audio output (cpal), MIDI (midir), controller decoding, mapping, LEDs.
crates/app/      thread wiring, terminal front end (egui UI in phase 5).
tools/midi-sniff prints the controller's messages (hardware truth).
config/          controller mapping.
docs/            reference knowledge and iteration log (in French).
```

Three threads:

- **audio** (cpal callback): renders blocks; no allocation, lock, I/O or panic;
- **control**: MIDI → mapping → commands, LEDs, terminal, stream rebuild when the output device changes;
- **MIDI** (CoreMIDI): receives timestamped messages.

The control thread talks to the engine through a lock-free `Command` queue (`rtrb`); the engine publishes
its state in an `EngineSnapshot` triple buffer. Everything the audio thread uses is allocated before the
stream starts. The full rules are in [docs/rust/realtime-audio.md](docs/rust/realtime-audio.md).

Signal chain: drums → beat-repeat → level; bass → level; synth → level; metronome → sum → master volume →
limiter (−0.3 dBFS ceiling, NaN guard) → stereo output.

### Development

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                                    # 126 tests, including an allocation-free one
cargo bench -p engine --bench process                     # cost of a 128-frame block
cargo run -p engine --example render_kit --release        # WAV renders in target/renders/
cargo run -p midi-sniff --release                         # raw controller messages
cargo run -p io --example replay -- 90037F 97397F         # replays MIDI bytes through the mapping
```

In debug builds the audio callback runs under `assert_no_alloc`: any allocation there aborts.

Scripts (`tools/`):

| Script | Purpose |
|---|---|
| `try-NNN.sh` | voice-guided test (`say`) of iteration NNN: a WAV to listen to, then steps at the controller; log in `target/NNN-test.txt`. `SKIP_WAV=1 try-017.sh 2 3` replays some steps, `QUIET=` logs every gesture |
| `perf.sh <binary>` | peak RSS and CPU of a binary |
| `refresh-knowledge.sh` | refreshes reference sources (Mixxx mapping, changelogs, crate versions) |

The project moves in **short iterations**: each has a measurable exit criterion, a test at the controller
signed off by the musician, and a log in [docs/iterations/](docs/iterations/).

### Documentation (in French)

| Topic | File |
|---|---|
| Roadmap and exit criteria | [ROADMAP.md](ROADMAP.md) |
| Iteration log | [docs/iterations/](docs/iterations/) |
| Musical concept, data model, mapping | [docs/music/live-engine.md](docs/music/live-engine.md) |
| DJControl Mix MIDI messages and LEDs | [docs/hardware/djcontrol-mix.md](docs/hardware/djcontrol-mix.md) |
| CoreMIDI / CoreAudio on macOS | [docs/hardware/midi-macos.md](docs/hardware/midi-macos.md) |
| Real-time rules | [docs/rust/realtime-audio.md](docs/rust/realtime-audio.md) |
| Crates and versions | [docs/rust/stack.md](docs/rust/stack.md) |
| DSP recipes | [docs/rust/dsp-cookbook.md](docs/rust/dsp-cookbook.md) |
| Design process and mockups | [docs/design/](docs/design/), [design/mockups/](design/mockups/) |
| Architecture decisions | [docs/decisions/](docs/decisions/) |

### License

Code: MIT or Apache-2.0, at your option.

---

## Français

Instrument de musique live pour macOS, écrit en Rust et joué entièrement depuis un contrôleur DJ
**Hercules DJControl Mix**. Les deux decks ne mixent pas deux morceaux : ils forment les deux moitiés
d'un même groupe sur une horloge commune.

- **Deck gauche — rythme** : batterie et basse mono.
- **Deck droit — mélodie** : synthé polyphonique, accords par degrés, voicings.

Tout reste dans la tonalité : patterns, lignes de basse et accords sont écrits en **degrés de gamme**,
donc changer de tonique ou de gamme transpose tout sans fausse note. Les changements musicaux (patterns,
styles, tonalité, gamme) tombent **à la mesure suivante**.

> État : phases 0 à 3 terminées, phase 4 (mélodie) en cours — itération 017 sur 019. Pas encore d'interface
> graphique : l'app tourne en console. Voir [ROADMAP.md](ROADMAP.md).

### Objectifs

| Exigence | Cible | Mesuré |
|---|---|---|
| Latence geste → son (USB) | < 10 ms | médiane 8,1–8,5 ms, max 10,3 ms ([009](docs/iterations/009-leds-latency.md)) |
| Décrochages audio (xruns) | 0 sur 1 h | 0 sur toutes les sessions d'essai ; endurance 1 h en phase 6 |
| Charge du callback (bloc de 128) | < 30 % toutes voix actives | 2,3 % au bench (16 voix de synthé + batterie) |
| Mémoire (RSS) | < 120 Mo | 11,6 Mo en phase 1 |

### Prérequis

- Mac Apple Silicon, macOS 15 (testé sur MacBook Air M1).
- Un Hercules DJControl Mix en USB. Sans contrôleur, l'app joue quand même et se pilote au clavier ;
  le contrôleur est détecté à chaud.
- Rust 1.99.1 (édition 2024), installé par `rustup` ; la version est épinglée dans
  [rust-toolchain.toml](rust-toolchain.toml).

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --profile minimal
```

### Démarrage

```bash
cargo run -p app --release
```

L'app ouvre la sortie audio par défaut (48 kHz, buffer de 128 trames, repli à 256), charge
[config/mapping.toml](config/mapping.toml), puis démarre la lecture : style « Hybride », pattern A,
la mineur, 120 BPM.

| Option | Effet |
|---|---|
| `--bpm <60-200>` | tempo de départ |
| `--buffer <frames>` | taille de buffer audio demandée |
| `--mapping <fichier>` | autre mapping (défaut : `config/mapping.toml`, sinon celui intégré au binaire) |
| `--kit <dossier>` | autre kit de batterie (un WAV par voix) |
| `--no-metronome` | démarre métronome coupé |
| `--quiet` | n'affiche que les bilans (charge, xruns) et les changements, pas chaque temps |
| `--seconds <n>` | s'arrête après n secondes (essais d'endurance) |
| `--record <wav>` | enregistre la sortie (60 s au plus) |

Commandes clavier (taper puis Entrée) : `p` lecture/arrêt, `+` / `-` tempo ±1 BPM, `t <bpm>` tempo exact,
`m` métronome, `h` kick, `q` quitter.

### Jouer au contrôleur

Le contrôleur mémorise le mode de ses pads (Hot Cue, Sampler, FX, Loop) : choisir le mode avec les boutons
au-dessus des pads. **Attention** : Shift + bouton Hot Cue passe en mode FX, Shift + bouton Sampler en mode
Loop. Pour la couche Shift des pads, choisir le mode **sans** Shift, puis tenir Shift et taper le pad.

Les faders à double rôle (Volume A, Volume B) utilisent le **soft-takeover** : après un changement de couche,
le fader n'agit qu'une fois revenu à la valeur actuelle du réglage, pour éviter les sauts de niveau.

#### Deck gauche — rythme

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
| **PFL** | métronome on/off | change de kit de batterie, à la mesure |
| **Sync** | tap tempo | — |
| **Play** | lecture / arrêt | reprise à la mesure 1 |

#### Deck droit — mélodie

| Contrôle | Seul | Avec Shift |
|---|---|---|
| Pads **Hot Cue** 1–4 | accords I, IV, V, vi, tenus tant que le pad l'est, joués tout de suite | ii, iii, vii, I à l'octave |
| Pads **Sampler** 1–4 | phrases du style, en boucle, lancées et arrêtées à la mesure | variation : seconde moitié une tierce plus haut |
| Pads **FX** 1–4 | arpège : off · montant · aller-retour · aléatoire (joue les accords tenus ou latchés pendant la lecture) | arpège sur 1 à 4 octaves |
| Pad **Loop** 1 | latch : l'accord tient après le relâchement, jusqu'au suivant (LED allumée = actif) | — |
| Pad **Loop** 2 | renversement + | renversement − |
| Pad **Loop** 3 | octave + | octave − |
| Pad **Loop** 4 | extension suivante : triade → 7e → sus4 → add9 | retour à la triade |
| Jog — bord | vitesse d'arpège : 1/4, 1/8, 1/8T, 1/16, 1/16T, 1/32 (un quart de tour par vitesse) | gamme (un demi-tour par gamme), à la mesure |
| Jog — dessus | transposition des accords, de l'arpège et des phrases dans la gamme (un quart de tour par degré, ±7), tout de suite | gamme |
| Fader **Tempo** | brillance du synthé (filtre ±2,5 octaves autour du patch) | — |
| **Volume** | niveau de la mélodie | niveau du métronome |
| **Cue** | tonique −1 demi-ton, à la mesure | tonique +1 |
| **Play** | lecture / arrêt | — |

Un changement de voicing rejoue tout de suite les accords qui sonnent. Les accords sont joués une octave
au-dessus de la basse, dans sa tonalité.

#### Global

| Contrôle | Fonction |
|---|---|
| **Master** | volume général (avant le limiteur de sécurité) |

Crossfader, Filter, Headphone et Vinyl ne sont pas encore affectés (morph de scènes en phase 6).

#### Styles

Un style change d'un coup, à la mesure : les 4 patterns, les fills, les 4 lignes de basse, le son de la
batterie (accordage, longueur, niveau de chaque voix), le caractère de la basse, le patch du synthé et le
swing. Le tempo et la tonalité restent ceux du joueur ; la console affiche le tempo conseillé.

| Style | Tempo conseillé | Swing | Synthé |
|---|---|---|---|
| Hybride | 120 | 54 % sur 1/8 | pad chaud |
| Dub / steppa | 140 (half-time) | droit | skank court |
| Afrobeat / reggae | 108 | 58 % sur 1/16 | orgue |
| UK garage | 132 | 62 % sur 1/16 | stab résonant |
| Lo-fi / hip-hop | 86 | 60 % sur 1/16 | keys doux |
| House / techno | 124 | 52 % sur 1/16 | stab house |

Gammes : mineur, majeur, dorien, phrygien, mixolydien, penta mineure.

#### Personnaliser le mapping

[config/mapping.toml](config/mapping.toml) relie chaque contrôle nommé (`a.play`, `b.pad.loop.3`,
`a.jog.edge`, `mixer.master`…) à une action, avec une couche `shift = true` optionnelle. La liste des
actions et de leurs paramètres est commentée en tête du fichier. Relancer l'app pour appliquer ; une erreur
de mapping est signalée avec le contrôle fautif.

### Architecture

```
crates/engine/   DSP, séquenceurs, horloge. Pur Rust, aucune E/S : testable hors ligne.
crates/io/       sortie audio (cpal), MIDI (midir), décodage du contrôleur, mapping, LEDs.
crates/app/      câblage des threads, console (UI egui en phase 5).
tools/midi-sniff affiche les messages du contrôleur (vérité matérielle).
config/          mapping du contrôleur.
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

### Développement

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                                    # 126 tests, dont un test sans allocation
cargo bench -p engine --bench process                     # coût d'un bloc de 128 trames
cargo run -p engine --example render_kit --release        # rendus WAV dans target/renders/
cargo run -p midi-sniff --release                         # messages bruts du contrôleur
cargo run -p io --example replay -- 90037F 97397F         # rejoue des octets MIDI dans le mapping
```

En debug, le callback audio est sous `assert_no_alloc` : toute allocation y fait échouer le programme.

Outils (`tools/`) :

| Script | Rôle |
|---|---|
| `try-NNN.sh` | essai guidé à la voix (`say`) de l'itération NNN : WAV d'écoute, puis étapes au contrôleur ; journal dans `target/NNN-test.txt`. `SKIP_WAV=1 try-017.sh 2 3` rejoue certaines étapes, `QUIET=` journalise chaque geste |
| `perf.sh <binaire>` | RSS max et CPU d'un binaire |
| `refresh-knowledge.sh` | rafraîchit les sources de référence (mapping Mixxx, changelogs, versions des crates) |

Le projet avance par **itérations courtes** : chacune a un critère de fin mesurable, un essai au contrôleur
validé par le musicien, et un journal dans [docs/iterations/](docs/iterations/).

### Documentation

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

### Licence

Code : MIT ou Apache-2.0, au choix.

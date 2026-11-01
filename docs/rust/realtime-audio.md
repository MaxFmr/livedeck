# Rust audio temps réel — règles et architecture

Référence fondatrice : Ross Bencina, *Real-time audio programming 101: time waits for nothing*
(<http://www.rossbencina.com/code/real-time-audio-programming-101-time-waits-for-nothing>).
Budget d'un bloc de 128 échantillons à 48 kHz : **2,67 ms, quoi qu'il arrive**. Un dépassement = un clic
audible devant le public.

## 1. Les règles du thread audio (non négociables)

Dans le callback cpal et **tout ce qu'il appelle** :

| Interdit | Pourquoi | À la place |
|---|---|---|
| Allocation / libération (`Vec::push` au-delà de la capacité, `Box::new`, `String`, `format!`, `collect()`, `clone()` d'un `Vec`) | l'allocateur prend des verrous, durée non bornée | tout pré-allouer ; capacités fixes (`arrayvec`/`heapless` ou tableaux) |
| **Drop** d'un objet qui possède du tas (`Vec`, `Box`, `Arc` dont on est le dernier détenteur) | libération = allocateur | renvoyer l'objet au thread de contrôle par une file « poubelle » |
| `Mutex`, `RwLock`, `Condvar`, canaux bloquants, `std::sync::mpsc` | inversion de priorité | files SPSC sans verrou (`rtrb`), atomiques, `triple_buffer` |
| E/S : fichiers, réseau, `println!`, `log`/`tracing`, MIDI out | appels système | compteurs atomiques lus ailleurs ; file d'événements vers le contrôle |
| `panic!`, `unwrap()`, indexation hors bornes | un panic dans le callback = son coupé (et `panic=abort` = crash) | `get()`, `debug_assert!`, saturations ; calculer des index déjà bornés |
| Boucles non bornées, algorithmes en O(n²) sur des données variables | durée imprévisible | bornes fixes (polyphonie max, nombre d'événements par bloc) |
| Dénormaux (flottants ~1e-38 en fin de décroissance) | x100 en coût CPU | flush-to-zero : bit FZ (24) du FPCR sur aarch64, posé à **chaque** callback (`mrs`/`msr`, 2 instructions, pas de crate) — fait en 010 dans `io::audio` ; les enveloppes coupent aussi sous 1e-4 |
| Appels à `Instant::now()` en boucle | coûteux | une mesure par bloc max (instrumentation) |

**Application mécanique :**
- Crate `engine` sans dépendance à `cpal`/`midir`/`egui` → testable hors ligne.
- Lints dans `engine/src/lib.rs` : `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic,
  clippy::indexing_slicing)]` (avec `#[allow]` local justifié si vraiment nécessaire).
- En debug : allocateur global `assert_no_alloc` (`AllocDisabler`) et le corps du callback dans
  `assert_no_alloc(|| …)` → toute allocation dans l'audio fait planter les tests et le debug. Désactivé en
  release.
- Skill `/rt-audit` : revue systématique de tout diff qui touche `crates/engine`.

## 2. Architecture des threads

```
 ┌──────────────┐ MidiEvent(ts,bytes)  ┌──────────────────────┐  Command (rtrb SPSC)  ┌──────────────────┐
 │ CoreMIDI     │ ───── rtrb ────────▶ │ Thread de CONTRÔLE   │ ───────────────────▶  │ Thread AUDIO     │
 │ (midir cb)   │                      │ • mapping MIDI→cmd   │                       │ (cpal callback)  │
 └──────────────┘                      │ • chargement samples │ ◀── Garbage (rtrb) ── │ • horloge        │
                                       │ • LEDs (MIDI out)    │                       │ • séquenceurs    │
 ┌──────────────┐  UiCommand (rtrb)    │ • hot-plug, presets  │ ◀── EngineEvent ───── │ • voix / DSP     │
 │ Thread UI    │ ───────────────────▶ │ • allocation OK ici  │                       │ • mix + limiteur │
 │ (eframe)     │ ◀── EngineSnapshot (triple_buffer, écrit par l'audio, lu par l'UI) ─ │                  │
 └──────────────┘                      └──────────────────────┘                       └──────────────────┘
```

- **Une seule porte d'entrée vers l'audio** : `rtrb::Producer<Command>` détenu par le thread de contrôle
  (SPSC = un seul producteur). Le callback MIDI et l'UI passent par le contrôle. Exception tolérée si la
  latence l'exige (jog) : une seconde file SPSC dédiée MIDI → audio, consommée en tête de bloc.
- `Command` est un `enum` **`Copy`, petit (≤ 32 octets)**, sans tas. Pour transmettre une ressource lourde
  (sample décodé, nouveau pattern), on envoie un pointeur déjà construit
  (`Box<SampleBuffer>` / `Arc<[f32]>`) et l'audio **renvoie l'ancien** dans la file poubelle.
  Alternative : `basedrop` (`Shared`/`Collector`) — plus confortable, mais crate peu maintenue : préférer
  la file poubelle explicite.
- `EngineSnapshot` (position, pattern courant/en attente, mutes, niveaux, charge CPU…) : struct `Copy`
  écrite une fois par bloc dans un `triple_buffer::Input` → l'UI lit le plus récent sans jamais bloquer.
- Compteurs (xruns, charge max, événements perdus) : `AtomicU32`/`AtomicU64` en `Relaxed`.

## 3. Horodatage et placement à l'échantillon

- midir donne `ts_µs` ; cpal donne `OutputCallbackInfo::timestamp()` en 0.18 (`callback` et `playback`
  en `StreamInstant` — fusionné en `CallbackInfo`, `playback`→`device` dans la version suivante). Au premier bloc, calibrer une correspondance horloge MIDI ↔ horloge audio.
- Événement **immédiat** : offset dans le bloc = `(ts_event + latence_fixe − t_bloc) * sr`, borné à
  `[0, frames)`. Une latence fixe constante (ex. 1 bloc) supprime la gigue au prix de 2,7 ms.
- Événement **quantifié** : l'audio calcule la prochaine frontière (1/16, temps, mesure, 4 mesures) et
  l'applique exactement à l'échantillon de cette frontière.
- Traitement en sous-blocs : découper le bloc aux offsets des événements, traiter chaque segment.

## 4. Horloge musicale

- Position en **échantillons (`u64`)** comme vérité ; position en temps (`f64` beats) dérivée. Ne jamais
  accumuler des `f32` (dérive).
- Changement de tempo : appliqué à une frontière de temps ou lissé ; la position en beats doit rester
  continue.
- Ableton Link (phase 7) : crate `rusty_link` 🔍 — la session Link devient la source de tempo/phase.

## 5. Mémoire : où va chaque octet

| Poste | Taille typique | Règle |
|---|---|---|
| Samples de batterie (mono f32 48 k) | 1 s = 188 Ko ; kit de 16 one-shots ≈ 3 Mo | décodés une fois (`symphonia`), rééchantillonnés à la fréquence du périphérique (`rubato`) au chargement, jamais en jeu |
| Voix de synthé | quelques Ko chacune | pool fixe (ex. 16 voix poly + 1 basse mono + 8 voix batterie) alloué à l'init |
| Délai / réverbération | 2 s stéréo ≈ 750 Ko | lignes à retard pré-allouées à la taille max |
| UI (egui + rendu GPU) | 30–80 Mo RSS | principal poste ; mesurer glow vs wgpu ; une seule police embarquée |
| **Cible totale** | **< 120 Mo RSS** | mesurée par `/perf-check` |

## 6. Profils Cargo

```toml
[profile.release]
lto = "fat"
codegen-units = 1
panic = "abort"      # pas de déroulement ; un panic = bug à corriger, pas à rattraper
debug = "line-tables-only"  # symboles pour Instruments sans gonfler

[profile.dev.package."*"]
opt-level = 2        # dépendances optimisées : l'audio reste jouable en debug
[profile.dev.package.engine]
opt-level = 2
```

## 7. Tests

- **Rendu hors ligne** : `engine` expose `render(&mut self, out: &mut [f32])` sans cpal → tests qui
  rendent N secondes et vérifient : pas de NaN/inf, crête ≤ 0 dBFS après limiteur, silence quand tout est
  muté, énergie au bon endroit (le kick tombe à l'échantillon prévu).
- **Golden files** : écrire des WAV (`hound`) dans `target/renders/` pour écoute ; ne pas les committer.
- **Tests sans allocation** : un test qui exécute 10 s de rendu sous `assert_no_alloc`.
- **Bench** : `criterion` sur `render` d'un bloc avec la charge max (toutes voix actives) → suivi de régression.
- **Propriétés** : fuzz léger des commandes (séquences aléatoires de `Command`) → jamais de panic.

## 8. Mesure en conditions réelles

- Charge du callback = durée mesurée / budget du bloc, max glissant publié dans le snapshot.
- Xruns : erreurs `ErrorKind::Xrun` (cpal 0.18.2+ sur CoreAudio) comptées.
- RSS / CPU : `/usr/bin/time -l` (max resident set size), `ps -o rss,%cpu`, Instruments (`xctrace record
  --template 'Time Profiler'`, `'Allocations'`).
- Test d'endurance avant tout live : 1 h de jeu, 0 xrun, RSS stable (pas de fuite).

# LiveDeck — instrument live piloté par un Hercules DJControl Mix

Application desktop native (Rust) qui transforme le DJControl Mix en instrument de musique live :
**deck gauche = rythme (batterie + basse)**, **deck droit = mélodie (accords, arpèges, phrases)**, sur une
horloge commune. Objectifs non fonctionnels : latence geste→son < 10 ms en USB, 0 xrun sur 1 h, RSS < 120 Mo,
CPU à l'arrêt ≈ 0.

L'utilisateur est musicien (@kerliwski), francophone : **répondre et documenter en français** ; le code,
les identifiants et les commentaires de code sont en anglais (usage Rust).

## Où est quoi

| Besoin | Fichier |
|---|---|
| Feuille de route, phase en cours, critères de sortie | [ROADMAP.md](ROADMAP.md) |
| Plan détaillé des itérations 001–019 (validé le 2026-10-31) | [docs/plan-iterations.md](docs/plan-iterations.md) |
| Journal des itérations (ce qui a été fait, appris, mesuré) | [docs/iterations/](docs/iterations/) |
| Messages MIDI du contrôleur, LEDs, inconnues à vérifier | [docs/hardware/djcontrol-mix.md](docs/hardware/djcontrol-mix.md) |
| CoreMIDI / CoreAudio, latence, pièges macOS | [docs/hardware/midi-macos.md](docs/hardware/midi-macos.md) |
| Règles temps réel, threads, mémoire, tests, profils | [docs/rust/realtime-audio.md](docs/rust/realtime-audio.md) |
| Crates retenues, versions, pièges d'API | [docs/rust/stack.md](docs/rust/stack.md) |
| Oscillateurs, filtres, enveloppes, batterie, effets | [docs/rust/dsp-cookbook.md](docs/rust/dsp-cookbook.md) |
| Concept musical, modèle de données, mapping v0 | [docs/music/live-engine.md](docs/music/live-engine.md) |
| Démarche et principes de design | [docs/design/design-process.md](docs/design/design-process.md) |
| Décisions d'architecture (ADR) | [docs/decisions/](docs/decisions/) |
| Sources brutes (mapping Mixxx, changelogs) | `research/` (données, ne pas copier de code GPL) |

## Structure cible du code

```
Cargo.toml            workspace (édition 2024, profils de docs/rust/realtime-audio.md §6)
crates/engine/        DSP + séquenceurs + horloge. Pur Rust, AUCUNE E/S, aucune dépendance cpal/midir/egui.
crates/io/            cpal (sortie), midir (entrée/sortie), mapping.toml → Command, LEDs, hot-plug
crates/app/           eframe/egui : UI, theme.rs, câblage des threads
tools/midi-sniff/     sniffer MIDI (phase 0)
config/mapping.toml   mapping contrôleur → actions
assets/               samples, police
```

## Commandes

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p midi-sniff              # affiche les messages du contrôleur
cargo run -p app --release           # l'application
tools/refresh-knowledge.sh           # rafraîchit sources Mixxx, changelogs, versions des crates
tools/perf.sh <binaire> [args]       # RSS max + CPU d'un binaire
```

## Règles d'or

1. **Temps réel** : rien dans le thread audio qui alloue, libère, verrouille, fait des E/S ou peut paniquer
   (table complète dans `docs/rust/realtime-audio.md` §1). Tout diff qui touche `crates/engine` ou un
   callback audio passe par le skill `rt-audit` avant d'être déclaré terminé.
2. **API externes vérifiées, jamais de mémoire** : cpal 0.18, egui/eframe 0.37, midir 0.11 ont changé
   récemment. Lire docs.rs pour la version du `Cargo.lock` ou `research/upstream/*-CHANGELOG.md`.
3. **Le matériel fait foi** : un fait sur le contrôleur marqué 🔍 n'est pas acquis ; le vérifier avec
   `midi-sniff` et mettre à jour `docs/hardware/djcontrol-mix.md` (🧪 + date).
4. **Mesurer, pas supposer** : toute affirmation de perf (CPU, RSS, latence) vient d'une mesure consignée
   dans le journal d'itération.
5. **Tester hors ligne** : le moteur se teste par rendu en mémoire (pas besoin de carte son ni de contrôleur).
6. **Itérations courtes** : une itération = un incrément jouable ou mesurable, avec critère de fin, suivie
   par le skill `iteration`. Pas de grand chantier sans journal.
7. **Design avant UI** : pas d'UI egui définitive sans maquette validée par l'utilisateur (piste D).
8. **Disque presque plein** sur cette machine : vérifier `df -h /` avant d'installer ou de compiler en
   release ; `cargo clean` quand `target/` dépasse ~3 Go ; ne pas ajouter de dépendance lourde sans raison.
9. Ne pas committer sans demande de l'utilisateur ; ne jamais committer `target/` ni des rendus WAV.

## Skills du projet (`.claude/skills/`)

| Skill | Quand |
|---|---|
| `djcontrol-mix` | tout ce qui touche au contrôleur : messages, LEDs, mapping, sniffing |
| `rt-audio-rust` | écrire ou modifier du code audio/MIDI temps réel, DSP, threads |
| `live-music` | séquençage, théorie, mapping musical, sons |
| `ui-design` | maquettes, système de design, UI egui |
| `/iteration` | démarrer / poursuivre / clôturer une itération de la roadmap |
| `/rt-audit` | auditer un diff ou un fichier pour le temps réel |
| `/midi-sniff` | vérifier le comportement réel du contrôleur et mettre à jour la fiche |
| `/perf-check` | mesurer latence, charge du callback, xruns, RSS, CPU |
| `/design-review` | revue d'une maquette ou de l'UI contre la checklist |
| `/refresh-knowledge` | rafraîchir sources, versions et changelogs |

# Feuille de route LiveDeck

Chaque phase se découpe en itérations courtes (skill `/iteration`), chacune avec un journal dans
`docs/iterations/NNN-titre.md`. Une phase est terminée quand **tous** ses critères de sortie sont mesurés
et notés. La piste **D (design)** avance en parallèle à partir de la phase 2.

Légende : `[ ]` à faire · `[~]` en cours · `[x]` fait (avec renvoi au journal).

---

## Phase 0 — Fondations et vérité matérielle

- [x] Installer la toolchain (`rustup` profil minimal + rustfmt + clippy), vérifier l'espace disque. → [001](docs/iterations/001-toolchain-workspace.md)
- [x] Workspace Cargo (`engine`, `io`, `app`, `tools/midi-sniff`), profils, lints, `.gitignore`, `git init`. → [001](docs/iterations/001-toolchain-workspace.md)
- [x] `midi-sniff` : liste les ports, affiche chaque message (hex + décodage via la table de la fiche),
      option `--send` pour tester les LEDs, option `--dump` (`B0 7F 7F`). → [002](docs/iterations/002-midi-sniff.md)
- [x] Dérouler la checklist de `docs/hardware/djcontrol-mix.md` sur le vrai contrôleur, tout passer en 🧪. → [003](docs/iterations/003-hardware-truth.md)
- [x] Mesurer le débit des jogs et la plage réelle des faders. → [003](docs/iterations/003-hardware-truth.md)

**Sortie** : fiche matérielle sans 🔍 bloquant ; `cargo clippy -D warnings` et `cargo test` passent.

## Phase 1 — Moteur audio squelette

- [x] Sortie cpal F32 48 kHz, buffer 128 (repli 256). → [004](docs/iterations/004-audio-clock.md)
- [x] Survie au débranchement de la carte son (re-routage macOS ; reconstruction codée). → [005](docs/iterations/005-rt-plumbing.md)
- [x] Horloge (position en échantillons), transport start/stop, métronome synthétique. → [004](docs/iterations/004-audio-clock.md)
- [x] Files `Command` / `EngineEvent` (rtrb), `EngineSnapshot` (triple_buffer). → [005](docs/iterations/005-rt-plumbing.md) (`Garbage` reportée aux samples, 015)
- [x] Limiteur de sécurité master + garde NaN. → [004](docs/iterations/004-audio-clock.md)
- [x] Instrumentation : charge du callback (max glissant), compteur et horodatage des xruns. → [006](docs/iterations/006-safety-nets.md)
- [x] Tests hors ligne (rendu, pas de NaN, crête), test sous `assert_no_alloc`, bench criterion. → [006](docs/iterations/006-safety-nets.md)
- [x] App Nap / veille désactivées pendant la session. → [005](docs/iterations/005-rt-plumbing.md)

**Sortie** : 10 min de métronome à 128 frames, 0 xrun, charge < 10 %, RSS mesuré. ✅ (600 s, 0 xrun, 0,9 %, 11,6 Mo)

## Phase 2 — Contrôleur → commandes

- [x] Connexion midir + hot-plug (reconnexion auto + renvoi du dump et des LEDs). → [007](docs/iterations/007-controller-live.md)
- [x] `config/mapping.toml` → table de dispatch précompilée ; actions typées. → [008](docs/iterations/008-mapping.md)
- [x] Fader Tempo 14 bits, jogs filtrés (anti-rebond du toucher), soft-takeover (pickup), tap tempo. → [008](docs/iterations/008-mapping.md)
- [x] **Retour LEDs (exigence utilisateur)** — banque + diff + renvoi par mode (007) ; états `LedState`
      calés sur le tempo (fixe, attente, attention, flashs de temps), pads allumés pendant l'appui (009).
      Les états musicaux des decks (pattern en attente, mutes) s'ajoutent avec eux (011–012).
      → [007](docs/iterations/007-controller-live.md), [009](docs/iterations/009-leds-latency.md)
- [x] Mesure de latence geste → son : horodatage CoreMIDI → sortie, médiane 8,1–8,5 ms, max 10,3 (26 frappes).
      → [009](docs/iterations/009-leds-latency.md)

**Sortie** : chaque contrôle physique a une action observable ; latence USB mesurée < 10 ms. ✅ latence
(médiane 8,1–8,5 ms, 1 frappe sur 26 à 10,3) ; contrôles : transport, tempo, niveaux, pads Hot Cue liés —
filtres, basses, crossfader et pads des autres modes recevront leurs actions avec les decks (010+).

## Phase 3 — Deck A : rythme

- [x] Voix de batterie synthétiques (kick, snare, hats avec choke, clap, perc). → [010](docs/iterations/010-drum-voices.md)
- [x] Séquenceur 16 pas (vélocité, probabilité, swing), 4 patterns + changement quantifié. → [011](docs/iterations/011-sequencer.md)
- [x] Mutes, fills, densité ordonnée, beat-repeat. → [012](docs/iterations/012-drum-gestures.md)
- [x] Basse mono (glide, accent, filtre), patterns en degrés de gamme. → [013](docs/iterations/013-bass.md)
- [x] `StylePreset` + 5 presets de départ (dub/steppa, afrobeat/reggae, UK garage, lo-fi, house/techno) — voir `docs/music/live-engine.md`. → [014](docs/iterations/014-style-presets.md)
- [ ] **3b** Lecteur de samples (chargement hors audio, `symphonia` + `rubato`, pool de voix) ; kits copiés dans `assets/` (ex. TR-909 du disque PHILIPS SOL), jamais lus depuis le disque externe en jeu.

**Sortie** : 15 min de jeu rythmique au contrôleur sans clic ni xrun ; valider le ressenti avec l'utilisateur.

## Phase 4 — Deck B : mélodie

- [ ] Poly synthé 16 voix (2 osc PolyBLEP, SVF, 2 ADSR), vol de voix.
- [ ] Gammes, tonalité, accords par degrés, voicings, renversements.
- [ ] Arpégiateur, phrases, transposition dans la gamme au jog.
- [ ] Delay synchronisé + réverb (pré-alloués), envois.

**Sortie** : morceau complet jouable (rythme + mélodie) ; charge < 30 % toutes voix actives.

## Piste D — Design (parallèle à partir de la phase 2)

- [x] D1 Intention et références (avec l'utilisateur). → [D1](docs/design/D1-intention.md)
- [x] D2 Architecture d'information (inventaire de l'état à afficher). → [D2](docs/design/D2-information.md)
- [x] D3 2–3 maquettes basse fidélité (artifacts HTML). → sources `design/mockups/`, [canvas](https://claude.ai/artifact/SEij2MqjacTXCRc8kuJKU6) : rangée de pas, afficheur, formes colorées — **retenue : 1 · rangée de pas** (2026-11-01).
- [x] D4 Maquette haute fidélité interactive, validée par l'utilisateur (2026-11-01). → `design/mockups/project/D4.dc.html`, [canvas](https://claude.ai/artifact/SEij2MqjacTXCRc8kuJKU6)
- [ ] D5 Système de design → `theme.rs`.

## Phase 5 — Interface egui

- [ ] Coquille eframe, thème, police embarquée ; comparaison RSS/CPU glow vs wgpu.
- [ ] Widgets : barre de temps, pads, grille de pas, accords/gamme, HUD de paramètres, vumètres.
- [ ] États : en attente, actif, muté, pickup, contrôleur absent, surcharge CPU.
- [ ] Repaint cadencé ; CPU UI à l'arrêt ≈ 0.
- [ ] D7 Revue sur scène (`/design-review`).

**Sortie** : UI conforme à la maquette D4 ; RSS total < 120 Mo ; checklist design verte.

## Phase 6 — Prêt pour la scène

- [ ] Scènes (8), presets de session (save/load TOML), morph au crossfader.
- [ ] Robustesse : débranchement contrôleur/casque en jeu, relance du stream, aucune perte de session.
- [ ] Endurance : 1 h, 0 xrun, RSS stable.
- [ ] Paquet `LiveDeck.app` (icône, signature ad hoc), démarrage < 1 s.

## Phase 7 — Extensions

- [ ] Sortie MIDI vers synthés externes (FM-1…) depuis le deck mélodie, horloge MIDI.
- [ ] Ableton Link.
- [ ] Enregistrement du set en WAV (thread disque, ring buffer).
- [ ] Vue « show » plein écran (2e écran / projecteur) animée par la musique — choix D1 : « visuel assumé, plus tard ».
- [ ] BLE MIDI (latence mesurée), éventuellement iPad (moteur réutilisé via FFI).

---

## Journal des phases

| Date | Évènement |
|---|---|
| 2026-11-01 | **Phase 2 terminée** (007–009) : contrôleur à chaud, mapping configurable, LEDs d'état en rythme, latence geste → son mesurée (médiane ≈ 8 ms). Prochaine : piste D (D1) et phase 3, itération 010. |
| 2026-10-31 | Harnais créé : base de connaissances, skills, roadmap. Phase 0 à démarrer. |
| 2026-10-31 | Plan détaillé validé (itérations 001–019, validation par l'utilisateur à chaque itération, tous styles via presets, synthèse puis samples). Itération 001 terminée. |
| 2026-10-31 | **Phase 1 terminée** (004–006) : sortie audio, horloge, commandes sans verrou, garde anti-allocation ; endurance 10 min 0 xrun, 0,9 % de charge, 11,6 Mo. Prochaine : phase 2, itération 007. |
| 2026-10-31 | **Phase 0 terminée** (002 sniffer, 003 vérité matérielle) : continus en 14 bits, LEDs cartographiées (pads, transport, Vinyl), toucher du jog à filtrer. Prochaine : phase 1, itération 004. |

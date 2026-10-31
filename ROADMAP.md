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
- [ ] Reconstruction du stream sur changement de périphérique (005).
- [x] Horloge (position en échantillons), transport start/stop, métronome synthétique. → [004](docs/iterations/004-audio-clock.md)
- [ ] Files `Command` / `Garbage` / `EngineEvent` (rtrb), `EngineSnapshot` (triple_buffer).
- [x] Limiteur de sécurité master + garde NaN. → [004](docs/iterations/004-audio-clock.md)
- [ ] Instrumentation : charge du callback (max glissant), compteur de xruns.
- [ ] Tests hors ligne (rendu, pas de NaN, crête), test sous `assert_no_alloc`, bench criterion.
- [ ] App Nap / veille désactivées pendant la session (🔍 à valider).

**Sortie** : 10 min de métronome à 128 frames, 0 xrun, charge < 10 %, RSS mesuré.

## Phase 2 — Contrôleur → commandes

- [ ] Connexion midir + hot-plug (reconnexion auto + renvoi du dump et des LEDs).
- [ ] `config/mapping.toml` → table de dispatch précompilée ; actions typées.
- [ ] Fader Tempo 14 bits, jogs relatifs lissés, soft-takeover (pickup).
- [ ] **Retour LEDs (exigence utilisateur)** : état LED des 4 modes de pads × 2 decks + Sync/Cue/Play/Vinyl
      tenu sur le thread de contrôle, diff envoyé via `led_message`, renvoi du mode affiché à chaque
      `PadModeSelect`, clignotement logiciel pour « en attente » (tout-ou-rien seulement), port gardé ouvert.
- [ ] Mesure de latence geste → son (enregistrement micro du clic du pad + son, ou boucle MIDI).

**Sortie** : chaque contrôle physique a une action observable ; latence USB mesurée < 10 ms.

## Phase 3 — Deck A : rythme

- [ ] Voix de batterie synthétiques (kick, snare, hats avec choke, clap, perc).
- [ ] Séquenceur 16 pas (vélocité, probabilité, swing), 4 patterns + changement quantifié.
- [ ] Mutes, fills, densité ordonnée, beat-repeat.
- [ ] Basse mono (glide, accent, filtre), patterns en degrés de gamme.
- [ ] `StylePreset` + 5 presets de départ (dub/steppa, afrobeat/reggae, UK garage, lo-fi, house/techno) — voir `docs/music/live-engine.md`.
- [ ] **3b** Lecteur de samples (chargement hors audio, `symphonia` + `rubato`, pool de voix) ; kits copiés dans `assets/` (ex. TR-909 du disque PHILIPS SOL), jamais lus depuis le disque externe en jeu.

**Sortie** : 15 min de jeu rythmique au contrôleur sans clic ni xrun ; valider le ressenti avec l'utilisateur.

## Phase 4 — Deck B : mélodie

- [ ] Poly synthé 16 voix (2 osc PolyBLEP, SVF, 2 ADSR), vol de voix.
- [ ] Gammes, tonalité, accords par degrés, voicings, renversements.
- [ ] Arpégiateur, phrases, transposition dans la gamme au jog.
- [ ] Delay synchronisé + réverb (pré-alloués), envois.

**Sortie** : morceau complet jouable (rythme + mélodie) ; charge < 30 % toutes voix actives.

## Piste D — Design (parallèle à partir de la phase 2)

- [ ] D1 Intention et références (avec l'utilisateur).
- [ ] D2 Architecture d'information (inventaire de l'état à afficher).
- [ ] D3 2–3 maquettes basse fidélité (artifacts HTML).
- [ ] D4 Maquette haute fidélité interactive, validée par l'utilisateur.
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
- [ ] BLE MIDI (latence mesurée), éventuellement iPad (moteur réutilisé via FFI).

---

## Journal des phases

| Date | Évènement |
|---|---|
| 2026-10-31 | Harnais créé : base de connaissances, skills, roadmap. Phase 0 à démarrer. |
| 2026-10-31 | Plan détaillé validé (itérations 001–019, validation par l'utilisateur à chaque itération, tous styles via presets, synthèse puis samples). Itération 001 terminée. |
| 2026-10-31 | **Phase 0 terminée** (002 sniffer, 003 vérité matérielle) : continus en 14 bits, LEDs cartographiées (pads, transport, Vinyl), toucher du jog à filtrer. Prochaine : phase 1, itération 004. |

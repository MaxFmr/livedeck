# Moteur musical live — concept et mapping

## Principe

Pas deux platines : **deux moitiés d'un même groupe** sur une horloge commune (tempo, tonalité, gamme).
Tout ce qui change la structure est **quantifié** (ça ne peut pas « tomber à côté ») ; tout ce qui sculpte
le son est **immédiat** (potards, jogs).

- **Deck A (gauche) — Rythme** : batterie (kick, snare, hats, clap/perc) + basse.
- **Deck B (droite) — Mélodie** : accords, arpèges, phrases, toujours dans la gamme.

## Modèle de données

```
Session
 ├─ tempo (BPM, 60–200), signature 4/4 (v1), swing global
 ├─ key (tonique 0–11) + scale (majeur, mineur nat., dorien, phrygien, mixolydien, penta min/maj, …)
 ├─ scenes[8] : instantané des choix de patterns/mutes/paramètres → rappel quantifié
 ├─ RhythmDeck
 │   ├─ drum_patterns[banks][4] : 16 pas × 4–6 voix (vélocité, probabilité, micro-décalage)
 │   ├─ bass_patterns[4] : 16 pas (degré de gamme, octave, durée, accent, glide)
 │   ├─ mutes, fill en attente, densité (0–1)
 └─ MelodyDeck
     ├─ progression : degrés (I..VII) + qualité déduite de la gamme, extensions (7, sus, add9)
     ├─ arp : mode (up/down/updown/random/chord), rate (1/4…1/32), octaves, gate
     ├─ phrases[4] : motifs mélodiques en degrés
     └─ voicing : renversement, registre, spread
```

Les patterns sont exprimés en **degrés de gamme**, jamais en notes absolues : changer de tonalité ou de
mode transpose tout, sans fausse note.

## Presets de style

Choix de l'utilisateur (2026-10-31) : **l'outil reste ouvert à tous les styles**. Aucun style n'est codé en
dur ; un style est un preset chargé sur le thread de contrôle et envoyé au moteur.

```
StylePreset
 ├─ name
 ├─ bpm_default, bpm_range (dans 60–200)
 ├─ swing (50 % = droit … 75 % = shuffle marqué), grille de swing (1/8 ou 1/16)
 ├─ kit : paramètres des voix synthétiques (ou kit de samples, phase 3b)
 ├─ drum_patterns[4] + fills, bass_patterns[4]
 ├─ key + scale par défaut, progression par défaut (degrés)
 └─ fx : envois delay/réverb par défaut, temps de delay (1/8 pointé, 1/4…)
```

| Preset de départ | BPM | Swing | Caractère |
|---|---|---|---|
| Dub / steppa | 70 (half-time) / 140 | 50 % | kick sur chaque temps (steppa) ou one-drop, basse sub dominante, delay 1/8 pointé, réverb longue |
| Afrobeat / reggae | 100–115 | 58 % | percussions en polyrythmie (euclidiens), skank sur le contretemps, basse mélodique |
| UK garage | 130–136 | 62 % sur 1/16 | 2-step (kicks décalés), hats shuffle, basse en notes courtes |
| Lo-fi / hip-hop | 80–92 | 60 % | kick/snare lourds et légèrement en retard, accords 7/9, filtre adouci |
| House / techno | 120–130 | 50–54 % | four-on-the-floor, open hat en contretemps, stabs d'accords |

Ces presets sont des points de départ ; tous les paramètres restent modifiables en jeu et enregistrables en
scène.

## Quantification

| Action | Grille par défaut |
|---|---|
| Changer de pattern / phrase / scène | fin de mesure (réglable : 1, 2, 4 mesures) |
| Mute / unmute | prochain temps |
| Fill | joué sur la dernière mesure avant la frontière |
| Accord (pads deck B) | prochain 1/8 (ou immédiat en mode « jeu libre ») |
| Beat-repeat | prochain 1/16 |

L'UI montre **ce qui joue** et **ce qui est en attente** (voir design).

## Mapping v0 (proposition — à valider en jouant)

Le contrôleur mémorise le mode des pads : on reçoit 4 modes × 4 pads × Shift = 32 fonctions par deck
(`docs/hardware/djcontrol-mix.md`).

Contraintes vérifiées en 003 : LEDs **tout-ou-rien** sur les pads (du mode affiché), Sync, Cue, Play et Vinyl
— exigence de l'utilisateur : elles reflètent l'état musical (pattern actif, mute, en attente…) ; Vinyl est un bouton unique global, bascule Filter/Bass globale,
continus en 14 bits (filtres et volumes sans paliers).

### Deck A — Rythme

| Contrôle | Fonction |
|---|---|
| Pads **Hot Cue** | patterns de batterie A/B/C/D (quantifiés) · Shift : variations |
| Pads **Sampler** | fills 1–4 (roll snare, roll perc, break kick, montée hats), de la noire suivante à la fin de mesure · Shift : sons seuls kick, snare, hats |
| Pads **FX** | mutes immédiats : kick / snare+clap / hats / perc (basse en 013) · Shift : solo |
| Pads **Loop** | beat-repeat 1/4, 1/8, 1/16, 1/32 (maintenu) : roule le dernier coup dès la frontière suivante ; pad plus court = subdivise ce qu'on entend, relâcher = retour en phase au pad tenu · Shift : reverse/gate |
| Jog bord | swing (−/+) |
| Jog dessus + rotation | tempo (en attendant les patterns de basse, 013) |
| Jog Shift + rotation | densité (élagage ordonné des pas par probabilité) |
| Tempo | densité de la batterie (0 = kick seul → 1 = complet) |
| Filter | filtre DJ du bus rythme |
| Bass | coupure du filtre de la basse |
| Volume | niveau rythme |
| Play | transport start/stop (global) · Shift : relancer à la mesure 1 |
| Cue | rappel de la scène courante · Shift : enregistrer la scène |
| Sync | tap tempo · Shift + jog : BPM fin |
| PFL | métronome on/off |

### Deck B — Mélodie

| Contrôle | Fonction |
|---|---|
| Pads **Hot Cue** | accords degrés I, IV, V, vi · Shift : ii, iii, VII°, ♭VII (selon gamme) |
| Pads **Sampler** | phrases mélodiques 1–4 · Shift : variations |
| Pads **FX** | mode d'arpège (off, up, updown, random) · Shift : octaves 1–4 |
| Pads **Loop** | maintien/latch, renversement −/+, octave −/+ |
| Jog bord | vitesse d'arpège (1/4 → 1/32) |
| Jog dessus + rotation | transposition **dans la gamme** (degré par degré) |
| Jog Shift + rotation | choix de la gamme |
| Tempo | brillance (coupure du synthé) |
| Filter | filtre DJ du bus mélodie |
| Bass | envoi delay/réverb |
| Volume | niveau mélodie |
| Play | deck mélodie on/off (quantifié) |
| Cue | tonique −/+ (Shift = +) |
| Sync | progression suivante |
| PFL | sortie MIDI vers un synthé externe (FM-1…) on/off — phase 7 |

### Global

| Contrôle | Fonction |
|---|---|
| Crossfader | **morph de scène** : interpole les paramètres continus entre la scène A et la scène B (mode 2 : balance rythme/mélodie) |
| Master | volume général (après limiteur) |
| Headphone | réverbération globale |
| Vinyl | bascule « jeu libre » (accords non quantifiés) |

## Fichier de mapping

`config/mapping.toml` (implémenté en 008, `crates/io/src/mapping.rs`) relie un **contrôle nommé** à une
action, sans recompiler :

```toml
[[bind]]
control = "a.tempo"          # a./b. + play, cue, sync, pfl, volume, filter, bass, tempo,
action = "tempo"             #   jog.edge, jog.top, pad.<hotcue|sampler|fx|loop>.<1-4> ;
range = [60.0, 200.0]        # mixer.<crossfader|master|headphone> ; vinyl
pickup = true                # soft-takeover

[[bind]]
control = "a.play"
shift = true                 # même touche avec Shift
action = "transport_restart"
```

Actions disponibles aujourd'hui : `transport_toggle`, `transport_restart`, `tap_tempo`,
`metronome_toggle`, `drum` (`voice` = kick, snare, closed_hat, open_hat, clap, perc), `tempo`,
`tempo_nudge` (`step`), `master_gain`, `metronome_level`, `drum_level`. Les actions
musicales (patterns, mutes, accords…) s'ajoutent avec les decks (010+), sur le même modèle.

## Théorie utile (rappels)

- Gammes en demi-tons : majeur `0 2 4 5 7 9 11`, mineur naturel `0 2 3 5 7 8 10`, dorien `0 2 3 5 7 9 10`,
  phrygien `0 1 3 5 7 8 10`, mixolydien `0 2 4 5 7 9 10`, penta mineure `0 3 5 7 10`.
- Accord sur le degré d : empiler les degrés d, d+2, d+4 (+6 pour la septième) dans la gamme → la qualité
  (maj/min/dim) en découle automatiquement.
- Rythmes euclidiens E(k, n) (Bjorklund/Toussaint) : générateur de patterns de perc. (E(3,8) tresillo,
  E(5,8) cinquillo).
- Densité ordonnée : chaque pas a un rang de priorité ; densité d → on joue les ⌈d·N⌉ pas les plus
  prioritaires (variation continue et musicale).

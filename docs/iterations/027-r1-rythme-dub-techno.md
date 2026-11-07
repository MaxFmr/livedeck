# 027 — R1 · Rythme dub techno : 3 kits, kick à la noire, deck A selon la carte D8

- Date de début : 2026-11-07
- Phase : refonte groovebox (docs/design/refonte-groovebox.md), étape R1
- Statut : terminée — essai au contrôleur groupé avec R2 (`tools/try-028.sh`)

## Objectif
Première étape jouable de la refonte : la section rythmique d'une groovebox dub techno, et le deck A
réorganisé selon la carte validée avec la maquette D8.

## Ce qui est fait
- **3 kits à la place des 6 styles** (`style.rs`, `STYLES = 3`) ; un kit amène toujours sons de
  batterie, 4 patterns, fills, lignes de basse, swing, patch du synthé et effets :
  - **808 · boom** (124 BPM conseillé) : kick grave et long, clap doux, hats feutrés.
  - **909 · techno** (128) : kick sec, open hats au contretemps, basse plus acide (résonance).
  - **Dub** (118) : kick étouffé, rimshot (caisse claire aiguë et courte), hats feutrés, beaucoup
    d'espace, réverb 5 s.
  - Tous les patterns gardent le **kick sur chaque temps** (test). Lignes de basse communes au
    contretemps ; fills communs ; patch « stab dub » court pour les accords ; delay 1/8 pointé.
- L'app **démarre sur le kit 808** (le moteur charge le kit 0 à sa création ; l'ancien son par
  défaut ne sert plus qu'aux tests du séquenceur).
- **Deck A selon la carte D8** :

  | Contrôle | Avant | R1 |
  |---|---|---|
  | Shift + pads Sampler A 1–3 | sons seuls kick / snare / hats | **kits** 808 · 909 · Dub, à la mesure |
  | Jog A tranche | swing | **densité** (≈ 50 % par tour) |
  | Jog A dessus | tempo | **swing** |
  | Shift + jog A | style | **tempo** (tranche ; dessus = fin) |
  | Filter A | — | **coupure de la basse** |
  | Bass A | coupure de la basse | libre (résonance en R2) |
  | Fader Tempo A | densité | libre (decay de l'enveloppe en R2) |

## Choix par défaut (à revoir)
- Les kits gardent le mécanisme des styles (changement à la mesure, le tempo reste le tien).
- Le « rim » du kit Dub est la caisse claire accordée haut et courte (pas de nouvelle voix).
- Shift + PFL A (batterie samples 909) reste branché, hors carte.
- Les phrases restent celles d'avant (communes aux 3 kits) jusqu'à la mélodie générée (R3).

## Vérification
- Tests : kick sur chaque temps dans tous les patterns, lignes d'une mesure ; mapping du deck A
  (kits, densité au jog, swing, tempo, coupure sur Filter A). 165 tests, clippy propre.
- Rendus d'écoute : `target/renders/styles.wav` (chaque kit, patterns 1–4), `fx.wav` (kit Dub).

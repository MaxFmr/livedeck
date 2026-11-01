# D2 — Architecture d'information

- Date : 2026-11-01
- Statut : validé par l'utilisateur (2026-11-01)
- Entrées : [D1](D1-intention.md), [design-process](design-process.md), modèle et mapping v0 de
  [live-engine](../music/live-engine.md)

## Hiérarchie

| Niveau | Règle | Rendu |
|---|---|---|
| **P1 — primaire** | lu en < 1 s entre deux gestes, toujours visible | grand, couleur du deck, contraste maximal |
| **P2 — secondaire** | visible en permanence, consulté quand on a le temps | petit, `--text-dim` ou couleur atténuée |
| **P3 — à la demande** | n'apparaît que quand on touche le contrôle (HUD ~1 s) ou en cas de problème | superposé dans la zone du deck concerné, puis disparaît |

**Source** : `snap` = déjà dans `EngineSnapshot` ; `ctl` = thread de contrôle (mapping, contrôleur) ;
`010+` = n'existe pas encore (arrive avec l'itération indiquée).

## Zones (miroir du contrôleur)

```
┌────────────────────────── barre du temps (globale) ──────────────────────────┐
│ ▶ 120.0 BPM   ■□□□ mesure 3 · phrase 2/8   La mineur   swing 58 %     ● ok   │
├──── DECK A · rythme (chaud) ───┬─ BASSE ─┬──── DECK B · mélodie (froid) ────┤
│ pattern actif / en attente      │ pattern │ accord actif / en attente        │
│ grille : voix × 16 pas          │ 16 pas  │ progression · arp · phrase       │
│ mutes · densité                 │ en      │ octave · renversement            │
│ 4 pads du mode actif (grands)   │ degrés  │ 4 pads du mode actif (grands)    │
│ + 3 autres modes (aperçu)       │ filtre  │ + 3 autres modes (aperçu)        │
│ [HUD du contrôle touché]        │         │ [HUD du contrôle touché]         │
├──────────────── centre bas : crossfader (morph de scène) · niveaux ──────────┤
└──────────────────────────────────────────────────────────────────────────────┘
```

## Inventaire

### Barre du temps (globale)

| Élément | Niveau | Source | Écho matériel |
|---|---|---|---|
| Lecture / arrêt | P1 | snap `playing` | LED Play (fixe / clignote) |
| Position dans la mesure (4 temps) | P1 | snap `beats` | flash Sync / Cue |
| BPM | P1 | snap `bpm` | — |
| Mesure dans la phrase (1–4 ou 1–8) | P2 | snap `beats` | — |
| Tonalité + gamme | P1 | 017 | — |
| Swing | P2 | 011 | — |
| Style (preset) | P2 | 014 | — |
| Scène courante / en attente | P2 | phase 6 | — |
| Santé : contrôleur, charge CPU, xruns | P3 sauf alerte (alors P1, couleur d'alerte) | ctl + `AudioStats` | — |

### Deck A — rythme

| Élément | Niveau | Source | Écho matériel |
|---|---|---|---|
| Pattern actif (A–D) | P1 | 011 | LED du pad Hot Cue |
| Pattern en attente + temps restant avant la frontière | P1 | 011 | pad en attente qui clignote |
| Grille de pas : **une ligne par voix** (kick, snare, hats, perc…) × 16 pas, curseur qui avance | P2 | 011 | — |
| Mutes (kick, snare, hats, basse) | P1 | 012 | LEDs des pads FX |
| Densité | P2 | 012 | — |
| Fill en attente | P1 (quand armé) | 012 | pad Sampler clignote |
| Beat-repeat actif (1/4…1/32) | P1 (quand maintenu) | 012 | pad Loop allumé |
| Niveau rythme | P2 | 012 | — |

### Basse — zone à part, entre les deux decks

Pont entre rythme et mélodie ; pilotée depuis le deck A (jog dessus, potard Bass), donc teinte
**chaude atténuée** (ambre) pour garder le lien avec les commandes de gauche.

| Élément | Niveau | Source | Écho matériel |
|---|---|---|---|
| Pattern de basse actif / en attente | P1 | 013 | — |
| 16 pas en degrés de gamme (hauteur relative), glides, accents | P2 | 013 | — |
| Coupure du filtre de la basse | P2 | 013 | — |
| Mute de la basse | P1 | 012–013 | LED du pad FX 4 (deck A) |

### Deck B — mélodie

| Élément | Niveau | Source | Écho matériel |
|---|---|---|---|
| Accord joué (degré + nom, ex. « vi · Fa♯m ») | P1 | 017 | LED du pad Hot Cue |
| Accord / phrase en attente + compte | P1 | 017–018 | pad clignote |
| Progression (suite de degrés, position) | P2 | 017 | — |
| Mode et vitesse d'arpège | P2 | 018 | LEDs des pads FX |
| Phrase active (1–4) | P2 | 018 | LED du pad Sampler |
| Octave, renversement, latch | P2 | 017–018 | LEDs des pads Loop |
| Jeu libre (non quantifié) | P1 (quand actif) | 017 | LED Vinyl |
| Niveau mélodie, envois delay/réverb | P2 | 019 | — |

### Pads (les deux decks)

| Élément | Niveau | Source |
|---|---|---|
| Mode affiché par le contrôleur (Hot Cue, Sampler, FX, Loop) | P1 | ctl (déjà suivi par `LedBank`) |
| Les 4 pads de ce mode, **en grand** : nom de la fonction + état (éteint, actif, en attente, tenu) | P1 | ctl + 011+ |
| Les 3 autres modes **en aperçu** (petits pads, états seulement : mutes, pattern en attente…) | P2 | 011+ |
| Couche Shift (libellés alternatifs pendant que Shift est tenu) | P2 | ctl |

### Centre bas (global)

| Élément | Niveau | Source |
|---|---|---|
| Crossfader : scène A ↔ scène B et position du morph | P2 | phase 6 |
| Niveaux de sortie (rythme, mélodie, master) + limiteur actif | P2 | 012+ (crête par bloc à publier) |

### HUD du geste (P3)

| Élément | Où | Source |
|---|---|---|
| Nom du paramètre + valeur (ex. « Swing 58 % », « Filtre A −40 % ») ~1 s | zone du deck touché (centre pour master, crossfader, casque) | ctl |
| Pickup en attente : valeur physique vs valeur à rejoindre + flèche du sens | idem | ctl `pending_pickup` |

## Ce qui n'est **pas** affiché pendant le jeu
Réglages audio, chemin du mapping, latences détaillées, journal : uniquement au lancement ou dans un écran
d'outils hors jeu (phase 6).

## Décisions de l'utilisateur
1. Pads : **mode actif en grand + aperçu des 3 autres modes** (on voit un mute ou un pattern en attente
   même quand son mode n'est pas affiché sur le contrôleur).
2. Grille de pas : **toutes les voix** (une ligne par voix × 16 pas).
3. Basse : **zone à part** entre les deux decks (teinte ambre, liée au deck A).

Conséquence pour D3 : trois colonnes (deck A · basse · deck B) sous la barre du temps ; la colonne basse
est étroite, les decks gardent la symétrie gauche/droite.

## Suite : D3
Trois maquettes (rangée de pas, afficheur, formes colorées) ; l'utilisateur a retenu **la rangée de pas**
(cases pleines qui s'allument, pads en grands carrés, aperçu des autres modes en mini-grilles).

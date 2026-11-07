# Plan d'essai intégral — groovebox dub techno (R1 → R5)

- Date : 2026-11-07
- Script guidé : `tools/try-groovebox.sh` (voix pour chaque étape, silence pendant le jeu libre ;
  Ctrl-C arrête tout, app comprise). Journal : `target/groovebox-test.txt`.
- Avant : contrôleur branché en USB, casque ou enceintes, `df -h /` correct.

## Ce que les tests automatiques couvrent déjà (sans matériel)

| Couche | Tests | Ce qui est vérifié |
|---|---|---|
| Moteur | 113 | kits (kick sur chaque temps), basse (réglages, ligne au hasard), phrase (générer, muter, figer, effacer, enregistrer par-dessus, évolution), throws, temps réel (`no_alloc`) |
| Mapping | 41 | chaque contrôle de la carte D8, Shift, pickup, jog sans dépendre du toucher |
| Bout en bout | 6 scénarios | gestes du contrôleur → mapping → app → moteur rendu hors ligne : kit à la mesure, densité au jog, swing, phrase générée qui sonne, REC d'accords, throws, son de la basse, aucun NaN |
| Fenêtre | captures | rendu de l'écran central rejoué par `midi` + `shot` |

L'essai au contrôleur vérifie ce que la machine ne peut pas juger : **le son, le ressenti, la
lisibilité à 1 m, la latence perçue**.

## Étapes

| # | Partie | Geste | Attendu | Noter |
|---|---|---|---|---|
| 1 | Rythme | Pads Hot Cue A 1–4 | pattern à la mesure, kick sur chaque temps, l'écran montre le pattern en attente puis la piste Rythme change | groove, intérêt des 4 patterns |
| 2 | Rythme | Jog A (n'importe comment) | densité qui monte / descend, affichée à l'écran | sensibilité |
| 3 | Rythme | Shift + Sampler A 1–3 | kits 808, 909, Dub à la mesure | caractère de chaque kit |
| 4 | Rythme | Pads FX A ; Shift + Loop A | mutes (pad allumé = joue) ; swing 50/54/58/62 % | |
| 5 | Basse | Potard Filter/Bass A (bouton Filter/Bass pour passer de Filter à Bass), fader Tempo A (et Shift) | coupure sur Filter, résonance sur Bass (Shift : enveloppe), decays ; l'anneau inactif est estompé | plages, pickup entre couches |
| 6 | Basse | Shift + Sampler A 4 | ligne au hasard à la mesure, visible dans la piste Basse | intérêt des lignes |
| 7 | Phrase | Sampler B 1 « Générer » | phrase à la mesure, notes dans la piste Phrase ; elle varie toute seule tous les 4 mesures | |
| 8 | Phrase | Fader Tempo B puis Générer ; potard Filter/Bass B sur Filter | plus ou moins de notes ; brillance | |
| 9 | Phrase | Sampler B 2, 3, 4 | muter (à la mesure), figer (ne varie plus), effacer | |
| 10 | Phrase | Loop B 1 tenu + accords Hot Cue B | accords ajoutés à la phrase (blocs larges dans la piste) | calage |
| 11 | Effets | FX B 1 et 2 tenus | throw delay, throw réverb ; voyants à l'écran | caractère dub |
| 12 | Effets | FX B 3, FX B 4 ; Bass B, Headphone | temps du delay, arpège ; envois | |
| 13 | Mixer | Crossfader | fondu rythme ↔ mélodie | |
| 14 | Interface | Regarder à 1 m, écran à 30 % | onglet du mode = mode du contrôleur, pads = LEDs, Shift affiche la seconde couche | lisibilité |
| 15 | Jeu libre | 10 min | aucune coupure, charge et xruns au journal | ce qui gêne |

## Critères de réussite
- 0 xrun, 0 NaN au journal ; charge max < 15 %.
- Chaque étape fait ce qui est attendu ; les écarts sont notés pour la suite.

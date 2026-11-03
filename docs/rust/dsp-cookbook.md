# Livre de recettes DSP (f32, sans allocation)

Tout s'écrit en `f32`, par échantillon ou par bloc, avec un état pré-alloué. Toute fréquence est passée en
paramètre, aucune constante `48000` en dur.

## Oscillateurs

- **Phase accumulator** : `phase += f / sr; if phase >= 1.0 { phase -= 1.0 }`. Phase en `f32` OK si
  remise dans [0,1) à chaque tour (pas de dérive).
- **Sinus** : `sin(2π·phase)` ; pour du volume de voix, approximation polynomiale ou table de 2048 points
  + interpolation linéaire (table `static`, pas allouée).
- **Dents de scie / carré anti-repliement : PolyBLEP** (Välimäki & Huovilainen). Correction résiduelle
  autour de la discontinuité :
  ```text
  t = phase, dt = f/sr
  if t < dt      { x = t/dt;        blep = x+x - x*x - 1 }
  elif t > 1-dt  { x = (t-1)/dt;    blep = x*x + x+x + 1 }
  else blep = 0
  saw = 2t - 1 - blep
  ```
  Carré = différence de deux dents de scie déphasées (largeur d'impulsion variable gratuite).
- **Bruit** : xorshift32 (`x ^= x<<13; x ^= x>>17; x ^= x<<5`) → `f32` dans [-1,1]. Pas de crate `rand`
  dans le moteur.

## Filtres

- **SVF TPT (Andrew Simper / Cytomic, « trapezoidal SVF »)** : filtre de référence. Stable en modulation
  rapide, sorties LP/BP/HP simultanées, coût faible.
  ```text
  g = tan(π·fc/sr); k = 1/Q
  a1 = 1/(1 + g(g+k)); a2 = g·a1; a3 = g·a2
  v3 = x - ic2eq; v1 = a1·ic1eq + a2·v3; v2 = ic2eq + a2·ic1eq + a3·v3
  ic1eq = 2v1 - ic1eq; ic2eq = 2v2 - ic2eq
  LP = v2, BP = v1, HP = x - k·v1 - v2
  ```
  `tan` est coûteux : recalculer `g` seulement quand fc change (ou approximation rationnelle).
- **Filtre DJ bipolaire** (potard Filter, centre = neutre) : centre ± zone morte de 3 % → bypass ;
  gauche → LP de 20 kHz vers ~80 Hz (courbe exponentielle) ; droite → HP de 20 Hz vers ~8 kHz.
  Résonance légère fixe (Q ≈ 0,9). Fondu croisé dry/wet sur la zone morte pour éviter un saut.
- **EQ grave** (potard Bass) : low-shelf biquad (RBJ Audio EQ Cookbook) ou kill par LP/HP complémentaire.

## Enveloppes

- **ADSR exponentiel** : chaque segment est un one-pole vers une cible dépassée
  (`env += (target - env)·coef`, `coef = 1 - exp(-1/(t·sr))`), précalculé quand t change.
- **AD** pour la batterie (attaque ~0, decay variable).
- Fin de voix quand `env < 1e-4` → voix libérée (et pas de dénormaux).

## Lissage des paramètres (anti « zipper »)

Toute valeur venant d'un potard MIDI 7 bits (128 pas) passe par un **one-pole** de 5–20 ms côté audio :
`y += (cible - y)·coef`. Le fader Tempo 14 bits aussi. Les booléens (mute) → mini-fondu de 2–5 ms.

## Batterie synthétique (pas besoin de samples pour commencer)

| Voix | Recette |
|---|---|
| Kick | sinus, hauteur 150→45 Hz par enveloppe exp (~40 ms), amplitude AD (decay 200–600 ms), clic = 2 ms de bruit filtré, saturation douce |
| Snare | 2 sinus ~180/330 Hz decay court + bruit HP 1,5 kHz decay 150 ms |
| Hat fermé / ouvert | bruit → HP 7 kHz (ou banque de 6 carrés « métalliques » 808), decay 40 / 300 ms, l'ouvert est coupé par le fermé (*choke group*) |
| Clap | bruit BP 1 kHz, 3 micro-attaques espacées de 10 ms + queue |
| Perc / tom | sinus + pitch env plus long |

## Basse

Mono, dents de scie + carré sub (-1 octave), SVF LP + enveloppe de filtre, **glide** (portamento one-pole
sur la fréquence) quand les notes se chevauchent, accent par pas.

## Poly synthé (deck mélodique)

- Pool fixe de **16 voix** ; vol de voix : la plus ancienne en release, sinon la plus ancienne.
- Par voix : 2 oscillateurs (saw/carré, désaccord ±7 cents), SVF LP, ADSR ampli + ADSR filtre.
- Accord = N voix déclenchées au même échantillon ; légère dispersion (strum) optionnelle 0–30 ms.

## Effets (lignes pré-allouées)

- **Delay** synchronisé au tempo : ring buffer de 2 s alloué à l'init, lecture interpolée (linéaire),
  feedback filtré (LP dans la boucle), changement de temps → fondu entre deux têtes pour éviter le glissement.
- **Réverbération** : FDN 8 lignes ou Freeverb (Schroeder/Moorer) ; tailles max pré-allouées.
  Implémenté en 019 (`engine::fx`) : delay 2 s, deux têtes en fondu de 40 ms, boucle LP 3,5 kHz + HP 150 Hz
  + `soft_clip` (95 % max) ; réverb FDN 8 lignes 29,7–73,1 ms, Hadamard normalisée, 2 allpass de diffusion,
  amortissement 5,5 kHz, gains `10^(-3·L/(RT60·sr))`, sorties G/D par signes distincts.
- **Beat-repeat / stutter** : capture d'1/4…1/32 dans un buffer pré-alloué et relecture quantifiée.

## Sortie

- Bus : batterie, basse, mélodie → filtre deck → volume deck → crossfader/balance → master.
- **Limiteur de sécurité obligatoire** sur le master (look-ahead 1–2 ms, plafond −0,3 dBFS) +
  `tanh` approché en soft-clip. Aucun réglage ne doit pouvoir produire > 0 dBFS ni NaN.
- Garde NaN : si un échantillon n'est pas fini → remise à zéro de l'état de la voix fautive + compteur.

## Utilitaires

- `db_to_gain(db) = 10^(db/20)`, `midi_to_hz(n) = 440·2^((n-69)/12)` (table de 128 précalculée).
- Approximations rapides : `tanh` (Padé 3/3), `exp2`, `tan` — vérifier l'erreur par test contre `std`.

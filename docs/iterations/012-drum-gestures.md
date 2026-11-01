# 012 — Gestes du deck rythme : mutes, fills, beat-repeat, densité

- Date de début : 2026-11-01
- Phase : 3
- Statut : terminée (2026-11-01)

## Objectif
Jouer le groove au contrôleur sans changer de pattern : couper des voix, lancer des fills, boucler la
batterie au beat-repeat et l'éclaircir ou la remplir au fader.

## Choix de l'utilisateur (2026-11-01)
- Fader Tempo A = **densité** ; le tempo passe au jog A par le dessus et au tap tempo (Sync A).
- Pads Sampler A = **fills**, Shift + pads Sampler = les sons seuls de 010.
- Mutes **immédiats**, LED allumée = le groupe joue.
- Beat-repeat sur **le son du bus batterie** (capture puis boucle), le séquenceur continue dessous.

## Périmètre
- Dedans :
  - séquenceur : mutes par voix (masque), densité (élagage ordonné par `importance` : kicks sur les
    temps = 1, puis voix × place dans la mesure × vélocité), 4 fills joués de la prochaine noire à la
    fin de la mesure (non élagués par la densité), notation de vélocité `1`–`9` ;
  - `repeat::BeatRepeat` (forme finale après les essais, voir Résultat) : roule le dernier coup
    joué, historique du bus, 4 boucles mémorisées, subdivision, fondus ; bus batterie séparé dans le
    moteur, niveau batterie appliqué après ;
  - commandes `SetMutes`, `SetDensity`, `Fill`, `BeatRepeat` ; instantané : mutes, densité, fill
    (en attente / joue), beat-repeat ;
  - mapping : `mute` (`voices = [...]`), `fill` (`fill = 1–4`), `beat_repeat` (`division`, pad tenu),
    `drum_density` (fader, sans pickup : la position physique fait foi) ;
  - app : LEDs des mutes (allumée = joue) et des fills (clignote vite en attente, fixe pendant le
    fill), pile des pads Loop tenus (relâcher le dernier revient au précédent), messages ;
  - WAV `target/renders/gestures.wav` (densité, fills, beat-repeat sur le groove B).
- Dehors : solo (Shift + FX), reverse/gate (Shift + Loop), banques de fills, basse (013).

## Critère de fin (mesurable)
- Tests : mutes par voix, densité 0 = kicks sur les temps, élagage monotone, fill de la noire suivante
  à la fin de mesure puis retour au pattern, fill ignoré à l'arrêt, capture puis boucle, rampes,
  boucle du bus batterie identique d'un tour à l'autre, mapping des pads FX/Loop/Sampler et du
  fader, test sans allocation avec les nouvelles commandes.
- Bench : 6 voix + beat-repeat + fill < 2 % du budget d'un bloc de 128.
- Essai : densité audible et progressive, mutes et LEDs, fills calés sur la noire, beat-repeat sans
  clic gênant et reprise en place, 0 xrun.

## Plan
1. Séquenceur (mutes, densité, fills) + tests. 2. Beat-repeat + bus batterie. 3. Commandes et
instantané. 4. Mapping. 5. App et LEDs. 6. Bench, WAV. 7. Essai guidé (`tools/try-012.sh`).

## Résultat
- Fait : tout le périmètre ; 90 tests verts, clippy debug/release propre.
- Mesures :
  - bench (sur batterie) : métronome 8,3 µs, 6 voix 14,1 µs, 6 voix + beat-repeat 1/32 + fill
    15,4 µs par bloc de 128, soit 0,6 % du budget de 2 667 µs ; charge max à l'essai 4,4 % ;
  - latence geste → son (Shift + Sampler) : médiane 7,9 ms (6,9–9,0) ; 0 xrun sur tous les essais ;
  - WAV des gestes 24 s, crête −0,3 dBFS.
- Essai utilisateur (2026-11-01, haut-parleurs du Mac) : densité « bon ordre », mutes et fills
  (son et LEDs) validés au premier essai, sons seuls Shift + Sampler validés. Le beat-repeat a
  demandé 8 tours d'essai :
  1. clics → fondu d'entrée de chaque tour en plus du fondu de sortie ;
  2. départ une division trop tard (capture puis boucle) → **historique du bus** : la tranche
     commence avant l'appui, la répétition part à la frontière suivante ;
  3. « le son coupe, des morceaux de notes » (tranches de grille vides ou coupées par le swing) →
     **rouler le dernier coup joué** (choix de l'utilisateur) ;
  4. clic du kick bouclé → fondus plus longs (10 ms aux bascules, 20 ms en cosinus en fin de tour,
     4 ms en début) ; mesure hors ligne : aucun raccord au-dessus de l'attaque normale du kick ;
  5. clic restant à l'appui → trouvé par l'utilisateur : une snare live tombant sur la frontière
     sonnait 10 ms avant la boucle → **le live s'éteint dans les 10 ms qui précèdent la frontière** ;
  6. pad 1 + pad 3, relâcher le 3 ne revenait pas au 1 → **4 boucles mémorisées** qui tournent en
     phase, retour immédiat ;
  7. de 1/16 vers 1/4, la boucle répétait le bégaiement, puis un nouvel appui rejouait une vieille
     boucle → tranches prises dans le **groove sec** (comme le mode slip des platines), appui =
     tranche neuve, relâche = retour (`BeatRepeatBack`) ; à faible densité, un coup trop ancien n'est
     plus roulé : on **attend le prochain coup** ;
  8. pad 1 sur une snare + pad 3 roulait le kick du dessous → une division plus courte
     **subdivise ce qui est entendu**, depuis le début du tour.
- Outil ajouté : `app --record <wav>` enregistre la sortie réelle (voie droite = beat-repeat
  engagé), sans allocation dans le callback ; `tools/record-repeat.sh`.
- Écarts : le fader de densité n'a pas de pickup (sa position s'applique dès la connexion, via le
  dump d'état du contrôleur).

## Appris / à reporter dans la doc
- Un beat-repeat qui boucle le bus doit séparer la batterie du clic et appliquer le niveau après
  la boucle, sinon le fader de volume serait figé pendant la répétition.
- Un beat-repeat jouable n'est pas une tranche de grille : il roule une note (le dernier coup, ou
  le prochain), commence à la frontière suivante sans laisser passer le live qui tombe dessus,
  subdivise ce qui est entendu et revient en phase au pad encore tenu.
- Un rendu hors ligne propre ne prouve pas l'absence de clic en jeu : le geste réel (moment de
  l'appui, coups qui tombent sur la frontière) compte ; d'où `--record`.

## Questions ouvertes
- Le jog A par le dessus garde le tempo tant que la basse (013) n'en a pas besoin.

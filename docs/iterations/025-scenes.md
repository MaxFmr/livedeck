# 025 — Scènes : 8 emplacements, rappel à la mesure, fichier de session

- Date de début : 2026-11-05
- Phase : 6
- Statut : terminée — essai au contrôleur à faire (`tools/try-025.sh`)

## Objectif
Figer le jeu en cours dans une scène et le rappeler d'un geste, pile au premier temps, sans perdre les
scènes en quittant l'app. Contenu validé par l'utilisateur le 2026-11-05.

## Périmètre
- Dedans : contenu d'une scène (style, pattern, swing, mutes, densité, source batterie, ligne de basse,
  mute basse, tonique, gamme, filtre basse, latch et accords latchés, voicing, arpège, phrase,
  transposition, brillance, envois et répétitions du delay, envoi réverb) ; 8 emplacements ; rappel à la
  mesure ; réenregistrement armé par Sync B ; fichier `config/session.toml` (`--session`), relu au
  démarrage, réécrit à chaque enregistrement (fichier temporaire puis renommage) ; bande « Scènes » dans
  le pied de la fenêtre.
- Dehors : tempo, volumes, master et crossfader (restent en main) ; temps du delay (suit le style) ;
  morph entre scènes (remplacé par le fondu entre decks, 024).

## Moteur
- `Command::AtBar(true/false)` encadre un groupe : les commandes qui agissent tout de suite (mutes,
  voicing, envois…) sont gardées dans un tableau fixe de 48 et appliquées au bloc qui contient la
  prochaine mesure ; celles qui attendent déjà la mesure (pattern, style, ligne, gamme, tonique, phrase)
  passent directement. Ainsi tout change au même premier temps (au plus un bloc, 2,7 ms, avant).
- `QueueSwing` : le swing de la scène s'applique après le style (qui impose le sien à la mesure).
- `QueuePhrase` (phrase voulue, pas une bascule) et `SetChords` (accords latchés d'un coup).

## Choix par défaut (à revoir)
- **Emplacements** : Shift + pads FX A = scènes 1–4, Shift + pads Loop A = scènes 5–8 (les seules
  couches de pads libres ; Shift + Sampler était déjà pris).
- **Enregistrer / rappeler** : case vide → enregistre ; case pleine → rappelle. Pour remplacer : Sync B
  arme l'enregistrement (LED qui clignote vite, pied « Enregistrer » en ambre), puis la case.
- **Retour visuel** dans la fenêtre seulement : les LEDs des pads montrent la couche sans Shift.
- **Accords** gardés seulement en latch (sinon ils dépendent des doigts).
- **Potards avec reprise en main** (envois delay et réverb) : après un rappel ils doivent rejoindre la
  valeur de la scène avant d'agir ; densité, filtre basse et brillance n'ont pas de reprise en main et
  sautent au premier mouvement.
- Le fichier de session est ignoré par git (données de jeu).

## Vérification
- Tests : groupe `AtBar` retenu jusqu'à la mesure (moteur), accords latchés d'un coup, aller-retour du
  fichier de session et emplacements invalides refusés, mapping des scènes. 165 tests, clippy propre.
- Revue temps réel : le tableau des commandes gardées est fixe (`[Option<Command>; 48]`), parcours
  bornés, aucune allocation ; `no_alloc` vert.
- Rejoué sans contrôleur (`midi` + `shot`) : scène 1 enregistrée, pattern B et hats coupés, scène 2,
  rappel de la scène 1 → pattern A et hats rétablis au premier temps ; fichier écrit et relu au
  démarrage (« 2 scène(s) »).
- Écart corrigé : la source batterie était renvoyée à chaque rappel (« batterie synthé à la mesure
  suivante ») ; elle n'est plus envoyée que si elle change. Le texte d'aide du pied débordait sur les
  vumètres : remplacé par le libellé « Enregistrer » et des contours ambre.

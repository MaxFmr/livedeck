# 029 — R3 + R4 + R5 · Phrase générative, throws dub, interface écran central, tests de bout en bout

- Date de début : 2026-11-07
- Phase : refonte groovebox, itération globale (demande de l'utilisateur : « dev global avec test
  automatisé, puis plan de test intégral »)
- Statut : terminée — essai intégral au contrôleur à faire (`tools/try-groovebox.sh`,
  `docs/tests/plan-essai-groovebox.md`)

## R3 · La phrase générative (deck B)
- Nouveau module `engine/src/phrase.rs` (tableaux fixes, xorshift, aucune allocation) : **une**
  phrase d'une mesure, jusqu'à 32 événements (notes en degrés ou accords).
  - **Générer** (Sampler B 1) : 3 à 12 notes selon la densité, marche aléatoire sur les degrés,
    contretemps favorisés, à la mesure ; la phrase devient libre.
  - **Muter** (Sampler B 2) : deux notes bougent (hauteur ou place), à la mesure.
  - **Figer** (Sampler B 3) : bascule ; libre, la phrase mute seule toutes les 4 mesures.
  - **Effacer** (Sampler B 4) : silence à la mesure.
  - **REC tenu** (Loop B 1) : les accords joués s'ajoutent à la phrase (calés à la double croche).
  - Densité au **fader Tempo B** ; la brillance passe sur **Filter B**.
  - La phrase suit tonalité, gamme, transposition (jog B) et octave des accords.
- **Retirés** : le looper 026 (4 boucles, REC sur PFL B, longueur), le mode notes (Sync B), la
  vitesse d'arpège au jog. Jog B = transposition quel que soit le toucher, Shift + jog B = gamme.

## R4 · Effets dub
- **Throw delay** (FX B 1 tenu) et **throw réverb** (FX B 2 tenu) : la mélodie et la batterie
  partent pleines dans l'effet, par-dessus les potards, lissés pour ne pas claquer.
- FX B 3 = temps du delay ; FX B 4 = arpège on / off (Shift + FX B : octaves).

## R5 · Interface « écran central » (maquette D8)
- `ui/groove.rs` remplace l'ancienne mise en page (rhythm.rs, melody.rs, barre du temps, pied).
- Côtés : 4 anneaux nommés par leur commande (couche Shift quand Shift est tenu ; « kit » tant
  qu'un réglage de basse n'a pas été touché), onglet du mode montré par le contrôleur, ses 4 pads
  tels que les LEDs, rappel du mode, rôle du jog.
- Écran : tempo, position, temps, tonalité, kit et swing ; pistes Rythme (kick, rim, hats, perc),
  Basse (notes à leur hauteur), Phrase (notes et accords) sous une même tête de lecture ;
  « À la mesure » (pattern, kit, ligne, basse au hasard, tonalité, phrase) ; crossfader, master,
  réverb, voyants de throw, vumètres, limiteur, charge ou alerte.
- Vérifié par captures rejouées (`midi` + `shot`) ; chevauchements corrigés (onglets, noms
  d'anneaux, rangée du mixer).

## Tests automatiques
- **Bout en bout** (`app/src/main.rs`, module `scenario`) : de vrais événements du contrôleur
  passent par le mapping, le traitement du fil de contrôle et le moteur, rendu hors ligne :
  kit à la mesure et densité au jog, swing, phrase générée qui sonne puis figée / mutée / effacée,
  accords enregistrés par REC, throws tenus et arpège, son de la basse et ligne au hasard, valeurs
  lues par la fenêtre, aucun NaN.
- Moteur : phrase (densité, attente de la mesure, évolution, REC). Mapping : carte D8 du deck B.
- **172 tests**, clippy propre, `no_alloc` vert.

## Choix par défaut (à revoir à l'essai)
- Phrase d'une mesure ; évolution toutes les 4 mesures ; vélocité fixe (accents rares).
- Throws : mélodie + batterie (0,6 au delay, 0,8 à la réverb).
- Shift + Loop B 2 = renversement (+1 seulement).
- Les phrases des kits (018) restent dans le moteur, plus jouables.

## Mesures (release, `tools/measure-cpu.sh`, 20 s par état)
| | CPU en lecture | CPU à l'arrêt | RSS |
|---|---|---|---|
| Fenêtre écran central (029) | 7,5 % | 1,5 % | 84 Mo |
| Fenêtre D4 (023, rappel) | 8,4 % | 1,9 % | 59 Mo |

RSS en hausse (+25 Mo, chemins des anneaux et des pistes), toujours sous l'objectif de 120 Mo.

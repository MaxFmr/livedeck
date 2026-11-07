# 028 — R2 · Basse versatile : résonance, enveloppe, decays, ligne au hasard

- Date de début : 2026-11-07
- Phase : refonte groovebox (docs/design/refonte-groovebox.md), étape R2
- Statut : terminée — essai au contrôleur groupé avec R1 (`tools/try-028.sh`)

## Objectif
Une basse simple à jouer mais réglable en son, selon la carte D8 du deck A.

## Ce qui est fait
- **Quatre réglages de son en direct** (`BassVoice`), qui prennent le pas sur le son du kit une fois
  touchés (le reste du son reste celui du kit) :

  | Contrôle | Réglage | Plage |
  |---|---|---|
  | Bass A | résonance | Q 0,5 → 8 (exponentiel) |
  | Shift + Bass A | quantité d'enveloppe du filtre | 0 → 5 octaves (accents : +1,5) |
  | Fader Tempo A | decay de l'enveloppe du filtre | 30 ms → 1,5 s |
  | Shift + fader Tempo A | decay de l'amplitude | 80 ms → 2 s ; en haut : note tenue |
  | Filter A (R1) | coupure | 60 Hz → 4 kHz |

  Chaque contrôle à deux couches attend de rejoindre sa valeur (pickup) quand on change de couche
  avec Shift.
- **Ligne de basse au hasard** : Shift + pad Sampler A 4 tire une ligne dub techno dans la gamme
  (racine sur les contretemps le plus souvent, quinte et septième en dessous, quarte, tierce,
  accents rares) qui remplace la ligne courante **à la mesure** ; elle reste jusqu'au choix d'une
  autre ligne ou d'un autre kit.
- Console et écho des gestes (HUD) pour chaque réglage ; les réglages touchés survivent à une
  reconstruction du flux audio.

## Choix par défaut (à revoir)
- Plages ci-dessus ; decay d'amplitude « tenu » au-dessus de 98 % du fader.
- La ligne au hasard remplace la ligne courante (pas d'emplacement à part).

## Vérification
- Tests : réglages qui remplacent le son du kit, decay d'amplitude qui éteint une note tenue, lignes
  au hasard (2 à 12 notes, attente de la mesure), mapping du deck A. 167 tests, clippy propre.
- Revue temps réel : calculs simples dans les réglages (appelés au fil des commandes), tirage au
  hasard par xorshift dans un tableau fixe, aucune allocation ; `no_alloc` vert.
- Rendu `target/renders/bass-sound.wav` (kit Dub, une mesure par réglage) : crête −6,8 dBFS.

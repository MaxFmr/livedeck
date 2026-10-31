# 007 — Contrôleur branché en direct

- Date de début : 2026-10-31
- Phase : 2
- Statut : terminée

## Objectif
L'app voit le DJControl Mix arriver et partir sans redémarrer, lit ses messages en continu et pilote ses
LEDs, avec un premier geste utile : Play lance/arrête le transport.

## Périmètre
- Dedans : `io::link` (connexion midir entrée + sortie, file `rtrb` du callback CoreMIDI vers le thread de
  contrôle, détection d'arrivée/départ par scan toutes les secondes, dump `B0 7F 7F` à la connexion,
  `B0 7F 00` à la fermeture), `io::leds` (état voulu de toutes les LEDs, envoi différentiel, renvoi complet à
  la reconnexion), `app` : Play A/B ↔ transport, LED Play = état du transport, LEDs Sync/Cue pulsées au temps.
- Dehors : mapping configurable (008), soft-takeover, jogs lissés (008), latence mesurée et clignotement
  « en attente » (009).

## Critère de fin (mesurable)
- Tests : banque de LEDs (diff, renvoi complet), file d'entrée pleine sans panique.
- Essai : débrancher / rebrancher le contrôleur pendant la lecture → l'app l'annonce, le son continue, les
  LEDs reviennent ; Play du contrôleur lance/arrête le métronome.

## Plan
1. `io::leds`. 2. `io::link`. 3. `app`. 4. Essai.

## Résultat
- Fait :
  - `io::leds::LedBank` : état voulu des 39 LEDs (6 transport, Vinyl, 2 × 4 modes × 4 pads), envoi
    différentiel, pads d'un mode caché envoyés quand il s'affiche (`mode_shown`), `invalidate` pour tout
    renvoyer. 4 tests.
  - `io::link::ControllerLink` : scan des ports toutes les secondes ; connexion entrée + sortie par nom ;
    callback CoreMIDI → file `rtrb` (2 048 messages, débordement compté) ; décodage sur le thread de
    contrôle ; mode affiché suivi via `PadModeSelect` et les pads ; dump `B0 7F 7F` à la connexion ;
    `B0 7F 00` à la fermeture ; événements `Connected` / `Disconnected` / `Input`.
  - `app` : Play (A ou B) ↔ transport ; LED Play = lecture ; Sync flashe au temps 1, Cue aux autres temps
    (120 ms) ; gestes discrets affichés ; bilan avec l'état du contrôleur ; boucle de contrôle à 2 ms.
  - 33 tests verts, clippy debug/release propre.
- Mesures : charge 0,2–0,4 %, 0 xrun, 0 message MIDI perdu pendant l'essai.
- Essai utilisateur : « tout marche » ✅ pour les LEDs, les gestes et la reconnexion ; **Play non testé**
  pendant cet essai (le bilan final montrait « commandes 2 ») → vérifié à part : essai de 35 s journalisé
  dans `target/play-test.txt`, Play A puis B ▶/■/▶, commandes 3 → 6 ✅.

## Appris / à reporter dans la doc
- Le panneau Terminal de l'app affiche la sortie avec retard : se fier au retour de l'utilisateur ou à un
  fichier journal pour les essais interactifs (à prévoir : option `--log`).
- Les gestes passent aujourd'hui par la boucle de contrôle (≤ 2 ms + 1 bloc). Pour les jogs et les
  potards immédiats, un chemin direct MIDI → audio sera évalué en 008–009 avec la mesure de latence.

## Questions ouvertes
- Latence geste → son à mesurer (009).

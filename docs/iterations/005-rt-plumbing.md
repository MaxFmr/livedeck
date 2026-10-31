# 005 — Plomberie temps réel

- Date de début : 2026-10-31
- Phase : 1
- Statut : terminée

## Objectif
Piloter le moteur pendant qu'il joue, sans jamais bloquer le thread audio, et voir son état en direct.

## Périmètre
- Dedans : `Command` (`Copy`, ≤ 32 o) par file SPSC `rtrb`, `EngineEvent` (audio → contrôle),
  `EngineSnapshot` en `triple_buffer`, thread de contrôle (ici le thread principal de `app`) avec
  commandes clavier, reconstruction du stream si le périphérique disparaît, App Nap / veille désactivés.
- Dehors : file `Garbage` (aucune ressource lourde avant les samples, 015 — ajoutée à ce moment-là),
  horodatage MIDI à l'échantillon (008), UI (phase 5).

## Critère de fin (mesurable)
- Tests : commandes appliquées au bloc suivant, snapshot à jour, file pleine sans panique, taille de
  `Command` vérifiée à la compilation.
- Essai : démarrer/arrêter et changer le tempo au clavier pendant la lecture, sans clic ni xrun ;
  débrancher/rebrancher la carte son sans plantage.

## Plan
1. API vérifiées dans les sources : rtrb 0.4.1 (`RingBuffer::new`, `push`, `pop`), triple_buffer 9.0.0
   (`triple_buffer(&init)`, `write`, `read`), objc2-foundation 0.3.2
   (`NSProcessInfo::processInfo().beginActivityWithOptions_reason`).
2. `engine::link` + tests. 3. Contrôle dans `app`. 4. App Nap. 5. Essai.

## Résultat
- Fait :
  - `engine::link` : `Command` (Start, Stop, SetTempo, Metronome ; taille ≤ 32 o vérifiée à la
    compilation), `EngineEvent::Beat`, `EngineSnapshot` ; `connect(engine)` → `AudioEngine` (thread audio)
    + `EngineHandle` (contrôle). Commandes appliquées en tête de bloc, au plus 256 par bloc ; événements
    perdus comptés si la file déborde.
  - `Clock::set_bpm` avec ancre : changement de tempo sans saut de phase (test dédié) ;
    `last_sample_beat` pour attribuer un battement au dernier échantillon joué.
  - `io::audio` : drapeau `invalidated` (DeviceNotAvailable / StreamInvalidated) ; `DeviceChanged` ignoré
    (le stream du périphérique par défaut suit tout seul).
  - `io::system::keep_alive` : `NSProcessInfo.beginActivity(UserInitiated | LatencyCritical)` pendant
    toute la session (pas d'App Nap, pas de veille).
  - `app` : thread principal = contrôle ; commandes clavier (p, +/−, t, m, q), affichage des mesures,
    bilan toutes les 5 s, reconstruction du stream avec rejeu de l'état voulu.
  - 16 tests moteur + 12 io = 28 verts.
- Mesures (essai utilisateur ≈ 4 min, UMC1820, 128 frames) : charge max 0,3–1,3 %, NaN 0, tempo de 60 à
  200 BPM sans clic, arrêt/relance propres. **2 xruns** vers 30 s de la première lecture, sans action de
  l'utilisateur → non expliqués.
- Écoute utilisateur : « tout est propre » ✅.

## Appris / à reporter dans la doc
- Une file SPSC pleine refuse aussi les commandes importantes : le contrôle doit vérifier le retour de
  `send` (fait dans les tests ; à traiter côté contrôle quand le débit MIDI arrivera, 008).
- La saisie clavier en terminal est pénible avec un affichage qui défile : commandes tolérantes
  (`+++`, majuscules). L'UI (phase 5) remplacera cette console.

## Questions ouvertes
- Origine des 2 xruns (à mesurer en 006 : endurance 10 min, horodatage des xruns).
- Débranchement de la UMC1820 pendant la lecture : **fonctionne d'après l'utilisateur** ✅, mais aucun message
  de reconstruction dans le journal → macOS a probablement re-routé le stream du périphérique par défaut
  (`DeviceChanged`). Le chemin « reconstruction » reste à observer (journaliser le type d'erreur reçu en 006).

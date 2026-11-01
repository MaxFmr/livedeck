# 009 — États des LEDs et latence geste → son

- Date de début : 2026-11-01
- Phase : 2
- Statut : terminée

## Objectif
Les LEDs montrent l'état (lecture, attente, attention) en rythme avec la musique, et la latence entre un
geste sur le contrôleur et le son est mesurée à chaque frappe, détaillée étape par étape.

## Périmètre
- Dedans :
  - `io::leds::LedState` : Off, On, Blink (attente, 1 fois par temps), BlinkFast (attention, 4 fois par
    temps), FlashBeat / FlashDownbeat / FlashOtherBeats, dessinés contre la position musicale (LEDs
    tout-ou-rien) ;
  - règles d'affichage dans l'app, déduites du mapping (`Mapping::bound` + `led_of`) : Play = lecture
    (fixe) / arrêt (clignote) ; Sync flashe au temps 1, Cue aux autres temps (en lecture) ; tap tempo
    clignote vite tant que le fader Tempo attend sa prise ; pad allumé tant qu'il est tenu ;
  - son de test (`test_hit`, 330 Hz, 150 ms : distinct du métronome) sur les pads Hot Cue des deux decks
    et la touche `h` ;
  - boucle de contrôle réveillée par le callback CoreMIDI (et le clavier) au lieu d'attendre 2 ms ;
  - horodatage : heure hôte du geste (CoreMIDI), du passage dans la boucle, de l'appel du callback, du
    tampon (`mHostTime`) et estimation cpal de la sortie.
- Dehors : sons de batterie (010), états musicaux propres aux decks (patterns en attente, mutes : 011–012),
  mesure acoustique au micro (à décider selon les chiffres).

## Critère de fin (mesurable)
- Tests : rendu des états de LED selon la position, envoi seulement aux changements, son de test au premier
  échantillon du bloc, horodatage remonté par le moteur, action `test_hit` liée aux pads.
- Essai : l'utilisateur voit les états des LEDs décrits ci-dessus ; latence médiane geste → sortie
  < 10 ms en USB sur au moins 10 frappes de pad.

## Plan
1. Moteur : `Command::Hit`, `EngineEvent::Hit`, `process_at(BlockTime)`. 2. `io::audio::BlockStamp`,
`io::system::host_time_us`. 3. `LedState`. 4. Mapping `test_hit`. 5. App. 6. Essai guidé
(`tools/try-009.sh`).

## Résultat
- Fait :
  - moteur : voix `hit` (clic accentué, joué même à l'arrêt) ; `Command::Hit { gesture_us }` →
    `EngineEvent::Hit { gesture_us, time }` ; `AudioEngine::process_at` reçoit les horodatages du bloc.
  - `io::audio::BlockStamp` : `called_us` (horloge hôte lue à l'entrée du callback), `callback_us`
    (`mHostTime`), `playback_us` (estimation cpal) ; `io::system::host_time_us` (mach, même horloge que
    CoreMIDI et CoreAudio).
  - `io::leds::LedState` + `LedBank::set_beats` : un clignotement n'envoie un message qu'aux changements.
  - mapping : `test_hit` sur `a/b.pad.hotcue.1–4`, `Mapping::bound`, `mapping::led_of`.
  - app : LEDs recalculées à chaque tour de boucle (position du moteur en lecture, tempo « libre » à
    l'arrêt) ; ligne `⏱` par frappe ; bilan dans le rapport de 5 s et en fin de session.
  - `LedBank::touched` : une LED que le contrôleur a pu changer seul (pad sous le doigt) est renvoyée.
  - `ControllerLink` : `unpark` du thread de contrôle à chaque message ; app en `park_timeout(2 ms)`.
  - 49 tests verts, clippy debug/release propre.
- Mesures (haut-parleurs du MacBook Air, 48 kHz, 128 frames, horodatage CoreMIDI → sortie) :
  - touche `h` : 7,6 / 8,0 / 8,8 ms (min / médiane / max) ;
  - essai 1, pads, boucle à 2 ms : 29 frappes, 7,1 / 9,4 / 11,3 ms (MIDI → boucle 0,1–2,5) ;
  - essais 2–3, boucle réveillée : 26 frappes, 6,8 / 8,1–8,5 / 10,3 ms (MIDI → boucle 0,0–0,5) ;
  - 0 xrun, 0 message MIDI perdu, charge max 2,1 %.
- Essai utilisateur (3 passages guidés à la voix) : Play fixe / clignotant à l'arrêt ✅, Sync/Cue en
  rythme ✅, clignotement rapide de Sync au pickup puis arrêt en rattrapant ✅, pads : son ✅ et LED pendant
  l'appui seulement ✅ (deux decks), ressenti « immédiat » ✅.
- Écarts :
  - essai 1 : Sync clignotait vite dès le départ (fader Tempo pas sur 120 au lancement → pickup en attente
    sans que l'utilisateur ait rien fait) → clignote seulement pendant 3 s après un geste ignoré ;
  - essai 1 : son de test = clic accentué du métronome, indiscernable → 330 Hz, 150 ms ;
  - essais 1–2 : pads restés allumés après relâchement — bug de l'app (les pads n'étaient jamais remis à
    éteint dans `update_leds`), pas du contrôleur ;
  - l'estimation de sortie de cpal (`playback`) ajoute tampon + offset de sécurité + latence matérielle
    à `mHostTime`, qui est déjà l'heure de remise au périphérique (≈ un tampon + offset après l'appel) :
    elle compte le tampon deux fois (+6,4 ms au lieu de +1,4). Mesuré en comparant avec l'heure réelle de
    l'appel ; la latence matérielle est donc déduite (estimation cpal − tampon).

## Appris / à reporter dans la doc
(reporté dans `docs/hardware/midi-macos.md`.)
- Le sommeil fixe de la boucle de contrôle coûtait jusqu'à 2,5 ms par geste : la réveiller sur arrivée
  MIDI ramène l'étape à ≈ 0,1 ms. Le reste (attente du bloc + tampon) ne baisse qu'avec un buffer plus
  petit (64 frames) ou un déclenchement daté dans le bloc.
- Les règles d'affichage des LEDs doivent être recalculées de zéro à chaque tour (état de base puis
  surcharges), sinon un état « allumé » survit à sa cause.
- `OutputCallbackInfo::timestamp().callback` n'est pas l'heure de l'appel mais `mHostTime` (≈ 5 ms plus
  tard à 128 frames sur le MacBook) ; lire l'horloge hôte soi-même pour l'heure réelle.
- Non compté : transport USB du contrôleur à l'hôte (horodatage CoreMIDI = arrivée, ≤ 1 ms en USB full
  speed), son dans l'air (≈ 3 ms par mètre).

## Questions ouvertes
- Chiffres avec l'UMC1820 (essais faits sur les haut-parleurs du MacBook).
- Mesure acoustique au micro pour valider la part « matériel » déduite de cpal : pas nécessaire tant que
  le ressenti est « immédiat ».
- Buffer 64 frames (≈ −2,7 ms) à évaluer quand la charge réelle des decks sera connue.
- Piste design D1 : références visuelles de l'utilisateur.

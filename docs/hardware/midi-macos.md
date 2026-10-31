# MIDI et audio sous macOS (Apple Silicon)

## CoreMIDI via `midir`

- `midir::MidiInput::connect(port, name, callback, data)` : le callback s'exécute sur un **thread CoreMIDI**
  (pas le thread audio, pas le thread UI). Il reçoit `(timestamp_µs: u64, &[u8], &mut T)`.
- Le timestamp est en **microsecondes, horloge hôte** (base `mach_absolute_time` sur macOS). C'est ce qui
  permet de placer un événement à l'échantillon près dans le bloc audio suivant (voir
  `docs/rust/realtime-audio.md` § Horodatage).
- Un message par appel sur CoreMIDI (les paquets sont découpés par midir) ; le *running status* est résolu.
- Hot-plug : midir ne notifie pas. Stratégie : relister les ports toutes les ~1 s sur un thread de contrôle ;
  si `DJControl MIX` disparaît → passer en état « contrôleur absent » (UI) sans couper le son, et se
  reconnecter automatiquement quand il revient (puis renvoyer `B0 7F 7F` + l'état des LEDs).
- Sortie (LEDs) : `MidiOutputConnection::send(&[u8])` est **bloquant/système** → jamais depuis le thread
  audio. L'audio publie l'état, le thread de contrôle calcule le diff de LEDs et envoie.

## CoreAudio via `cpal`

- Le callback de sortie tourne sur le thread temps réel de CoreAudio (déjà en priorité temps réel :
  la feature `realtime` de cpal est inutile sur macOS, utile sur Linux/Windows).
- Buffer : `BufferSize::Fixed(128)` à 48 kHz = 2,67 ms. Vérifier la plage via `SupportedBufferSize`
  avant de le fixer ; certaines sorties Bluetooth imposent ≥ 512 (et ajoutent ~150 ms : **jamais de casque
  Bluetooth en live**).
- Le système peut changer de périphérique de sortie (branchement d'un casque) → cpal remonte une erreur
  (`DeviceChanged` / `StreamInvalidated`) dans le callback d'erreur → reconstruire le stream depuis le
  thread de contrôle.
- Fréquence : forcer 48 kHz si disponible, sinon suivre le périphérique ; le moteur doit fonctionner à toute
  fréquence (pas de constantes à 48000 en dur dans le DSP).

## Latence de bout en bout (cible)

| Étape | USB | BLE |
|---|---|---|
| Contrôleur → hôte | ~1 ms | 7,5–15 ms + gigue |
| Attente du prochain bloc | ≤ 2,7 ms (128 @ 48 k) | idem |
| Sortie CoreAudio (sécurité + DAC) | ~3–5 ms | idem |
| **Total** | **≈ 5–9 ms** | 15–25 ms |

Les actions **quantifiées** (lancer un pattern à la mesure) ne sont pas sensibles à la latence ; les actions
**immédiates** (jog, filtre, one-shot) le sont → toujours tester sur USB.

## Pièges macOS

- **App Nap** : une app en arrière-plan peut être ralentie (timers, threads non temps réel → le thread de
  contrôle et l'UI). Désactiver via `NSProcessInfo.beginActivity(.userInitiated | .latencyCritical)` (via
  `objc2-foundation`) pendant toute la session de jeu. 🔍 à valider en phase 1.
- Économiseur d'écran / veille pendant un set : même `beginActivity` avec `idleSystemSleepDisabled`.
- Permissions : aucune pour MIDI ou sortie audio (seule l'entrée micro en demanderait).
- Volumes amovibles : voir la mémoire utilisateur (TCC) si des samples sont lus depuis un disque externe.

# Hercules DJControl Mix — fiche technique MIDI

> Sources : mapping officiel Mixxx (`research/mixxx/Hercules DJControl MIX.midi.xml` + `…-scripts.js`,
> auteurs DJ Phatso / Hercules Technical Support et Kerrick Staley, Mixxx ≥ 2.4, licence GPL — on s'en sert
> comme **référence de faits**, on ne copie pas de code), manuel Mixxx
> <https://manual.mixxx.org/2.5/en/hardware/controllers/hercules_djcontrol_mix>, manuel Hercules
> <https://ts.hercules.com/download/sound/manuals/DJC_Mix/User_Manuals/DJControl_Mix_user_manual_EN.pdf>.
>
> Légende de fiabilité : ✅ confirmé par le mapping Mixxx · 🔍 à vérifier au sniffer (`/midi-sniff`) ·
> 🧪 vérifié sur l'appareil de l'utilisateur (à remplir au fil des tests).

## Ne pas confondre

- **DJControl Mix** (réf. 4780921, 2021–) : le modèle de l'utilisateur. BLE + USB-C.
- **DJControl Mix Ultra** (4780942, 2025) : autre contrôleur (jogs tactiles, rétroéclairage, mapping différent).
- **DJControl Starlight** : quasi identique électroniquement ; son mapping Mixxx a servi de base.

## Connexion

| Lien | État | Notes |
|---|---|---|
| **USB-C → Mac** | 🧪 2026-10-31 : USB MIDI *class compliant*, aucun pilote | **Lien de référence du projet.** Alimente aussi le contrôleur. Ports CoreMIDI entrée **et** sortie : `DJCONTROL MIX`. ⚠️ Un câble de charge seule ne fait rien apparaître (pas même en USB) : utiliser un câble data, de préférence branché directement sur le Mac. |
| Bluetooth LE MIDI | 🔍 fonctionne sous macOS d'après le forum Mixxx, conçu pour djay mobile | Appairage : *Configuration audio et MIDI → Fenêtre MIDI → Configuration Bluetooth*. Latence 7,5–15 ms + gigue (intervalle de connexion BLE). Hors périmètre v1. |

Pas d'interface audio intégrée : le son sort par la carte son du Mac (ou une interface externe). La
pré-écoute casque de djay passe par un câble séparateur stéréo → **pas de cue casque réel** sans interface
4 canaux ; la fonction « casque » des boutons PFL est donc libre pour notre usage.

## Inventaire physique

Par deck (A gauche, B droite) : jog (bord + dessus tactile), Play, Cue, Sync, PFL (casque), fader de
volume, potard Filter/Bass + bouton de bascule, fader/potard Tempo, 4 pads + 4 boutons de mode
(Hot Cue, Loop, FX, Sampler).
Global : crossfader, Master, Headphone, Shift, Vinyl.

## Carte des messages entrants (contrôleur → hôte)

Notation : `status` hexadécimal (nibble haut = type, nibble bas = canal 0-based), `n` = note/CC.

### Boutons (Note On, `0x7F` = appuyé, Note On vélocité `0x00` = relâché) 🧪

| Contrôle | Deck A | Deck B | Avec Shift A | Avec Shift B |
|---|---|---|---|---|
| Sync | `91 05` | `92 05` | `94 05` | `95 05` |
| Cue | `91 06` | `92 06` | `94 06` | `95 06` |
| Play | `91 07` | `92 07` | `94 07` | `95 07` |
| Jog — toucher le dessus | `91 08` | `92 08` | `94 08` | `95 08` |
| PFL (casque) | `91 0C` | `92 0C` | `94 0C` | `95 0C` |
| Vinyl (global) | `91 03` **et** `92 03` (un appui envoie les deux) 🧪 | | — | — |
| Bascule Filter/Bass (global) | `90 01` 🧪 | | | |
| Bouton de mode Hot Cue | `91 0F` 🧪 | `92 0F` 🧪 | | |
| Bouton de mode Sampler | `91 10` 🧪 | `92 10` 🧪 | | |
| Shift + bouton de mode → **FX** | `91 11` 🧪 | `92 11` 🧪 | | |
| Shift + bouton de mode → **Loop** | `91 12` 🧪 | `92 12` 🧪 | | |
| **Shift** (global) | `90 03` (≥ `0x40` = tenu) | | | |

→ **Shift est géré par le contrôleur** : il décale le canal de +3 (ch2/3 → ch5/6). L'hôte n'a pas besoin de
suivre l'état de Shift pour ces boutons, mais il le reçoit quand même (`90 03`).
Exception 🧪 : les **boutons de mode** restent sur le canal du deck avec Shift, la couche Shift est
codée dans la note (`11` = FX, `12` = Loop).

### Pads (Note On, canal 7 = deck A `0x96`, canal 8 = deck B `0x97`) ✅

`note = offset_du_mode + index_pad (0..3) + (Shift ? 0x08 : 0)`

| Mode (bouton matériel) | Offset | Pads | Shift + pads |
|---|---|---|---|
| Hot Cue | `0x00` | `00–03` | `08–0B` |
| Sampler | `0x10` | `10–13` | `18–1B` 🧪 |
| FX | `0x20` | `20–23` | `28–2B` |
| Loop | `0x30` | `30–33` | `38–3B` 🧪 |

→ **Le mode de pad est mémorisé dans le contrôleur** : on reçoit directement 4 modes × 4 pads × 2 (Shift) =
**32 fonctions par deck** sans aucune logique d'état côté hôte.
🧪 2026-10-31 : les boutons de mode envoient aussi leur propre message (table des boutons) ; **vélocité
fixe `0x7F`** (pads non dynamiques) ; relâchement = Note On vélocité `0x00`.

### Continus (Control Change) — 🧪 tous en 14 bits

🧪 2026-10-31 : **tous les contrôles absolus sont en 14 bits** (Mixxx n'en lit que 7) : MSB sur le CC
indiqué, LSB sur `CC + 0x20`, MSB envoyé en premier. Valeur = `MSB<<7 | LSB` (0–16383).

| Contrôle | Status | CC | Résolution / codage |
|---|---|---|---|
| Crossfader | `B0` | `00` (+ `20`) | 14 bits |
| Master | `B0` | `03` (+ `23`) | 14 bits |
| Headphone | `B0` | `04` (+ `24`) | 14 bits |
| Volume deck A / B | `B1` / `B2` | `00` (+ `20`) | 14 bits |
| Filter deck A / B | `B1` / `B2` | `01` (+ `21`) | 14 bits, plage complète 0–16383 🧪 ; envoyé quand la bascule `90 01` est sur Filter |
| Bass (EQ grave) deck A / B | `B1` / `B2` | `02` (+ `22`) | 14 bits 🧪 ; envoyé quand la bascule est sur Bass (état au démarrage, le dump n'envoie que celui-là) |
| Tempo deck A / B | `B1` / `B2` | `08` (+ `28`) | 14 bits ; au repos centré à 8192 sur le deck B lors du test |
| Jog — bord (bend) | `B1` / `B2` | `09` | **relatif** : `0x01` = horaire, `0x7F` = anti-horaire (7 bits signé) |
| Jog — dessus touché | `B1` / `B2` | `0A` | relatif, idem |
| Jog — dessus + Shift | `B4` / `B5` | `0A` | relatif, idem |

Jog 🧪 : chaque message vaut **±1 cran** (aucune valeur d'accélération), Mixxx compte **248 crans/tour**.
Débit max mesuré : **~250 messages/s par le bord, ~300/s par le dessus** → la vitesse se déduit de la
fréquence des messages (à lisser côté hôte).
⚠️ 🧪 Le capteur tactile du dessus est **instable en rotation rapide par le bord** : il émet des
`JogTouch down/up` parasites (quelques dizaines de ms) et le jog bascule de `09` à `0A`. Le mapping doit
filtrer : ignorer un toucher plus court que ~80 ms, ou traiter bord et dessus comme le même geste quand
la rotation est continue.

Continus 🧪 : tous couvrent la plage complète 0–16383 (pas de zone morte aux butées) ; un fader
balayé vite envoie une paire MSB/LSB toutes les ~2 ms.

## Messages sortants (hôte → contrôleur)

| Message | Effet |
|---|---|
| `B0 7F 7F` | **Demande de vidage** 🧪 : en ~1 ms, le contrôleur renvoie 18 messages (MSB+LSB) dans cet ordre : crossfader, master, headphone, volume A, volume B, tempo A, tempo B, bass A, bass B. **Pas de filtre** dans le dump (seul le CC du mode actif du potard ?). À envoyer à l'init. |
| `B0 7F 00` | Éteint toutes les LEDs (à envoyer à la fermeture). ✅ |
| **Règle générale des LEDs** 🧪 | Note On à la **même adresse que le message d'entrée** du contrôle ; tout-ou-rien : `0x01–0x7F` allume, `0x00` éteint (ni intensité, ni couleur). **Garder le port MIDI ouvert** : envoyées par une connexion qui se ferme aussitôt, certaines LEDs (Sync/Cue) ne restent pas allumées. |
| `91 05` / `92 05` | Sync A / B 🧪 |
| `91 06` / `92 06` | Cue A / B 🧪 |
| `91 07` / `92 07` | Play A / B 🧪 |
| `91 03` | Vinyl 🧪 |
| `96 nn` / `97 nn` | **Pads A / B** 🧪, `nn` = offset du mode (`00` Hot Cue, `10` Sampler, `20` FX, `30` Loop) + pad (0–3) ; pad 1 = haut gauche (pads en 2×2). **Seules les LEDs du mode affiché sont visibles** : l'hôte doit garder l'état des 4 modes et le renvoyer à chaque `PadModeSelect`. |
| `91 0C` (casque A) | Aucune LED 🧪. |
| Boutons de mode | S'allument quand on balaye `91 00–0F` en bloc, mais aucune sous-plage ne les isole → probablement **gérés par le contrôleur** (indicateur du mode actif) 🔍. |
| `96 70–73` | Allume aussi les pads B (ordre inversé) — adresse annexe, **ne pas utiliser** (préférer `97`). |
| `93 05` / `94 05` | LED « sync leader » selon Mixxx — 🔍 incohérent (`94 05` est aussi Shift+Sync A en entrée), à tester. |

Pads 🧪 : disposés en **2×2**, numérotés sur l'appareil (1 = haut gauche) ; pad 1 = index 0 en entrée
comme en sortie. Le retour LED des pads et du transport est **une exigence de l'utilisateur**
(2026-10-31) : implémenté dans le décodeur (`led_message`) et planifié en itération 009.

## Comportements à connaître

- Shift ne génère pas d'« appui long » : c'est un modificateur matériel.
- Les potards ne sont pas motorisés/sans fin : après un changement de scène logicielle, la valeur physique
  peut différer de la valeur logicielle → prévoir un **mode « pickup » / soft-takeover** (le paramètre ne
  suit qu'une fois le potard passé par la valeur courante).
- Pas de zone morte en butée 🧪 ; prévoir quand même un petit seuil logiciel aux extrêmes pour les « kills ».

## Checklist de vérification sur l'appareil (phase 0)

- [x] 🧪 Nom exact du port CoreMIDI en USB : `DJCONTROL MIX` (entrée et sortie).
- [x] 🧪 Boutons de mode : `0F`/`10`, Shift → `11` (FX)/`12` (Loop), sur le canal du deck.
- [x] 🧪 Shift + pads en Sampler et Loop : `18–1B`, `38–3B`.
- [x] 🧪 Relâchement : Note On vélocité 0.
- [x] 🧪 Vélocité des pads : fixe 127.
- [x] 🧪 Bascule Filter/Bass : `90 01`, globale, bascule CC `01` ↔ `02`. (Effet sur le deck B à confirmer.)
- [x] 🧪 Fader Tempo et tous les continus : 14 bits, MSB puis LSB, plage complète 0–16383.
- [x] 🧪 LEDs : même adresse que l'entrée, tout-ou-rien ; transport, Vinyl et pads (mode affiché) pilotables ; casque non.
- [x] 🧪 `B0 7F 7F` renvoie tout l'état sauf le filtre (voir ci-dessus).
- [x] 🧪 Débit max des jogs : ~250/s (bord), ~300/s (dessus) ; toucher instable en rotation rapide.
- [ ] BLE sous macOS 15 : fonctionne ? latence mesurée ? → reporté en phase 7.

Reporter chaque résultat ici avec 🧪 et la date.

# Checklist d'essai pas à pas — LiveDeck groovebox

À suivre seul, au contrôleur, dans l'ordre. Chaque pas : **un geste**, **ce qu'on doit entendre et
voir**, une case à cocher et une ligne pour noter. Compter environ 45 min (pauses comprises).

Légende : ☐ à faire · ✅ ok · ❌ problème (noter quoi) · ➖ pas testé.
« Écran » = la zone centrale de la fenêtre ; « panneau A / B » = les colonnes gauche / droite.

---

## 0. Préparation (5 min)

| # | Faire | Vérifier | ☐ | Notes |
|---|---|---|---|---|
| 0.1 | Brancher le DJControl Mix en USB, casque ou enceintes branchés, volume bas | — | ☐ | |
| 0.2 | Dans un terminal : `cd ~/dev/livedeck && cargo run -p app --release` | la fenêtre s'ouvre, la lecture démarre, le terminal écrit « contrôleur connecté » | ☐ | |
| 0.3 | Regarder le bas de l'écran | « charge x % » (pas d'alerte rose) | ☐ | |
| 0.4 | Mettre tous les potards et faders du contrôleur **au milieu**, crossfader au centre | — | ☐ | |
| 0.5 | Appuyer sur **F** dans la fenêtre (plein écran), puis F à nouveau | la fenêtre passe en plein écran et revient | ☐ | |

> Si quelque chose bloque dès ici : noter le message du terminal, Ctrl-C, et s'arrêter.

---

## 1. Transport et tempo (3 min)

| # | Geste | Attendu | ☐ | Notes |
|---|---|---|---|---|
| 1.1 | **Play A** (ou Play B) | la lecture s'arrête ; l'icône ▶ devient un carré ; la LED Play clignote | ☐ | |
| 1.2 | **Play A** à nouveau | la lecture repart | ☐ | |
| 1.3 | **Shift + Play A** | reprise à la mesure 1 (l'écran affiche 1.1) | ☐ | |
| 1.4 | **Shift + jog A** (tourner) | le tempo change, l'écran affiche le nouveau BPM et une étiquette « Tempo » | ☐ | |
| 1.5 | Taper **Sync A** 4 fois en rythme | le tempo suit la frappe | ☐ | |
| 1.6 | **PFL A** | métronome on / off | ☐ | |

---

## 2. Rythme — deck gauche (8 min)

Mettre le deck gauche en mode **Hot Cue** (bouton de mode du contrôleur). L'onglet « Hot Cue » du
panneau A doit s'allumer.

| # | Geste | Attendu | ☐ | Notes |
|---|---|---|---|---|
| 2.1 | Pads **Hot Cue A 1, 2, 3, 4** (un par mesure) | le pad choisi clignote en pointillé, « pattern n » apparaît dans « À la mesure », puis la piste Rythme change au 1er temps ; **le kick reste sur chaque temps** | ☐ | groove de chaque pattern : |
| 2.2 | **Jog A** (tourner, la main posée dessus ou par la tranche) | la densité monte / descend (« densité x % » dans l'écran) ; à 0 % il ne reste que les kicks | ☐ | sensibilité : |
| 2.3 | Mode **Sampler**, pads **Sampler A 1–4** | remplissages : 1 roulement de caisse claire, 2 roulement de perc, 3 « drop » (silence jusqu'à la mesure), 4 montée de hats | ☐ | |
| 2.4 | **Shift + Sampler A 1, 2, 3** | kits **808 · boom**, **909 · techno**, **Dub**, à la mesure ; le nom du kit change en haut à droite de l'écran | ☐ | caractère de chaque kit : |
| 2.5 | Mode **FX**, pads **FX A 1–4** | mutes kick / rim / hats / perc : pad allumé = la voix joue ; la ligne correspondante grise dans la piste Rythme | ☐ | |
| 2.6 | **Shift + FX A 1** | même chose que sans Shift (mute du kick) | ☐ | |
| 2.7 | Mode **Loop**, **tenir** un pad Loop A 1–4 | beat-repeat 1/4, 1/8, 1/16, 1/32 tant que le pad est tenu ; « beat-repeat » dans l'écran | ☐ | |
| 2.8 | **Shift + Loop A 1–4** | swing 50 · 54 · 58 · 62 % (affiché en haut à droite) | ☐ | |
| 2.9 | **Volume A** | niveau de la batterie (étiquette « Batterie ») | ☐ | |

---

## 3. Basse — deck gauche (8 min)

> Le potard **Filter/Bass** est **un seul potard** par deck. Le **bouton Filter/Bass** du
> contrôleur le fait passer de Filter à Bass. Le panneau A écrit « Filter/Bass : sur Filter » ou
> « sur Bass », et l'anneau du rôle inactif est **estompé**.

| # | Geste | Attendu | ☐ | Notes |
|---|---|---|---|---|
| 3.1 | Tourner le potard Filter/Bass A, puis appuyer sur le **bouton Filter/Bass** et le retourner | le panneau A indique le rôle ; l'anneau actif bouge, l'autre est estompé | ☐ | |
| 3.2 | Sur **Filter** : tourner | **coupure** de la basse (anneau « Coupure », « coupure x % » dans la piste Basse) | ☐ | plage : |
| 3.3 | Sur **Bass** : tourner | **résonance** | ☐ | plage : |
| 3.4 | Sur **Bass** : **Shift +** tourner | **enveloppe** du filtre ; le potard doit d'abord rejoindre la valeur (étiquette en contour) avant d'agir | ☐ | |
| 3.5 | **Fader Tempo A** | **decay** de l'enveloppe du filtre (notes plus ou moins « wah ») | ☐ | |
| 3.6 | **Shift + fader Tempo A** | **durée des notes** ; tout en haut, la note est tenue | ☐ | |
| 3.7 | **Shift + Volume A** | niveau de la basse | ☐ | |
| 3.8 | **Shift + Hot Cue A 1–4** | ligne de basse 1–4 à la mesure | ☐ | |
| 3.9 | **Shift + Sampler A 4** (plusieurs fois, une par mesure) | une ligne **au hasard** arrive à la mesure (« basse au hasard » dans « À la mesure ») | ☐ | intérêt des lignes : |
| 3.10 | **Cue A** | mute de la basse (piste Basse grise, LED Cue éteinte) ; rappuyer | ☐ | |
| 3.11 | **Cue B**, puis **Shift + Cue B** | tonalité −1 / +1 demi-ton, à la mesure (en haut à droite) | ☐ | |

---

## 4. Accords et phrase — deck droit (10 min)

| # | Geste | Attendu | ☐ | Notes |
|---|---|---|---|---|
| 4.1 | Mode **Hot Cue**, tenir **Hot Cue B 1–4** | accords I, IV, V, vi tant que tenus | ☐ | son des stabs : |
| 4.2 | **Shift + Hot Cue B 1–4** | accords ii, iii, vii, I à l'octave | ☐ | |
| 4.3 | Mode **Loop**, **Loop B 2** puis un accord | latch : l'accord reste après relâchement ; Loop B 2 à nouveau pour couper | ☐ | |
| 4.4 | **Loop B 3** / **Shift + Loop B 3** | octave des accords + / − | ☐ | |
| 4.5 | **Loop B 4** / **Shift + Loop B 4** | extension (7e, sus4, add9) / retour à la triade | ☐ | |
| 4.6 | **Shift + Loop B 2** | renversement de l'accord | ☐ | |
| 4.7 | Mode **Sampler**, **Sampler B 1 « Générer »** | la phrase arrive **à la mesure** ; des notes bleues dans la piste Phrase ; état « libre » | ☐ | |
| 4.8 | Écouter 8 mesures sans toucher | libre, la phrase **change un peu toute seule** toutes les 4 mesures | ☐ | |
| 4.9 | **Fader Tempo B** en bas puis en haut, Générer à chaque fois | peu de notes / beaucoup de notes | ☐ | |
| 4.10 | **Sampler B 2 « Muter »** | quelques notes bougent, à la mesure | ☐ | |
| 4.11 | **Sampler B 3 « Figer »** | état « figée », la phrase ne bouge plus ; la LED du pad reste allumée | ☐ | |
| 4.12 | **Sampler B 4 « Effacer »** | la phrase se vide à la mesure (« vide ») | ☐ | |
| 4.13 | Mode **Loop**, **tenir Loop B 1 (Rec)**, jouer 2–3 accords en Hot Cue, relâcher | « ● rec » dans l'écran ; les accords apparaissent en **blocs larges** dans la piste Phrase et sont rejoués à chaque mesure | ☐ | calage : |
| 4.14 | **Jog B** (tourner) | transposition dans la gamme (accords et phrase), « transpo » dans l'écran | ☐ | |
| 4.15 | **Shift + jog B** | gamme suivante, à la mesure | ☐ | |
| 4.16 | Potard Filter/Bass B **sur Filter** | **brillance** du synthé (anneau « Brillance ») | ☐ | |
| 4.17 | **Volume B** / **Shift + Volume B** | niveau de la mélodie / du métronome | ☐ | |

---

## 5. Effets dub (6 min)

Générer une phrase (4.7) et jouer quelques accords avant.

| # | Geste | Attendu | ☐ | Notes |
|---|---|---|---|---|
| 5.1 | Potard Filter/Bass B **sur Bass** | envoi au **delay** (anneau « Delay ») | ☐ | |
| 5.2 | **Shift +** ce potard | **répétitions** du delay | ☐ | |
| 5.3 | **Headphone** | envoi à la **réverb** (« Réverb x % » en bas de l'écran) | ☐ | |
| 5.4 | Mode **FX**, **tenir FX B 1** puis relâcher | **throw delay** : tout part dans le delay tant que le pad est tenu ; voyant « Throw delay » allumé ; les échos se prolongent après | ☐ | effet dub : |
| 5.5 | **Tenir FX B 2** | **throw réverb** ; voyant « Throw réverb » | ☐ | |
| 5.6 | **FX B 3** (plusieurs fois) / **Shift + PFL B** | temps du delay 1/8 pointé, 1/4, 1/8, 1/4 pointé, 1/16 | ☐ | |
| 5.7 | **FX B 4** | arpège on / off (les accords tenus sont arpégés) | ☐ | |
| 5.8 | **Shift + FX B 1–4** | arpège sur 1 à 4 octaves | ☐ | |

---

## 6. Mixer (2 min)

| # | Geste | Attendu | ☐ | Notes |
|---|---|---|---|---|
| 6.1 | **Crossfader** tout à gauche, centre, tout à droite | rythme seul / les deux / mélodie seule ; le curseur suit en bas de l'écran | ☐ | |
| 6.2 | **Master** | niveau général ; le voyant **limiteur** s'allume si on pousse fort | ☐ | |

---

## 7. Interface (5 min)

| # | Vérifier | ☐ | Notes |
|---|---|---|---|
| 7.1 | L'onglet de mode de chaque panneau suit le bouton de mode du contrôleur | ☐ | |
| 7.2 | Les 4 pads affichés s'allument comme les LEDs du contrôleur | ☐ | |
| 7.3 | **Tenir Shift** : les panneaux affichent la seconde couche (badge « SHIFT », noms des pads et des anneaux) | ☐ | |
| 7.4 | Tout ce qui est affiché comme réglage répond à une commande (sinon noter lequel) | ☐ | |
| 7.5 | Écran à **30 % de luminosité, à 1 m** : tempo, mesure, ce qui attend la mesure et les pistes se lisent | ☐ | ce qui se lit mal : |
| 7.6 | L'étiquette du geste (au-dessus de la zone touchée) disparaît après ~1 s | ☐ | |

---

## 8. Robustesse (5 min)

| # | Geste | Attendu | ☐ | Notes |
|---|---|---|---|---|
| 8.1 | **Débrancher** le contrôleur en jeu | le son continue ; alerte « Contrôleur absent » en bas de l'écran | ☐ | |
| 8.2 | **Rebrancher** | l'alerte disparaît, les LEDs se rallument, les gestes répondent | ☐ | |
| 8.3 | Débrancher / rebrancher le **casque** (sortie audio) | le son revient sur la nouvelle sortie | ☐ | |

---

## 9. Jeu libre (10 min)

Jouer un vrai morceau : construire, faire tourner, faire varier, couper, relancer.

| # | Vérifier | ☐ | Notes |
|---|---|---|---|
| 9.1 | Aucun craquement ni coupure | ☐ | |
| 9.2 | En bas de l'écran : jamais d'alerte « Xrun » ni « Surcharge CPU » | ☐ | |
| 9.3 | Ce qui manque, gêne ou plaît | ☐ | |

---

## 10. Fin et compte rendu

1. Ctrl-C dans le terminal.
2. Copier les **3 dernières lignes de bilan** du terminal (`[… BPM │ charge max … │ xruns … │ NaN …]`).
3. Me donner : les cases ❌ avec leurs notes, les notes « plage / caractère / intérêt », et le bilan.

**Critères de réussite** : 0 xrun, 0 NaN, charge max < 15 % ; chaque pas fait ce qui est attendu.

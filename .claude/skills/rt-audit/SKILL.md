---
name: rt-audit
description: Audit temps réel d'un diff ou de fichiers LiveDeck — traque allocations, libérations, verrous, E/S, panics, boucles non bornées, dénormaux et erreurs de threading dans tout code atteignable depuis le callback audio. Utiliser après toute modification de crates/engine, d'un callback cpal/midir, ou sur demande « /rt-audit ».
argument-hint: "[chemin ou plage git, défaut : diff courant]"
---

# /rt-audit

Cible : `$ARGUMENTS` ; par défaut le diff non commité (`git diff` + fichiers non suivis de `crates/`).

## Procédure

1. Identifier les **points d'entrée audio** : le callback de `build_output_stream`, `Engine::process`/
   `render`, et tout `impl` appelé depuis eux. Construire la liste des fonctions atteignables touchées par
   la cible (suivre les appels, y compris les traits et closures).
2. Pour chacune, vérifier la table de [docs/rust/realtime-audio.md](../../../docs/rust/realtime-audio.md) §1 :
   - allocation : `Vec::new/push/extend/collect/clone/to_vec`, `Box::new`, `String`, `format!`,
     `to_string`, `Arc::new`, `HashMap` insert, `vec![]`, croissance implicite ;
   - libération : `drop` (explicite ou fin de portée) d'un `Vec`/`Box`/`Arc` détenu, remplacement
     `x = new_vec` ;
   - verrous et blocage : `Mutex`, `RwLock`, `Condvar`, `mpsc`, `crossbeam` bloquant, `thread::sleep`,
     `Once` bloquant, `lazy_static`/`OnceLock` initialisé dans le callback ;
   - E/S : `println!`, `eprintln!`, `dbg!`, `log!`, `tracing`, fichiers, MIDI out ;
   - panics : `unwrap`, `expect`, `panic!`, `unreachable!`, indexation `[]` non prouvée, division entière
     par une variable, `as` qui tronque une valeur utilisée comme index ;
   - bornes : boucles dépendant de données externes sans plafond ;
   - numérique : dénormaux (décroissances sans plancher), NaN possibles (`log(0)`, `x/0.0`), constantes
     de fréquence en dur ;
   - threading : file SPSC avec deux producteurs, `Ordering` incohérent, état partagé non `Copy`.
3. Lancer, si le workspace compile :
   ```bash
   cargo clippy -p engine --all-targets -- -D warnings
   cargo test -p engine no_alloc
   ```
4. Rendre un rapport : tableau `fichier:ligne | catégorie | gravité (bloquant/à surveiller) | correction`.
   Bloquant = peut produire un clic/crash sur scène. Proposer les corrections ; les appliquer si
   l'utilisateur ou l'itération en cours le prévoit.
5. Si un nouveau type de piège est découvert, l'ajouter à la table de `realtime-audio.md`.

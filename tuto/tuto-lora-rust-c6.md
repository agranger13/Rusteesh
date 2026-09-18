# Nœud LoRa MeshCore en Rust sur ESP32-C6

**Tutoriel complet pour débutant en Rust**

Ce document suppose que tu sais faire de l'électronique et que tu as déjà programmé un microcontrôleur (Arduino, PlatformIO, C/C++). Il ne suppose **aucune** connaissance de Rust : chaque partie introduit les concepts du langage au moment où tu en as besoin.

**Matériel visé** : ESP32-C6 (512 Ko SRAM, 4 Mo flash, pas de PSRAM) + module LoRa SX126x + écran TFT SPI + 2 boutons.

**Objectif final** : un nœud qui écoute un maillage MeshCore existant, décode les paquets, les affiche, et sait émettre ses propres messages en `PAYLOAD_TYPE_RAW_CUSTOM`.

---

## Comment lire ce document

Chaque partie suit la même structure :

- **Objectif** — ce que tu construis
- **Concepts Rust** — ce que tu apprends du langage
- **Mise en œuvre** — le code et les commandes
- **Livrable** — comment savoir que c'est fini
- **Pièges** — ce qui va te coûter du temps
- **Exercices** — trois par partie, de difficulté croissante

### Les exercices

| Marqueur | Sens |
|---|---|
| ★ | application directe, quelques minutes |
| ★★ | demande de réfléchir, 30 min à 1 h |
| ★★★ | difficile, peut prendre une soirée |
| 💻 | **faisable sur PC, sans matériel** |

Les exercices marqués 💻 se font entièrement avec `cargo test` sur ton ordinateur. Ils sont parfaits pour avancer dans le train ou quand tu n'as pas ta carte sous la main — et ce sont souvent les plus formateurs, parce qu'ils portent sur la logique pure.

**Les corrections sont dans un fichier séparé** (`corrections-tuto-lora-rust-c6.md`). Cherche vraiment avant d'y aller : sur les exercices ★★ et ★★★, c'est la bataille avec le compilateur qui t'apprend quelque chose, pas la lecture de la solution.

Les parties 0 à 7 ne nécessitent **qu'un seul nœud**. Seule la partie 8 exige une deuxième carte.

---

## Avant tout : ce qui change par rapport à Arduino

Si tu viens de PlatformIO, quatre choses vont te surprendre. Autant les nommer maintenant.

### 1. Il n'y a pas de `setup()` et `loop()`

En Rust embarqué, tu écris un `main` qui ne retourne **jamais**. Le type de retour est `!` (« never »), et le compilateur vérifie que tu ne peux effectivement pas en sortir.

```rust
#[main]
fn main() -> ! {
    // ton setup ici
    loop {
        // ta loop ici
    }
}
```

C'est plus honnête : sur un microcontrôleur, il n'y a nulle part où retourner.

### 2. Les périphériques sont des valeurs que tu possèdes

En Arduino, `digitalWrite(13, HIGH)` marche depuis n'importe où. En Rust, la GPIO 13 est un **objet** que tu prends, que tu configures, et que tu déplaces. Une fois pris, plus personne d'autre ne peut le prendre.

Ça paraît lourd au début. C'est ce qui fait qu'un conflit de broches devient une **erreur de compilation** au lieu d'un bug matériel à 2 h du matin.

### 3. Pas de bibliothèque standard

Tu écris `#![no_std]` en haut du fichier. Tu perds `println!`, `Vec`, `String`, les fichiers, les sockets. Tu gardes tout le langage : traits, génériques, `Option`, `Result`, itérateurs, sécurité mémoire.

Sur ton C6, avec 512 Ko dont 50 à 70 partent au WiFi, ce n'est pas un choix esthétique : c'est la seule option viable.

### 4. Le compilateur va beaucoup t'engueuler

C'est normal, et c'est le but. En C++, un mauvais pointeur compile et plante en vol. En Rust, il ne compile pas. Le temps que tu passes à batailler avec le compilateur est du temps que tu ne passes pas à déboguer sur la carte — où tu n'as ni `printf` confortable, ni débogueur évident.

**Conseil pratique** : lis les messages d'erreur en entier. Le compilateur Rust est réputé pour ses diagnostics ; il te propose très souvent la correction exacte.

---

## Partie 0 — Reconnaissance (sans Rust)

### Objectif

Savoir dans quoi tu te lances avant d'écrire une ligne de code.

### Pourquoi cette étape

Si tes paramètres radio ne correspondent pas exactement à ceux du maillage local, ton nœud **n'entendra rien**. Pas de message d'erreur, pas d'indice : juste le silence. Tu chercheras le bug dans ton code Rust pendant des jours alors qu'il est dans une constante.

Une soirée ici t'économise la classe de bugs la plus pénible du projet.

### Mise en œuvre

**1. Flashe MeshCore C++ sur ton C6.**

Utilise le web flasher : <https://meshcore.ninja/> ou le flasher officiel. Choisis la variante **Companion Radio BLE** (ou USB si le BLE pose problème). Pars d'une variante proche de ta carte — `xiao_c6` est un bon point de départ pour du C6 générique.

**2. Connecte l'application mobile MeshCore** et regarde ce qui apparaît.

**3. Relève et note ces valeurs** — c'est ta fiche de référence pour tout le projet :

| Paramètre | Valeur | Où la trouver |
|---|---|---|
| Fréquence | ex. 869.525 MHz | réglages radio de l'app |
| Bande passante (BW) | ex. 250 kHz | idem |
| Spreading factor (SF) | ex. 11 | idem |
| Coding rate (CR) | ex. 5 (4/5) | idem |
| Sync word | souvent 0x12 | doc MeshCore / source |
| Longueur de préambule | souvent 16 | idem |

**4. Note ton brochage** — SPI de la radio (SCK, MOSI, MISO, NSS, RESET, BUSY, DIO1), SPI de l'écran (SCK, MOSI, CS, DC, RST, BL), et tes deux boutons.

**5. Vérifie qu'il y a du monde.** Laisse tourner quelques heures. Des contacts apparaissent-ils ? Si tu ne vois aucun répéteur, relis la section « bifurcation » : sans infrastructure autour, la trame MeshCore te coûte de la complexité sans bénéfice, et un protocole maison est plus sensé.

### Livrable

Une fiche écrite avec tes paramètres RF et ton brochage. Garde-la ouverte pendant tout le reste du tutoriel.

### Pièges

- **Ne devine pas les paramètres.** Les valeurs par défaut de MeshCore ont changé entre versions, et certaines communautés locales utilisent des réglages personnalisés.
- **Vérifie ta bande légale.** EU868 en Europe, US915 en Amérique du Nord. Un module de la mauvaise bande ne rattrapera jamais son retard.
- **Jamais d'alimentation sans antenne.** L'étage de puissance se détruit en émettant dans un circuit ouvert.

### Exercices

**0.1 ★ — La fiche de référence**
Remplis intégralement le tableau des paramètres RF et note ton brochage complet. Garde-la à portée pendant tout le projet.

**0.2 ★★ — Calcule ton temps d'antenne** 💻
À partir de ton SF, ta BW et ton CR, calcule le temps d'antenne (*time on air*) d'un paquet de 50 octets. Puis déduis-en combien de temps tu dois te taire pour respecter le 1 % réglementaire, et combien de messages par heure tu as légalement le droit d'émettre.

Utilise un calculateur en ligne pour vérifier, mais pose d'abord le calcul toi-même : préambule + en-tête + payload, en symboles, multiplié par la durée d'un symbole (2^SF / BW).

**0.3 ★★★ — Cartographie ton maillage**
Laisse ton nœud MeshCore tourner 24 h. Note combien de répéteurs distincts apparaissent, à quelle distance, et estime le nombre de paquets qui transitent par heure. Ces chiffres déterminent le dimensionnement de ton cache de déduplication en partie 7.

---

## Partie 1 — Chaîne d'outils et premier flash

### Objectif

Une LED qui clignote. C'est tout, et c'est suffisant : tu valides l'installation, la compilation croisée et le flash.

### Concepts Rust

#### Cargo, crates, `Cargo.toml`

Rust n'a pas d'IDE obligatoire ni de système de projet propriétaire. **Cargo** fait tout : dépendances, compilation, tests, flash (via extensions).

Une **crate** est une bibliothèque ou un binaire. L'équivalent d'une bibliothèque Arduino, mais avec versionnage sémantique et résolution automatique des dépendances.

Le fichier `Cargo.toml` déclare ton projet :

```toml
[package]
name = "mon-noeud"
version = "0.1.0"
edition = "2021"

[dependencies]
esp-hal = { version = "1.1.0", features = ["esp32c6", "unstable"] }
esp-backtrace = { version = "0.19.0", features = ["esp32c6", "panic-handler", "println"] }
esp-println = { version = "0.17.0", features = ["esp32c6"] }
```

Les **features** sont des options de compilation. `features = ["esp32c6"]` dit à esp-hal quelle puce cibler ; sans ça, il ne peut pas savoir.

#### La cible (target triple)

Ton PC compile pour lui-même par défaut. Pour ton C6, il faut une **cible croisée** :

```
riscv32imac-unknown-none-elf
```

Décodage : architecture RISC-V 32 bits, avec extensions **i**nteger, **m**ultiply, **a**tomic, **c**ompressed ; pas de fabricant particulier ; **`none`** = pas de système d'exploitation ; format ELF.

Ce `none` au milieu, c'est littéralement ce que veut dire `no_std`.

#### Les attributs `#![no_std]` et `#![no_main]`

```rust
#![no_std]
#![no_main]
```

Le `#!` (avec le point d'exclamation) signifie « s'applique au fichier entier ». Sans le `!`, un `#[...]` s'applique à l'élément qui suit.

- `no_std` : ne lie pas la bibliothèque standard
- `no_main` : n'utilise pas le point d'entrée normal de Rust (qui suppose un OS qui passe `argc`/`argv`)

#### Le panic handler

En Rust, un `panic!` est une erreur irrécupérable. Avec `std`, le runtime affiche un message et termine le processus. Sans OS, **tu dois dire quoi faire**.

La crate `esp-backtrace` s'en charge : elle affiche une trace sur la sortie série puis redémarre. C'est pour ça qu'elle est dans tes dépendances alors que tu ne l'appelles jamais explicitement — d'où le `use esp_backtrace as _;` que tu verras (le `as _` signifie « importe-la pour ses effets, je n'utilise pas son nom »).

#### Premières notions d'ownership

```rust
let peripherals = esp_hal::init(config);
let mut led = Output::new(peripherals.GPIO0, Level::Low, OutputConfig::default());
```

Ligne 1 : tu récupères **toutes** les périphériques de la puce, dans une seule structure. `esp_hal::init` ne peut être appelé qu'une fois — le type système le garantit.

Ligne 2 : `peripherals.GPIO0` est **déplacé** dans `Output::new`. Après cette ligne, `peripherals.GPIO0` n'existe plus. Si tu essaies de le réutiliser ailleurs, ça ne compile pas.

C'est le cœur de Rust : **une valeur a un seul propriétaire**. Le déplacement (move) transfère la propriété. Pour un périphérique matériel, ça veut dire qu'il est impossible d'avoir deux morceaux de code qui pilotent la même broche sans le savoir.

Le `mut` de `let mut led` : par défaut tout est **immuable** en Rust. Tu dois demander explicitement le droit de modifier.

### Mise en œuvre

**1. Installe Rust** (si ce n'est pas fait) :

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**2. Ajoute la cible RISC-V** :

```bash
rustup target add riscv32imac-unknown-none-elf
```

C'est tout. Pas d'`espup`, pas de toolchain forkée — c'est le bénéfice d'avoir choisi un C6 plutôt qu'un ESP32 classique ou un S3.

**3. Installe les outils de flash** :

```bash
cargo install espflash
cargo install esp-generate
```

**4. Génère le projet** :

```bash
esp-generate --chip=esp32c6 mon-noeud
cd mon-noeud
```

L'outil te pose quelques questions (Embassy ? logging ? allocateur ?). Pour l'instant : **non à tout**. On ajoutera Embassy en partie 4, quand tu comprendras ce que ça change.

**5. Le code minimal** (`src/main.rs`) :

```rust
#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    gpio::{Level, Output, OutputConfig},
    main,
    time::{Duration, Instant},
};

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let mut led = Output::new(peripherals.GPIO8, Level::Low, OutputConfig::default());

    loop {
        led.toggle();
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(500) {}
    }
}
```

Remplace `GPIO8` par ta broche de LED réelle.

**6. Compile et flashe** :

```bash
cargo build --release
cargo run --release
```

`cargo run` est configuré par le template pour appeler `espflash` — il compile, flashe et ouvre le moniteur série d'un coup.

### Livrable

Une LED qui clignote à 1 Hz.

### Pièges

- **`--release` n'est pas optionnel.** En mode debug, le code Rust embarqué est souvent 10 fois plus gros et plus lent. Certains projets ne rentrent même pas en flash. Prends l'habitude dès maintenant.
- **Erreur `can't find crate for 'core'`** → tu as oublié `rustup target add`.
- **Erreur de port série** → sous Linux, ajoute ton utilisateur au groupe `dialout` puis reconnecte-toi.
- **Le fichier `.cargo/config.toml`** généré contient la cible par défaut et le runner espflash. Ne le supprime pas.

### Exercices

**1.1 ★ — Motif de battement de cœur**
Modifie le clignotement pour faire deux éclats courts (100 ms) suivis d'une pause d'une seconde. Boucle.

**1.2 ★★ — Provoque une erreur d'ownership** 💻
Ajoute délibérément une seconde ligne qui réutilise `peripherals.GPIO8` après l'avoir donné à `Output::new`. Compile.

Lis le message d'erreur **en entier**. Note le numéro d'erreur (`E0382` ou similaire), puis lance `rustc --explain E0382`. Écris en une phrase, avec tes mots, ce que le compilateur vient d'empêcher.

C'est l'exercice le plus important de cette partie : tu ne comprendras l'ownership qu'en le heurtant.

**1.3 ★★★ — Debug contre release**
Compile ton blink dans les deux modes. Compare la taille de `.text` avec `riscv32-esp-elf-size -A`. Note le facteur.

Puis lance `cargo bloat --release -n 10` et identifie ce qui occupe le plus de place dans un programme qui ne fait pourtant que clignoter. La réponse va t'apprendre quelque chose sur ce que `no_std` embarque quand même.

---

## Partie 2 — Les deux boutons

### Objectif

Lire l'état de deux boutons, avec anti-rebond.

### Pourquoi ici

C'est le périphérique le plus simple du projet. C'est le bon moment pour digérer les patterns d'`esp-hal` — ownership, typestate, configuration — avant d'affronter le SPI, où une erreur de compréhension coûte beaucoup plus cher à déboguer.

### Concepts Rust

#### Le typestate

Regarde ces deux types :

```rust
Output<'_>   // une broche configurée en sortie
Input<'_>    // une broche configurée en entrée
```

Ce sont des types **différents**. `Output` a une méthode `set_high()` mais pas `is_high()`. `Input` a `is_high()` mais pas `set_high()`.

Conséquence : essayer de lire une broche configurée en sortie **ne compile pas**. Ce n'est pas une vérification à l'exécution, c'est structurellement impossible.

C'est le **typestate pattern** : l'état de configuration du matériel est encodé dans le type. Ça surprend quand on vient de C, où tout est un `int`.

#### `Option<T>` — l'absence de valeur, sans `NULL`

Rust n'a pas de pointeur nul. Pour représenter « peut-être une valeur, peut-être rien » :

```rust
enum Option<T> {
    Some(T),
    None,
}
```

Tu ne peux pas utiliser la valeur sans avoir traité le cas `None`. Le compilateur t'y oblige. C'est la raison pour laquelle Rust n'a pas de « null pointer exception ».

#### `match` — le `switch` qui vérifie tout

```rust
match evenement {
    Evenement::BoutonA => compteur += 1,
    Evenement::BoutonB => compteur -= 1,
    Evenement::Rien => {}
}
```

Le `match` doit être **exhaustif** : si tu ajoutes une variante à ton enum et que tu oublies de la traiter quelque part, ça ne compile pas. C'est une des fonctionnalités les plus utiles du langage pour le code embarqué, où les états oubliés sont une source classique de bugs.

#### Structs et `impl`

Une `struct` regroupe des données. Un bloc `impl` lui attache des méthodes :

```rust
struct Debouncer {
    stable: bool,
    compteur: u8,
}

impl Debouncer {
    fn new() -> Self {
        Self { stable: false, compteur: 0 }
    }

    fn update(&mut self, brut: bool) -> bool {
        // ...
        self.stable
    }
}
```

`Self` (majuscule) est un alias pour le type. `self` (minuscule) est l'instance, comme `this` en C++.

`&mut self` signifie « j'emprunte l'instance en écriture ». C'est l'**emprunt** (borrowing) : tu accèdes à la valeur sans en prendre la propriété. Les trois formes que tu verras partout :

| Forme | Sens |
|---|---|
| `self` | je consomme l'instance (elle disparaît après) |
| `&self` | je la lis seulement |
| `&mut self` | je la lis et la modifie |

### Mise en œuvre

```rust
use esp_hal::gpio::{Input, InputConfig, Pull};

let config = InputConfig::default().with_pull(Pull::Up);
let bouton_a = Input::new(peripherals.GPIO9, config);
let bouton_b = Input::new(peripherals.GPIO10, config);
```

Avec un pull-up interne, le bouton relie la broche à la masse : **appuyé = niveau bas**. `bouton_a.is_low()` est donc vrai quand on appuie.

Un anti-rebond simple par comptage :

```rust
struct Debouncer {
    etat: bool,
    compteur: u8,
}

impl Debouncer {
    const SEUIL: u8 = 5;

    fn new() -> Self {
        Self { etat: false, compteur: 0 }
    }

    /// Appelée toutes les ~2 ms. Retourne true au front descendant.
    fn update(&mut self, appuye: bool) -> bool {
        if appuye == self.etat {
            self.compteur = 0;
            return false;
        }
        self.compteur += 1;
        if self.compteur >= Self::SEUIL {
            self.compteur = 0;
            self.etat = appuye;
            return appuye; // front d'appui uniquement
        }
        false
    }
}
```

### Livrable

Bouton A allume la LED, bouton B l'éteint, sans rebond parasite.

### Pièges

- **Le pull-up interne du C6 est faible** (~45 kΩ). Avec des fils longs ou un environnement bruité, ajoute un condensateur de 100 nF en parallèle du bouton.
- **N'utilise pas d'interruption tout de suite.** La scrutation avec anti-rebond suffit largement et t'évite les problèmes de partage de données entre contexte d'interruption et code principal — qui sont un sujet à part entière en Rust.

### Exercices

**2.1 ★ — Le second bouton**
Ajoute un `Debouncer` pour le bouton B. A allume la LED, B l'éteint.

**2.2 ★★ — Détection d'appui long**
Étends `Debouncer` pour distinguer trois événements : appui court (< 800 ms), appui long (≥ 800 ms), et relâchement. Retourne un `enum` plutôt qu'un `bool`.

Indice : tu auras besoin de mémoriser l'instant du front descendant. `Instant::now()` et `.elapsed()` sont tes outils.

**2.3 ★★★ — Ta première fonction générique** 💻
Écris une fonction qui accepte **n'importe quelle** broche d'entrée, pas seulement `Input` :

```rust
fn lire_actif_bas<P>(pin: &P) -> bool
where
    P: embedded_hal::digital::InputPin,
{
    // ...
}
```

Fais-la compiler. Puis écris un type factice `FauxBouton` qui implémente `InputPin` et retourne toujours `true`, et vérifie que ta fonction l'accepte aussi.

Tu viens de faire, en petit, ce que tu feras en grand avec `DrawTarget` en partie 3 : découpler ton code du matériel réel. C'est aussi la technique qui te permettra de tester ton code sur PC.

---

## Partie 3 — L'écran TFT

### Objectif

Afficher du texte, puis un compteur piloté par les boutons.

### Concepts Rust

#### Les traits — le concept central

Un **trait** est un contrat : un ensemble de méthodes qu'un type promet d'implémenter. C'est l'équivalent d'une interface, mais bien plus puissant.

```rust
trait Surface {
    fn dessiner_pixel(&mut self, x: i32, y: i32, couleur: Rgb565);
}
```

N'importe quel type peut implémenter `Surface`. Le code qui l'utilise n'a pas besoin de savoir lequel.

C'est exactement ce que fait `embedded-graphics` avec son trait **`DrawTarget`**. Ton pilote `mipidsi` l'implémente. Le simulateur desktop l'implémente aussi. Ton code de dessin fonctionne avec les deux, sans modification.

#### Les génériques

```rust
fn afficher_statut<D>(ecran: &mut D, texte: &str) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    // ...
}
```

Lecture : « cette fonction accepte n'importe quel type `D`, à condition que `D` implémente `DrawTarget` avec des couleurs Rgb565 ».

Contrairement aux templates C++, la contrainte (`where D: ...`) est vérifiée **à la déclaration**, pas à l'instanciation. Les messages d'erreur sont donc lisibles.

#### `Result` et l'opérateur `?`

```rust
enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

Rust n'a pas d'exceptions. Une fonction qui peut échouer retourne un `Result`. L'opérateur `?` propage l'erreur automatiquement :

```rust
fn dessiner(&mut self) -> Result<(), Error> {
    Text::new("Hello", Point::new(10, 20), style).draw(&mut self.ecran)?;
    Ok(())
}
```

Le `?` signifie : si c'est `Err`, retourne immédiatement cette erreur ; sinon, continue avec la valeur.

#### `unwrap()`, `expect()` et pourquoi s'en méfier

```rust
let x = resultat.unwrap();            // panique si Err
let y = resultat.expect("écran HS");  // panique avec un message
```

Pratique pendant le développement. Mais sur un nœud LoRa alimenté par batterie, une panique = redémarrage. Pour du code final, traite les erreurs explicitement.

**Règle** : `unwrap()` est acceptable à l'initialisation (si l'écran ne s'initialise pas, autant redémarrer), jamais dans la boucle principale.

### Mise en œuvre

Dépendances :

```toml
[dependencies]
embedded-graphics = "0.8"
mipidsi = "0.9"
embedded-hal-bus = "0.3"
```

Squelette :

```rust
use embedded_graphics::{
    mono_font::{ascii::FONT_9X15, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    text::Text,
};
use mipidsi::{models::ST7789, Builder};

// SPI, puis ExclusiveDevice, puis SpiInterface, puis Builder
let display = Builder::new(ST7789, di)
    .display_size(240, 240)
    .reset_pin(rst)
    .init(&mut delay)
    .unwrap();

let style = MonoTextStyle::new(&FONT_9X15, Rgb565::WHITE);
Text::new("Hello", Point::new(10, 30), style)
    .draw(&mut display)
    .unwrap();
```

**Le rétroéclairage n'est pas géré par `mipidsi`.** C'est une GPIO à toi :

```rust
let mut backlight = Output::new(peripherals.GPIO5, Level::High, OutputConfig::default());
```

### Livrable

Du texte à l'écran, et un compteur qui s'incrémente au bouton A.

### Pièges

Ce sont les trois qui coûtent le plus de temps, dans l'ordre de fréquence :

1. **Écran noir** → le rétroéclairage. Vérifie-le avant tout le reste.
2. **Bande de pixels parasites sur un bord** → mauvais offset. Joue sur `.display_offset(x, y)`. Une même puce ST7789 se décline en dalles de tailles différentes avec des décalages d'adressage variables.
3. **Couleurs inversées** → `.invert_colors(ColorInversion::Inverted)` ou `.color_order(ColorOrder::Bgr)`.

Le guide de dépannage de mipidsi couvre précisément ces cas : <https://github.com/almindor/mipidsi>

**Ne dessine pas en plein écran à chaque itération.** Redessine seulement les zones qui changent (un rectangle de fond, puis le nouveau texte). Sur ton C6 sans PSRAM, c'est ce qui te permet de te passer complètement de framebuffer.

### Exercices

**3.1 ★ — Un cadre et un titre**
Dessine une bordure de 2 px sur tout le pourtour, un titre centré en haut, et une ligne horizontale sous le titre. Utilise `Rectangle`, `Line` et `PrimitiveStyle`.

**3.2 ★★ — Rends ton rendu générique** 💻
Extrais ton code de dessin dans une fonction générique :

```rust
fn dessiner_statut<D>(cible: &mut D, compteur: u32) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    // ...
}
```

Puis prouve qu'elle est vraiment générique — **sans écran, sans dépendance externe** : écris un faux `DrawTarget` maison qui ne fait qu'enregistrer combien de pixels il a reçus, dans le style du `FauxBouton` de l'exercice 2.3. Appelle ta fonction avec ce faux type, affiche le compteur de pixels par `println!`.

Si ça compile et que le compteur n'est pas nul, tu viens de faire tourner exactement la même fonction contre deux implémentations totalement différentes de `DrawTarget` — ton vrai écran, et un type que tu as inventé en cinq lignes. C'est la preuve que la fonction ne connaît que le contrat, jamais l'implémentation.

*(Tu veux un vrai test qui tourne sur ton PC, avec une fenêtre desktop et le simulateur officiel ? C'est exactement l'exercice I.3, juste après cette partie — il ne te manque qu'une pièce, le duo `lib.rs`/`main.rs`, pour que `cargo test` fonctionne proprement sur cette cible embarquée.)*

**3.3 ★★★ — Redessin partiel mesuré**
Version A : efface tout l'écran et redessine tout, à chaque changement de compteur.
Version B : efface uniquement le rectangle du compteur, puis redessine le nombre.

Mesure le temps des deux avec `Instant::now()`. Affiche l'écart en millisecondes. Puis calcule combien d'octets partent sur le SPI dans chaque cas.

Ce chiffre justifie à lui seul de se passer de framebuffer sur le C6.

---

## Interlude — Découper le code : modules, lib/bin, bonnes pratiques

### Objectif

Faire sortir `Debouncer`, le dessin, et bientôt tout le reste, de `main.rs`. Poser une architecture qui tiendra jusqu'à la partie 9, au lieu d'un seul fichier de 2000 lignes.

### Pourquoi maintenant, précisément ici

Ton `main.rs` contient déjà l'init des périphériques, le `Debouncer`, le SPI, l'init de `mipidsi`, et une fonction de dessin. C'est le moment où ça commence à piquer — mais avant que ça devienne douloureux. En partie 4, tu vas ajouter des tâches Embassy ; en partie 5, une radio ; en partie 6, un parseur entier. **Découper maintenant coûte peu. Découper après coûte une refonte.**

C'est aussi le moment idéal parce que tu as déjà, sans le savoir, deux catégories de code bien différentes : le `Debouncer` (pur calcul, aucune dépendance matérielle) et l'init de l'écran (entièrement matériel). Cette distinction est le cœur de tout ce qui suit.

### Concepts Rust

#### Les modules : `mod`, fichiers, visibilité

Un module regroupe du code sous un espace de noms. Depuis l'édition 2018, plus besoin de `mod.rs` : un fichier `src/debounce.rs` **devient** le module `debounce` dès que tu le déclares dans `main.rs` :

```rust
// src/main.rs
mod debounce;   // charge src/debounce.rs

use debounce::Debouncer;
```

```rust
// src/debounce.rs
pub struct Debouncer {
    etat: bool,
    compteur: u8,
}

impl Debouncer {
    pub fn new() -> Self { /* ... */ }
    pub fn update(&mut self, appuye: bool) -> bool { /* ... */ }
}
```

**Tout est privé par défaut**, y compris les champs d'une struct. `pub` expose au monde extérieur au module ; sans lui, même `main.rs` ne peut pas lire `debouncer.etat` directement — il doit passer par tes méthodes.

C'est un choix de conception, pas juste une règle de syntaxe : ça t'empêche d'exposer accidentellement un détail d'implémentation que tu voudras changer plus tard. Regarde le `Debouncer` ci-dessus : `etat` et `compteur` restent privés, seules `new()` et `update()` sont publiques. Tu peux réécrire l'algorithme interne sans casser personne.

Il existe des niveaux intermédiaires : `pub(crate)` (visible dans tout le projet, mais pas à l'extérieur si un jour tu publies la crate) est souvent le bon choix par défaut — plus prudent que `pub`, moins contraignant que rien.

#### `use crate::`, `use super::`, les chemins

```rust
crate::debounce::Debouncer   // depuis la racine du crate
super::Debouncer             // depuis le module parent
self::Debouncer              // depuis le module courant
```

En pratique, tu écriras surtout `use crate::debounce::Debouncer;` en haut de chaque fichier qui en a besoin. Les chemins absolus (`crate::...`) vieillissent mieux que les relatifs quand tu réorganises des fichiers.

#### Le duo `lib.rs` + `main.rs` — le vrai levier

C'est le point le plus important de cet interlude, et celui qui explique comment les tests des parties 6 et 7 pourront tourner sur ton PC alors que le firmware entier est `no_std`.

Un projet Cargo peut produire **deux cibles à partir du même package** : une bibliothèque (`src/lib.rs`) et un binaire (`src/main.rs`). Le binaire peut dépendre de la bibliothèque comme n'importe quelle crate externe.

```
src/
├── lib.rs      # toute la logique : debounce, parsing, dedup...
└── main.rs     # juste l'assemblage matériel + boucle principale
```

```rust
// src/lib.rs
#![cfg_attr(not(test), no_std)]

pub mod debounce;
pub mod ui;
```

Le `#![cfg_attr(not(test), no_std)]` est la clé : *« applique `no_std` sauf quand on compile pour les tests »*. Résultat :

- `cargo build --release --target riscv32imac-unknown-none-elf` → compile `lib.rs` en `no_std`, embarqué sur ta puce
- `cargo test` (sans cible, donc pour ton PC) → compile la **même** `lib.rs` avec `std` disponible, et fait tourner tes `#[test]`

Tu n'écris pas deux versions de ton code. C'est le même fichier, compilé deux fois, avec deux jeux de contraintes.

```rust
// src/main.rs — reste no_std inconditionnellement, jamais testé sur host
#![no_std]
#![no_main]

use mon_projet::debounce::Debouncer;   // ta lib, importée comme une dépendance externe

#[esp_hal::main]
fn main() -> ! { /* assemblage matériel */ }
```

**`main.rs` ne doit contenir aucune logique testable.** Son unique rôle : initialiser le matériel, construire les objets définis dans `lib.rs`, et les brancher ensemble. Si tu te surprends à écrire un `if`/`match` non trivial dans `main.rs`, c'est probablement un signe qu'il devrait vivre dans `lib.rs`.

#### Workspace : quand un seul crate ne suffit plus

Un **workspace** regroupe plusieurs crates sous un `Cargo.toml` racine :

```toml
# Cargo.toml à la racine
[workspace]
members = ["firmware", "proto"]
```

Ça devient utile quand une partie du code doit être partagée entre plusieurs binaires distincts — par exemple si tu écris plus tard un outil PC qui décode aussi des paquets MeshCore, et que tu veux réutiliser exactement ton parseur de la partie 6 sans dupliquer le code.

**Pour l'instant, ne le fais pas.** Le duo `lib.rs`/`main.rs` dans un seul crate couvre tous tes besoins jusqu'à la partie 9. Un workspace prématuré ajoute de la friction (chemins relatifs entre crates, versions à synchroniser) sans bénéfice tant que tu n'as qu'un seul binaire cible.

#### Features : compiler différemment selon le contexte

```toml
# Cargo.toml
[features]
default = []
mock-display = []
```

```rust
#[cfg(feature = "mock-display")]
pub fn creer_ecran() -> FauxEcran { FauxEcran::new() }

#[cfg(not(feature = "mock-display"))]
pub fn creer_ecran() -> MonEcranReel { /* ... */ }
```

`#[cfg(...)]` inclut ou exclut du code **à la compilation**, sans coût à l'exécution. C'est ce mécanisme qui permettra, en partie 9, de compiler la même logique applicative contre un vrai `mipidsi::Display` ou contre `embedded-graphics-simulator`, comme tu l'as déjà entrevu à l'exercice 3.2.

#### Les `dev-dependencies` — des dépendances qui n'embarquent jamais

```toml
[dev-dependencies]
embedded-graphics-simulator = "0.8"
```

Une crate déclarée ici n'est compilée que pour `cargo test` et `cargo bench` — **jamais** pour `cargo build --release` visant ton C6. C'est le bon endroit pour `embedded-graphics-simulator` : tu veux le simulateur pour tes tests sur PC, mais il n'a évidemment aucun sens sur la cible embarquée (il dépend de SDL2, qui suppose un OS).

### Mise en œuvre — le découpage de ce que tu as déjà

Structure cible à ce stade du tutoriel :

```
src/
├── lib.rs        # #![cfg_attr(not(test), no_std)]
├── debounce.rs    # Debouncer — pur, testable
├── ui.rs          # fonctions de dessin génériques (exercice 3.2)
└── main.rs        # #![no_std] #![no_main] — assemblage uniquement
```

```toml
# Cargo.toml — ajoute le nom de la lib si besoin
[lib]
name = "mon_projet"
```

`main.rs` importe depuis `mon_projet::` comme n'importe quelle dépendance :

```rust
use mon_projet::{debounce::Debouncer, ui::dessiner_statut};
```

### Bonnes pratiques — la règle à retenir pour tout le reste du tutoriel

C'est la version anticipée de la séparation `proto/` contre `hw/` que tu retrouveras formalisée en partie 9. Trois règles suffisent à la maintenir :

1. **`lib.rs` et ses modules ne touchent jamais un type d'`esp-hal` directement.** Un module qui décode des paquets ou débounce un bouton n'a besoin de connaître ni `Input`, ni `Spi`, ni `Output`. S'il en a besoin, c'est qu'il fait deux métiers à la fois — sépare.
2. **`main.rs` ne contient aucun algorithme.** Juste de l'assemblage : construire les objets, les brancher, lancer la boucle. Si tu écris une boucle de calcul non triviale dans `main.rs`, extrais-la.
3. **Chaque module de `lib.rs` porte ses propres tests**, dans un `#[cfg(test)] mod tests` en bas du fichier. Un module sans test à ce stade du projet est un module que tu n'as pas encore pris le temps de rendre testable — souvent parce qu'il mélange encore logique et matériel.

### Livrable

Le même comportement qu'à la fin de la partie 3, mais réparti en modules, avec `cargo test` qui fait tourner au moins les tests du `Debouncer` sur ton PC — sans flasher.

### Pièges

- **`#![no_std]` ne peut pas être conditionnel dans `main.rs`.** L'attribut `cfg_attr` fonctionne dans `lib.rs` parce que la lib est recompilée pour deux cibles différentes (host pour les tests, C6 pour le firmware). `main.rs`, lui, n'est jamais compilé pour host — il reste `#![no_std]` sans condition, pour toujours.
- **N'importe pas `esp_hal` dans `lib.rs`** si tu veux garder la possibilité de tester sur host. Dès qu'une dépendance matérielle s'invite dans un module de la lib, `cargo test` échoue à compiler — le message ressemblera à une erreur de cible, alors que la vraie cause est une dépendance mal placée.
- **Ne crée pas un workspace avant d'en avoir besoin.** Le signe qu'il en faut un : un second binaire réel qui doit réutiliser ton code, pas juste « ça fait plus propre ».

### Exercices

**I.1 ★ — Extraire le `Debouncer`**
Déplace `Debouncer` dans `src/debounce.rs`, exposé en `pub`. Ajoute `mod debounce;` dans `main.rs`. Ajoute deux tests unitaires dans un `#[cfg(test)] mod tests` en bas du fichier. Lance `cargo test` : ils doivent passer sans toucher au matériel.

**I.2 ★★ — Le duo `lib.rs` / `main.rs`** 💻
Crée `src/lib.rs` avec `#![cfg_attr(not(test), no_std)]`, déplace-y `mod debounce;`. Adapte `Cargo.toml` pour déclarer la cible `[lib]`. `main.rs` doit maintenant importer `Debouncer` depuis ta propre crate (`use mon_projet::debounce::Debouncer;`) plutôt que par un `mod` local.

Vérifie que les deux commandes suivantes réussissent toutes les deux, depuis la racine du projet :
```bash
cargo test
cargo build --release --target riscv32imac-unknown-none-elf
```
Si l'une casse l'autre, c'est qu'une dépendance matérielle s'est glissée dans `lib.rs`.

**I.3 ★★★ — `ui.rs` avec test sur simulateur** 💻
Déplace ta fonction générique de dessin (exercice 3.2) dans `src/ui.rs`, dans `lib.rs`. Ajoute `embedded-graphics-simulator` en `dev-dependencies`. Écris un test qui instancie un `SimulatorDisplay`, appelle ta fonction de dessin, et vérifie au moins un pixel avec `display.get_pixel(...)`.

Confirme que ce test tourne avec `cargo test` mais que la crate `embedded-graphics-simulator` **n'apparaît pas** dans `cargo tree --target riscv32imac-unknown-none-elf`. C'est la preuve que les `dev-dependencies` n'ont jamais touché ton firmware.

---

## Partie 4 — Passage en async avec Embassy

### Objectif

Restructurer le programme en tâches concurrentes.

### Pourquoi maintenant

Une boucle bloquante ne tiendra pas quand la radio arrivera. Tu dois pouvoir attendre un paquet LoRa **pendant** que l'écran se rafraîchit et que les boutons sont scrutés. Sans OS et sans threads, la réponse en Rust embarqué s'appelle **async**.

Fais ce saut maintenant, sur du code que tu maîtrises, plutôt qu'en même temps que la radio.

### Concepts Rust

#### `async` / `await`

```rust
async fn clignoter(mut led: Output<'static>) {
    loop {
        led.toggle();
        Timer::after(Duration::from_millis(500)).await;
    }
}
```

Le `.await` signifie : « je n'ai rien à faire jusqu'à ce que ce soit prêt, **rends la main** ».

Différence essentielle avec un délai bloquant : pendant ce demi-seconde, l'exécuteur fait tourner les autres tâches. Et s'il n'y a rien à faire, il met le CPU en veille — ce qui compte énormément sur batterie.

#### Ce qu'est réellement une tâche async

Une fonction `async` ne s'exécute pas quand tu l'appelles : elle retourne un **Future**, un objet qui décrit un travail à faire. L'**exécuteur** (fourni par Embassy) reprend ces futures et les fait progresser.

Concrètement, le compilateur transforme ta fonction async en une machine à états. Il n'y a **pas de pile séparée par tâche**, contrairement aux threads d'un RTOS. C'est pour ça que ça tient dans quelques centaines d'octets par tâche au lieu de quelques kilooctets.

C'est le point clé pour toi : sur 512 Ko, tu peux te permettre dix tâches async ; dix threads FreeRTOS te coûteraient beaucoup plus cher.

#### Le `'static` qui va t'embêter

```rust
#[embassy_executor::task]
async fn tache_ecran(display: MonDisplay) { }
```

Les tâches Embassy doivent avoir une durée de vie `'static` : leurs données doivent vivre aussi longtemps que le programme.

C'est logique — une tâche peut tourner indéfiniment, donc elle ne peut pas emprunter quelque chose qui disparaîtra. Mais c'est la source d'erreur numéro un des débutants avec Embassy.

La solution habituelle est la macro `static_cell::make_static!` ou les `StaticCell`, qui allouent en mémoire statique et te rendent une référence `'static`.

#### Les canaux — communiquer entre tâches

```rust
use embassy_sync::channel::Channel;

static EVENEMENTS: Channel<CriticalSectionRawMutex, Evenement, 8> = Channel::new();

// producteur
EVENEMENTS.send(Evenement::BoutonA).await;

// consommateur
let ev = EVENEMENTS.receive().await;
```

Le `8` est la capacité, fixée à la compilation — pas d'allocation dynamique. C'est un **const generic**, un paramètre de type qui est un nombre.

**Le principe à retenir** : ne partage pas de données mutables entre tâches, fais circuler des messages. Ça évite toute une catégorie de bugs de concurrence, et Rust rend l'alternative pénible exprès.

### Mise en œuvre

```toml
[dependencies]
embassy-executor = "0.9"
embassy-time = "0.5"
embassy-sync = "0.7"
esp-rtos = { version = "0.2", features = ["esp32c6", "embassy"] }
static_cell = "2"
```

Structure cible :

```rust
#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let peripherals = esp_hal::init(config);
    // ... init écran, boutons ...

    spawner.spawn(tache_boutons(bouton_a, bouton_b)).unwrap();
    spawner.spawn(tache_ecran(display)).unwrap();
}

#[embassy_executor::task]
async fn tache_boutons(a: Input<'static>, b: Input<'static>) {
    let mut deb_a = Debouncer::new();
    loop {
        if deb_a.update(a.is_low()) {
            EVENEMENTS.send(Evenement::BoutonA).await;
        }
        Timer::after(Duration::from_millis(2)).await;
    }
}

#[embassy_executor::task]
async fn tache_ecran(mut display: MonDisplay) {
    let mut compteur = 0u32;
    loop {
        let ev = EVENEMENTS.receive().await;
        match ev {
            Evenement::BoutonA => compteur += 1,
            Evenement::BoutonB => compteur = 0,
        }
        // redessiner seulement la zone du compteur
    }
}
```

### Livrable

Le même comportement qu'en partie 3, mais en tâches séparées qui ne se bloquent pas mutuellement.

### Pièges

- **`esp-rtos` a remplacé `esp-hal-embassy`.** Beaucoup de tutoriels en ligne sont périmés sur ce point. L'ordonnanceur en a été extrait et amélioré, notamment parce que les blobs radio d'Espressif en ont besoin.
- **Les versions d'Embassy doivent être cohérentes** avec celles qu'attend `esp-rtos`. Si tu as des erreurs de traits incompatibles, c'est presque toujours ça. Vérifie sur <https://docs.espressif.com/projects/rust/>.
- **Ne mets jamais de boucle bloquante dans une tâche async.** Un `while start.elapsed() < ...` bloque tout l'exécuteur. Utilise `Timer::after(...).await`.

### Exercices

**4.1 ★ — Une troisième tâche**
Ajoute une tâche indépendante qui fait clignoter la LED à 1 Hz, sans jamais perturber l'écran ni les boutons.

**4.2 ★★ — Sature le canal**
Réduis la capacité de ton canal à 1. Dans la tâche boutons, remplace `send().await` par `try_send()` et affiche à l'écran combien d'événements sont perdus.

Puis ralentis délibérément la tâche écran (ajoute un `Timer::after(500ms)` dans sa boucle) et martèle les boutons. Observe.

Question à laquelle tu dois savoir répondre ensuite : dans quel cas veux-tu `send().await` (qui attend qu'il y ait de la place) et dans quel cas `try_send()` (qui abandonne) ? Pour des événements de boutons, lequel est correct ?

**4.3 ★★★ — Casse l'exécuteur, puis répare-le**
Remplace un `Timer::after(...).await` par une boucle d'attente bloquante :

```rust
let start = Instant::now();
while start.elapsed() < Duration::from_millis(500) {}
```

Compile, flashe, observe : **tout** le système se fige, y compris les autres tâches.

Explique par écrit pourquoi. Puis corrige. Cette expérience vaut dix pages de documentation sur la différence entre concurrence coopérative et préemptive — et elle t'évitera de recréer ce bug quand la radio arrivera.

---

## Partie 5 — Réception LoRa brute ⭐

### Objectif

Afficher à l'écran, en hexadécimal, les octets bruts des paquets qui circulent sur le maillage local.

### Pourquoi c'est LE jalon

C'est le moment où tu prouves que ta configuration RF est juste. Tant que tu n'as pas vu un seul octet arriver, tout doute est possible : câblage, paramètres, alimentation, antenne.

Une fois cette étape franchie, **tout ce qui suit est du logiciel pur**. Plus jamais de doute sur la radio. C'est pour ça qu'elle mérite tout le soin que tu peux y mettre.

### Concepts Rust

#### Les slices : `&[u8]` et `&mut [u8]`

Un **slice** est une vue sur une zone mémoire contiguë : un pointeur et une longueur. C'est l'équivalent sûr du couple `(uint8_t*, size_t)` en C.

```rust
let mut buffer = [0u8; 255];        // tableau de taille fixe, sur la pile
let vue: &mut [u8] = &mut buffer;   // slice mutable sur ce tableau
let debut: &[u8] = &buffer[0..10];  // slice en lecture des 10 premiers octets
```

La différence avec C est décisive : **la longueur voyage avec le pointeur**. Un dépassement de tableau provoque une panique explicite au lieu d'une corruption silencieuse. Sur un décodeur de paquets réseau, qui traite des données venues de l'extérieur, c'est exactement la garantie que tu veux.

#### Tableau, `heapless::Vec`, `Vec` : lequel choisir

| Type | Taille | Mémoire | Disponible en `no_std` |
|---|---|---|---|
| `[u8; 255]` | fixe à la compilation | pile | ✅ toujours |
| `heapless::Vec<u8, 255>` | variable, plafond fixe | pile | ✅ toujours |
| `Vec<u8>` | variable, illimitée | tas | ❌ nécessite un allocateur |

Pour ce projet, tu utiliseras les deux premiers exclusivement. Le `255` de `heapless::Vec<u8, 255>` est un **const generic** : la capacité fait partie du type, donc elle est connue à la compilation et rien n'est alloué dynamiquement.

#### Trait bounds multiples

La signature de `lora-phy` ressemble à ça :

```rust
impl<RK, DLY> LoRa<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{ }
```

Lecture : « `LoRa` fonctionne avec n'importe quelle puce radio `RK` qui implémente `RadioKind`, et n'importe quelle source de délai `DLY` ». C'est ce qui permet à la même crate de piloter une SX1262, une SX1276 ou une LLCC68.

Tu vas beaucoup rencontrer ce motif. Quand une erreur de compilation te dit « the trait bound `X: Y` is not satisfied », elle te dit qu'un de ces contrats n'est pas rempli — presque toujours parce qu'une version de crate ne correspond pas.

### Mise en œuvre

```toml
[dependencies]
lora-phy = "3"
```

Squelette de réception (vérifie l'API exacte sur <https://docs.rs/lora-phy>, elle bouge) :

```rust
// 1. Le bus SPI vers la radio, plus les broches de contrôle
//    NSS (chip select), RESET, BUSY, DIO1

// 2. Créer l'interface puis l'objet LoRa
let mut lora = LoRa::new(Sx126x::new(spi_device, iv, config), false, delay)
    .await
    .unwrap();

// 3. Les paramètres relevés en partie 0 — c'est ici que tout se joue
let modulation = lora.create_modulation_params(
    SpreadingFactor::_11,
    Bandwidth::_250KHz,
    CodingRate::_4_5,
    869_525_000,
).unwrap();

let rx_params = lora.create_rx_packet_params(
    16,     // longueur de préambule
    false,  // en-tête implicite ?
    255,    // taille max de payload
    true,   // CRC activé
    false,
    &modulation,
).unwrap();

// 4. Boucle de réception
let mut buffer = [0u8; 255];
lora.prepare_for_rx(RxMode::Continuous, &modulation, &rx_params).await.unwrap();

loop {
    match lora.rx(&rx_params, &mut buffer).await {
        Ok((len, status)) => {
            // buffer[..len] contient le paquet
            // status.rssi, status.snr
            PAQUETS.send(/* ... */).await;
        }
        Err(e) => {
            // journaliser, mais surtout ne pas paniquer
        }
    }
}
```

Affiche sur ton écran : la longueur, le RSSI, le SNR, et les 16 premiers octets en hexa.

### Livrable

Des paquets réels du maillage local, affichés en hexadécimal sur ton TFT, avec leur puissance de réception.

### Pièges

**Si tu n'entends rien — l'ordre de vérification :**

1. **Le sync word.** C'est le suspect numéro un. S'il ne correspond pas, le modem rejette tout silencieusement, comme si l'air était vide.
2. **La fréquence.** Une erreur de quelques kilohertz passe encore ; quelques centaines, non.
3. **SF, BW, CR.** Doivent correspondre exactement.
4. **L'antenne.** Branchée ? Bonne bande ?
5. **La broche BUSY.** Sur SX126x, elle est obligatoire. Si tu ne la câbles pas ou la déclares mal, le pilote attend indéfiniment.
6. **DIO1.** C'est l'interruption de fin de réception. Mal câblée, tu ne recevras jamais de notification.

**Astuce de diagnostic** : mets temporairement le sync word en mode « public » LoRaWAN (0x34) et vois si tu captes autre chose. Si tu captes du trafic quelconque, ton matériel fonctionne et le problème est dans tes paramètres MeshCore.

**Ne panique pas sur erreur radio.** Un CRC invalide est normal et fréquent. Traite l'erreur, ignore le paquet, continue.

### Exercices

**5.1 ★ — Qualité de réception**
Affiche pour chaque paquet reçu : la longueur, le RSSI et le SNR. Ajoute un indicateur visuel simple (barres ou couleur) selon la force du signal.

**5.2 ★★ — Débit du maillage**
Calcule et affiche un compteur glissant de paquets par minute. Garde les horodatages des paquets des 60 dernières secondes dans un `heapless::Deque` et purge les plus anciens.

Laisse tourner une heure. Le chiffre que tu obtiens est la charge réelle de ton maillage local — utile pour dimensionner ton cache en partie 7.

**5.3 ★★★ — Scanner de sync word**
Écris un mode diagnostic qui parcourt une liste de sync words candidats, reste 30 secondes sur chacun, et affiche combien de paquets valides il a captés pour chaque valeur.

C'est l'outil que tu voudras avoir le jour où tu n'entends rien et où tu ne sais pas si le problème vient de ta configuration ou de ton câblage. Écris-le maintenant, pendant que ça marche.

---

## Partie 6 — Décoder l'en-tête MeshCore

### Objectif

Transformer les octets bruts en information lisible : « FLOOD · 3 sauts · type 0x01 · 47 o ».

### La source de vérité

Le format est documenté ici — **lis-le avant de coder** :

- <https://github.com/meshcore-dev/MeshCore/blob/main/docs/packet_format.md>
- <https://github.com/meshcore-dev/MeshCore/blob/main/src/Packet.h>
- <https://deepwiki.com/meshcore-dev/MeshCore/7-network-protocol>

Ce que l'on sait de la structure générale : un en-tête d'**un octet** encodant trois champs par bit-packing, suivi d'un octet `path_length`, du chemin lui-même, puis du payload. Le tout plafonné à 255 octets.

L'encodage de `path_length` est `(hash_size - 1) << 6 | hash_count` — donc les deux bits de poids fort donnent la taille des hash, les six bits restants leur nombre.

Les types de route incluent `ROUTE_TYPE_FLOOD` (0x01) et `ROUTE_TYPE_DIRECT` (0x02), plus leurs variantes transport. Le type de payload qui t'intéresse est `PAYLOAD_TYPE_RAW_CUSTOM` (0x0F).

**Je ne te donne pas les positions de bits exactes de l'en-tête** : elles peuvent changer, et tu dois prendre l'habitude de lire la source. C'est aussi le meilleur exercice de la partie.

### Concepts Rust

#### Manipulation de bits

```rust
let route_type = header & 0b0000_0011;
let hash_size  = (path_length >> 6) + 1;
let hash_count = path_length & 0b0011_1111;
```

Les littéraux binaires avec `_` comme séparateur rendent le masquage lisible. Utilise-les.

⚠️ **Rust panique en cas de dépassement arithmétique en mode debug** et boucle silencieusement en release. Pour du parsing réseau, sois explicite :

```rust
let total = a.checked_add(b).ok_or(ParseError::Overflow)?;
let reste = longueur.saturating_sub(consomme);
```

#### Les enums avec données — le vrai super-pouvoir

En C, un enum est un entier. En Rust, chaque variante peut porter ses propres données :

```rust
enum RouteType {
    Flood { hops: u8 },
    Direct { next_hop: u8, restants: u8 },
    ZeroHop,
}
```

Combiné au `match` exhaustif, ça veut dire qu'il est **impossible** de lire `next_hop` sur un paquet flood : le compilateur t'oblige à traiter le cas.

C'est la fonctionnalité qui remplace le plus avantageusement les `union` + tag de C, et elle est parfaite pour du décodage de protocole.

#### Un type d'erreur propre

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
enum ParseError {
    TropCourt { attendu: usize, recu: usize },
    RouteInconnue(u8),
    CheminIncoherent,
}
```

Le `#[derive(...)]` génère automatiquement du code : `Debug` pour l'affichage de diagnostic, `PartialEq` pour la comparaison dans les tests, `Copy` pour que le type se copie au lieu de se déplacer.

#### `TryFrom` — la conversion qui peut échouer

```rust
impl TryFrom<&[u8]> for Packet {
    type Error = ParseError;

    fn try_from(octets: &[u8]) -> Result<Self, Self::Error> {
        if octets.len() < 2 {
            return Err(ParseError::TropCourt { attendu: 2, recu: octets.len() });
        }
        // ...
    }
}
```

Implémenter un trait standard plutôt qu'inventer `Packet::parse()` te fait bénéficier de tout l'écosystème : `octets.try_into()?` fonctionnera partout.

#### Les tests unitaires — ton meilleur allié ici

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_flood_minimal() {
        let brut = [0x01, 0x00, 0x0F, 0xAA, 0xBB];
        let p = Packet::try_from(&brut[..]).unwrap();
        assert_eq!(p.payload_type, 0x0F);
    }

    #[test]
    fn rejette_trop_court() {
        assert!(Packet::try_from(&[0x01][..]).is_err());
    }
}
```

`#[cfg(test)]` signifie « ce module n'est compilé que pour les tests ». Il ne prend aucune place dans ton firmware.

**Le point important** : `cargo test` compile ces tests **pour ton PC**, pas pour le C6. Tu testes ton décodeur à la vitesse de ton ordinateur, sans flasher, avec un vrai débogueur si besoin.

Pour que ça marche, ton module de parsing ne doit dépendre d'aucun matériel — que du `core` et des slices. C'est un excellent réflexe d'architecture : **la logique pure d'un côté, le matériel de l'autre**.

Capture quelques paquets réels en partie 5, note-les en hexa, et fais-en des vecteurs de test. Tu construis ton décodeur entièrement sur PC, puis tu le flashes une fois qu'il passe.

### Livrable

Ton écran affiche l'interprétation des paquets, plus une suite de tests verte sur ton PC.

### Pièges

- **Ne fais jamais confiance aux données reçues.** Un `path_length` corrompu peut annoncer 60 hash de 4 octets dans un paquet de 20 octets. Vérifie systématiquement que tu as assez d'octets avant de découper.
- **Le format évolue.** Les firmwares 1.12.0 et antérieurs ne géraient que des hash de chemin sur 1 octet. Une spec V2 est annoncée.
- **Utilise `get()` plutôt que l'indexation** quand la longueur est incertaine : `octets.get(3)` retourne `Option<&u8>` au lieu de paniquer.

### Exercices

**6.1 ★ — Type de route** 💻
Décode le route type depuis l'octet d'en-tête et affiche-le en clair : FLOOD, DIRECT, ou la valeur brute si tu ne la reconnais pas.

**6.2 ★★ — Un parseur robuste avec ses tests** 💻
Implémente `TryFrom<&[u8]> for Packet` avec vérification complète des bornes. Puis écris au minimum cinq tests :

1. un paquet flood minimal valide
2. un paquet avec chemin non vide
3. une entrée trop courte (2 octets)
4. un `path_length` qui annonce plus d'octets que le paquet n'en contient
5. un paquet de longueur exactement égale à la limite

Capture de vrais paquets en partie 5, note-les en hexadécimal, et sers-t'en comme vecteurs de test. Tout doit passer avec `cargo test` sur ton PC, sans flasher.

**6.3 ★★★ — Ton parseur ne doit jamais paniquer** 💻
Écris un test qui génère 100 000 tableaux d'octets pseudo-aléatoires de longueurs variées (0 à 255) et les passe à ton parseur. Il doit retourner `Ok` ou `Err`, mais **jamais paniquer**.

```rust
#[test]
fn jamais_de_panique() {
    let mut graine = 12345u32;
    for _ in 0..100_000 {
        // générateur congruentiel simple, remplis un buffer
        // let _ = Packet::try_from(&buf[..len]);
    }
}
```

Si ce test panique, tu viens de trouver un bug que des données réelles corrompues auraient déclenché tôt ou tard — sur un nœud alimenté par batterie, à 800 m de chez toi.

C'est du fuzzing rudimentaire, et c'est exactement le genre de test que justifie du code qui traite des données venues de l'extérieur.

---

## Partie 7 — Déduplication

### Objectif

Ne traiter chaque message qu'une fois, alors que le flooding te le livre en plusieurs exemplaires.

### Concepts Rust

#### Crypto en `no_std`

```toml
sha2 = { version = "0.10", default-features = false }
```

Le `default-features = false` est **essentiel** : par défaut, `sha2` active `std`. C'est exactement le motif dont on a parlé — feature `std` activée par défaut, désactivable.

```rust
use sha2::{Sha256, Digest};

let mut hasher = Sha256::new();
hasher.update(&[payload_type]);
hasher.update(payload);
let hash = hasher.finalize();
```

**Bonne nouvelle** : SHA-256 est une fonction de hachage pure, sans aléa. Tu ne rencontreras **pas** le problème `getrandom` ici. Il ne t'attendrait que si tu implémentais la crypto d'identité (Ed25519, ECDH), ce que ton rôle Companion en `0x0F` te dispense de faire.

#### `heapless::Deque` et les const generics

```toml
heapless = "0.9"
```

```rust
use heapless::Deque;

struct DedupCache {
    vus: Deque<[u8; 8], 32>,
}
```

Tu stockes 32 empreintes tronquées à 8 octets, soit 256 octets de RAM. Fixé à la compilation, aucune allocation.

**Dimensionnement** : sur un maillage actif, un paquet peut te revenir par 3 à 5 chemins. Une fenêtre de 32 entrées couvre confortablement. Si tu vois des doublons passer, augmente.

#### Les itérateurs

```rust
fn deja_vu(&self, hash: &[u8; 8]) -> bool {
    self.vus.iter().any(|h| h == hash)
}
```

`.iter()` produit un itérateur, `.any()` le consomme en s'arrêtant au premier succès. Le `|h| ...` est une **closure** — une fonction anonyme, comme une lambda C++.

Les itérateurs Rust sont **à coût nul** : ce code compile vers la même boucle que tu aurais écrite à la main. Tu peux les utiliser librement en embarqué.

#### Insertion avec éviction

```rust
fn inserer(&mut self, hash: [u8; 8]) {
    if self.vus.is_full() {
        self.vus.pop_front();
    }
    let _ = self.vus.push_back(hash);
}
```

Le `let _ =` dit explicitement « j'ignore ce `Result` ». Rust émet un avertissement si tu ignores un `Result` silencieusement — ici c'est volontaire, puisqu'on vient de faire de la place.

### Livrable

Un compteur à l'écran : « paquets reçus : 147 · uniques : 38 ». Tu verras concrètement le coût du flooding.

### Pièges

- **Hash tronqué = collisions possibles.** 8 octets suffisent largement à cette échelle, mais comprends le compromis que tu fais.
- **Le cache est volatil.** Après un redémarrage, tu retraiteras d'anciens messages. Acceptable ici ; c'est la RTC SRAM qui résoudrait ça (annexe A).

### Exercices

**7.1 ★ — Le cache d'empreintes** 💻
Implémente `DedupCache` avec `heapless::Deque`, ses méthodes `deja_vu` et `inserer`, et trois tests unitaires (insertion, détection de doublon, éviction quand plein).

**7.2 ★★ — Mesure le coût du flooding**
Affiche en permanence « reçus : N · uniques : M · doublons : N-M ». Laisse tourner une heure et note le ratio.

Ce chiffre te dit combien de fois, en moyenne, chaque message t'arrive. C'est une mesure directe de la redondance de ton maillage.

**7.3 ★★★ — Dimensionnement et collisions** 💻
Rends ton cache générique sur sa capacité :

```rust
struct DedupCache<const N: usize> {
    vus: Deque<[u8; 8], N>,
}
```

Puis réponds par le calcul : avec des empreintes tronquées à 8 octets et le débit mesuré en 5.2, quelle est la probabilité d'une collision (deux messages différents avec la même empreinte) sur une journée ? Sur un an ?

Indice : c'est le paradoxe des anniversaires. Vérifie ensuite si 4 octets suffiraient — et ce que tu gagnerais en RAM.

---

## Partie 8 — Émission en `RAW_CUSTOM` ⭐

### Objectif

Construire et transmettre un paquet FLOOD avec un payload `0x0F` contenant tes propres données.

**Prérequis** : un second nœud pour vérifier la réception. C'est la seule partie qui l'exige.

### Concepts Rust

#### `serde` et `postcard` — la sérialisation

```toml
serde = { version = "1", default-features = false, features = ["derive"] }
postcard = { version = "1", default-features = false }
```

```rust
#[derive(Serialize, Deserialize)]
struct MonMessage {
    seq: u32,
    temperature: f32,
    texte: heapless::String<32>,
}

let mut buf = [0u8; 96];
let octets = postcard::to_slice(&msg, &mut buf).unwrap();
```

`#[derive(Serialize)]` est une **macro procédurale** : elle génère le code de sérialisation à la compilation, en lisant la définition de ta struct.

C'est très différent d'une macro C (substitution de texte) : elle manipule l'arbre syntaxique et produit du vrai code Rust, vérifié par le compilateur.

`postcard` produit un format binaire compact — typiquement 3 à 5 fois plus petit que du JSON. Sur LoRa, où chaque octet coûte du temps d'antenne, ça compte directement.

#### Un rappel sur `f32`

Le C6 **n'a pas d'unité de calcul flottant matérielle**. Tout flottant est émulé en logiciel, ce qui est lent et gonfle le binaire. Pour de la télémétrie, préfère des entiers mis à l'échelle :

```rust
temperature_centidegres: i16,   // 2350 = 23,50 °C
```

### Mise en œuvre

**1. Construis le paquet** : en-tête FLOOD, `path_length` = 0 (chemin vide, tu es l'origine), payload type `0x0F`, puis tes octets postcard.

**2. Écoute avant d'émettre (CSMA)** :

```rust
// Vérifier que le canal est libre avant de transmettre
let rssi = lora.get_rssi().await?;
if rssi > SEUIL_CANAL_OCCUPE {
    Timer::after(Duration::from_millis(backoff_aleatoire)).await;
    continue;
}
```

**3. Émets**, puis repasse immédiatement en réception.

**4. Respecte le rapport cyclique.** En EU868, la limite est de **1 %**. Un paquet de 50 octets en SF11 dure environ une seconde — tu dois donc te taire pendant environ 100 secondes après.

Implémente un compteur de temps d'antenne et refuse d'émettre s'il est dépassé. Ce n'est pas une politesse, c'est une obligation légale.

### Livrable

Deux de tes nœuds échangent un message à travers les répéteurs des autres.

### Pièges

**À vérifier en tout premier** : les répéteurs relaient-ils réellement les payloads `0x0F` ?

La décision de routage se prend sur le *route type*, pas sur le *payload type*, donc en théorie oui — et le commentaire dans `Packet.h` dit explicitement que ce type existe pour les applications à payload personnalisé. Mais **teste-le avant de construire dessus** : flashe MeshCore en mode Repeater sur ta seconde carte, émets un `0x0F` depuis ton nœud Rust, vérifie qu'il ressort.

Tu as la réponse en une soirée.

**Tu es invité sur le réseau des autres.** Un nœud qui émet sans discipline sature l'air pour tout le monde. Commence avec des intervalles très espacés (une émission par minute maximum) le temps de valider ton code.

### Exercices

**8.1 ★ — Mesure ta charge utile** 💻
Sérialise une structure de télémétrie avec `postcard` et affiche sa taille en octets. Compare avec l'équivalent en JSON écrit à la main.

Puis remplace tes `f32` par des entiers mis à l'échelle et mesure à nouveau. Note l'écart, en octets **et** en temps d'antenne (avec la formule de l'exercice 0.2).

**8.2 ★★ — Le garde-fou réglementaire**
Implémente un compteur de rapport cyclique : il additionne le temps d'antenne de chaque émission sur une fenêtre glissante d'une heure, et **refuse** toute émission qui ferait dépasser 1 %.

Affiche le budget restant à l'écran, en pourcentage. Ce n'est pas une option : c'est une obligation légale, et c'est aussi ce qui t'évitera de saturer le réseau des autres pendant tes tests.

**8.3 ★★★ — CSMA avec backoff exponentiel**
Avant d'émettre, écoute le canal. S'il est occupé, attends un délai aléatoire, puis réessaie en doublant la fenêtre d'attente à chaque échec, jusqu'à un plafond.

Deux difficultés réelles :
- **Générer de l'aléa en `no_std`.** Tu ne peux pas utiliser `getrandom`. Options : le générateur matériel du C6 (`esp_hal::rng::Rng`), ou un générateur pseudo-aléatoire simple amorcé par le bruit du RSSI.
- **Choisir le seuil de « canal occupé »**. Mesure le RSSI ambiant sur plusieurs minutes pour le calibrer.

---

## Partie 9 — L'application

### Objectif

Assembler tout ça en un appareil utilisable, piloté par deux boutons.

### Concepts Rust

#### La machine à états par enum

C'est le motif qui va structurer ton UI :

```rust
enum Ecran {
    Statut,
    ListeMessages { index: usize },
    Detail { index: usize },
    Config { champ: ChampConfig },
}

impl Ecran {
    fn suivant(self, ev: Evenement) -> Self {
        match (self, ev) {
            (Ecran::Statut, Evenement::BoutonA) => Ecran::ListeMessages { index: 0 },
            (Ecran::ListeMessages { index }, Evenement::BoutonA) => {
                Ecran::ListeMessages { index: index + 1 }
            }
            (Ecran::ListeMessages { index }, Evenement::BoutonB) => Ecran::Detail { index },
            (etat, _) => etat,
        }
    }
}
```

Le `match` sur un **tuple** `(état, événement)` donne une table de transition lisible d'un coup d'œil. Et l'exhaustivité te garantit qu'aucune combinaison n'est oubliée.

Le `self` (sans `&`) consomme l'ancien état et retourne le nouveau — un état ne peut pas exister en double.

#### UI à deux boutons : la convention qui marche

Avec seulement deux boutons, la répartition classique est :

- **Bouton A** = naviguer (avancer dans la liste, incrémenter)
- **Bouton B court** = valider / entrer
- **Bouton B long** = retour / annuler

L'appui long se détecte en mesurant la durée entre front descendant et front montant.

### Structure finale suggérée

```
src/
├── main.rs           # init, spawn des tâches
├── proto/
│   ├── mod.rs
│   ├── packet.rs     # parsing MeshCore  ← testé sur PC
│   ├── dedup.rs      # cache d'empreintes ← testé sur PC
│   └── message.rs    # ton payload postcard ← testé sur PC
├── hw/
│   ├── radio.rs      # tâche LoRa
│   ├── display.rs    # tâche écran
│   └── buttons.rs    # tâche boutons
└── app/
    ├── state.rs      # machine à états
    └── ui.rs         # rendu
```

**Le principe** : tout ce qui est dans `proto/` est du calcul pur, sans matériel, testable par `cargo test` sur ton PC. Tout ce qui est dans `hw/` touche la carte. C'est la même séparation que l'architecture en couches — elle paie toujours.

### Exercices

**9.1 ★ — Deux écrans**
Implémente une machine à états à deux écrans (Statut et Liste) avec le bouton A pour basculer de l'un à l'autre.

**9.2 ★★ — Navigation complète**
Ajoute la convention à deux boutons : A pour naviguer, B court pour entrer, B long pour revenir. Trois niveaux : Statut → Liste → Détail.

Contrainte : le `match` sur `(état, événement)` doit être exhaustif **sans bras fourre-tout `_`**. Si le compilateur accepte ton code, tu as traité toutes les transitions explicitement.

**9.3 ★★★ — Historique persistant**
Stocke les 20 derniers messages reçus dans un `heapless::Vec`, navigables à l'écran, avec horodatage relatif (« il y a 3 min »).

Puis, en combinant avec l'annexe A : fais survivre cet historique à un deep sleep en le plaçant en RTC SRAM. Question à trancher toi-même : qu'est-ce qui doit survivre à une coupure totale d'alimentation, et qu'est-ce qui peut disparaître ?

---

## Annexe A — Persistance

### Ce qu'il faut sauvegarder

Ta configuration (paramètres radio, préférences UI), et éventuellement ton compteur de séquence.

### Deux options

**EEPROM I²C externe** (`eeprom24x`, v0.7.2) — écriture non bloquante pour le CPU, endurance ~1 million de cycles par octet. C'est l'option recommandée si tu écris souvent.

**Flash interne** (`esp-storage` v0.9.0 + `sequential-storage`) — gratuit, mais **l'écriture bloque le CPU** (le cache d'instructions est désactivé pendant l'opération) et l'endurance est d'environ 100 000 cycles par secteur.

### Règles

- N'écris que sur **événement explicite** (changement de config validé), jamais dans une boucle.
- Ajoute un **CRC32 et un numéro de version** en tête. Une config corrompue lue sans vérification, c'est un nœud en brique.
- Prévois un retour aux valeurs par défaut si la vérification échoue.

### La RTC SRAM

Tes 16 Ko de RTC SRAM **survivent au deep sleep**. Compteurs, cache de déduplication, heure : garde-les là plutôt qu'en flash. C'est gratuit, illimité en écriture, et ça préserve l'endurance de ta flash pour ce qui doit vraiment survivre à une coupure totale.

---

## Annexe B — Débogage

### `esp-println` — le `printf` du pauvre

```rust
use esp_println::println;
println!("paquet: {} octets, rssi {}", len, rssi);
```

Simple, efficace, suffisant pour 90 % des cas. Coût : quelques kilooctets de flash et du temps CPU.

### `defmt` — le logging pour l'embarqué

`defmt` n'envoie pas la chaîne de caractères sur le fil : il envoie un **identifiant** et les arguments bruts. La chaîne est reconstituée côté PC à partir des symboles du binaire.

Résultat : des logs quasi gratuits en flash et en temps CPU. C'est ce qu'il faut utiliser dès que le débit de logs devient significatif.

### Lire un backtrace

Quand ça panique, `esp-backtrace` affiche une liste d'adresses. Pour les traduire :

```bash
espflash decode --chip esp32c6 target/riscv32imac-unknown-none-elf/release/mon-noeud
```

### Diagnostic : « je n'entends rien »

Dans l'ordre :

1. Sync word
2. Fréquence, SF, BW, CR
3. Antenne branchée et bonne bande
4. BUSY et DIO1 correctement câblés et déclarés
5. Test avec sync word public (0x34) pour valider le matériel

### Diagnostic : « ça compile pas »

| Erreur | Cause habituelle |
|---|---|
| `can't find crate for 'std'` | une dépendance n'est pas `no_std` → `cargo tree` |
| `can't find crate for 'core'` | cible non installée → `rustup target add` |
| `the trait bound X: Y is not satisfied` | versions de crates incompatibles |
| `borrowed value does not live long enough` | problème de `'static` avec une tâche Embassy |

---

## Annexe C — Budget mémoire

### Mesurer la taille de ton binaire

```bash
cargo install cargo-bloat
cargo bloat --release -n 20
```

Te montre les 20 fonctions les plus grosses. Souvent révélateur : le formatage de chaînes et le calcul flottant sont les coupables classiques.

### Voir la répartition mémoire

```bash
riscv32-esp-elf-size -A target/riscv32imac-unknown-none-elf/release/mon-noeud
```

- `.text` → code, va en flash
- `.rodata` → constantes, en flash
- `.data` + `.bss` → variables, **en RAM**

C'est `.data` + `.bss` qu'il faut surveiller.

### Budget indicatif pour ce projet

| Poste | RAM |
|---|---|
| Ton code (.data/.bss) | 20–30 Ko |
| Piles des tâches Embassy | 10–30 Ko |
| lora-phy + tampons | ~10 Ko |
| Cache de déduplication | < 1 Ko |
| **Total** | **~50–70 Ko** |

Sur 512 Ko, tu es très à l'aise. **À condition de rester en `no_std`** : le mode `std` via ESP-IDF consommerait à lui seul plus de la moitié de ta RAM.

Si tu ajoutes le WiFi (`esp-radio`), compte 50 à 70 Ko de plus.

---

## Ressources

### Documentation officielle

- **Hub esp-rs** (versions à jour de toutes les crates) — <https://docs.espressif.com/projects/rust/>
- Formation no_std officielle — <https://docs.espressif.com/projects/rust/no_std-training/>
- esp-hal — <https://github.com/esp-rs/esp-hal>
- Awesome ESP Rust (projets C6 réels) — <https://github.com/esp-rs/awesome-esp-rust>

### Apprendre Rust

- The Rust Book (chapitres 1 à 10 suffisent ici) — <https://doc.rust-lang.org/book/>
- The Embedded Rust Book — <https://docs.rust-embedded.org/book/>
- Embassy Book (async embarqué) — <https://embassy.dev/book/>

### Bibliothèques

- lora-phy — <https://crates.io/crates/lora-phy> · projet <https://github.com/lora-rs/lora-rs>
- embedded-graphics — <https://github.com/embedded-graphics/embedded-graphics>
- mipidsi — <https://github.com/almindor/mipidsi>
- eeprom24x — <https://github.com/eldruin/eeprom24x-rs>

### MeshCore

- Dépôt — <https://github.com/meshcore-dev/MeshCore>
- Format de paquet — <https://github.com/meshcore-dev/MeshCore/blob/main/docs/packet_format.md>
- Protocole réseau — <https://deepwiki.com/meshcore-dev/MeshCore/7-network-protocol>
- Firmwares prêts — <https://meshcore.ninja/firmware/meshcore/>

---

## Note sur les versions

Les numéros donnés ici sont ceux de juillet 2026 : esp-hal 1.1.0, esp-storage 0.9.0, lora-phy 3.0.1, mipidsi 0.9.0, eeprom24x 0.7.2.

L'écosystème ESP32 en Rust bouge vite. `esp-wifi` est devenu `esp-radio`, `esp-hal-embassy` a été fusionné dans `esp-rtos` — beaucoup de tutoriels en ligne sont périmés sur ces points.

**En cas de doute, la source de vérité est <https://docs.espressif.com/projects/rust/>**, et le code généré par `esp-generate` reflète toujours l'API courante. Si un exemple de ce document ne compile pas, compare-le au template généré.

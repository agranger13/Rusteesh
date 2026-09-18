# Corrections — Nœud LoRa MeshCore en Rust sur ESP32-C6

Corrections des exercices du tutoriel principal.

**Avant de lire une correction**, assure-toi d'avoir vraiment cherché. Sur les exercices ★★ et ★★★, la difficulté *est* l'apprentissage. Une solution lue et comprise s'oublie ; une solution trouvée après avoir bataillé avec le compilateur reste.

Les solutions données ne sont pas les seules valables. Si la tienne diffère mais fonctionne et que tu sais expliquer pourquoi, elle est bonne.

---

# Partie 0 — Reconnaissance

## 0.1 ★ — La fiche de référence

Pas de correction possible : tes valeurs dépendent de ton maillage local. Vérifie simplement que ta fiche contient les six paramètres RF **et** ton brochage complet, y compris BUSY et DIO1 pour la radio, et la broche de rétroéclairage pour l'écran.

Ces deux dernières sont celles qu'on oublie, et ce sont celles qui coûtent le plus cher en partie 3 et 5.

## 0.2 ★★ — Calcule ton temps d'antenne

### La méthode

**Durée d'un symbole :**

```
Ts = 2^SF / BW
```

**Durée du préambule :**

```
T_preamble = (n_preamble + 4,25) × Ts
```

**Nombre de symboles du payload :**

```
n_payload = 8 + max( ceil( (8×PL − 4×SF + 28 + 16×CRC − 20×IH) / (4×(SF − 2×DE)) ) × (CR+4) , 0 )
```

Avec `PL` = octets de payload, `CRC` = 1 si activé, `IH` = 0 si en-tête explicite, `DE` = 1 si l'optimisation bas débit est active, `CR` = 1 à 4 pour 4/5 à 4/8.

### Exemple chiffré — SF11, BW 250 kHz, CR 4/5, 50 octets

```
Ts = 2048 / 250000 = 8,192 ms

T_preamble = (16 + 4,25) × 8,192 = 165,9 ms

numérateur   = 8×50 − 4×11 + 28 + 16 = 400
dénominateur = 4 × 11 = 44
400 / 44 = 9,09 → ceil = 10
10 × 5 = 50
n_payload = 8 + 50 = 58 symboles

T_payload = 58 × 8,192 = 475,1 ms
```

**Temps d'antenne total ≈ 641 ms.**

### La conséquence réglementaire

À 1 % de rapport cyclique :

```
Cycle minimal = 641 ms × 100 = 64,1 s
Silence obligatoire = 641 ms × 99 ≈ 63,5 s
Messages autorisés = 3600 / 64,1 ≈ 56 par heure
```

### Ce qu'il faut en retenir

**Environ un message par minute.** Pas cinquante par seconde.

C'est le chiffre qui doit gouverner toute la conception de ton application. Une UI qui envoie un paquet à chaque appui sur un bouton est légalement impossible. Tu dois penser en termes de messages rares et de retransmission économe.

Refais le calcul avec SF7 : tu tomberas autour de 60 ms de temps d'antenne, donc environ 600 messages par heure — mais avec une portée bien moindre. C'est le compromis fondamental de LoRa.

## 0.3 ★★★ — Cartographie ton maillage

Pas de correction. Ce qui compte, ce sont tes chiffres. Retiens surtout le **débit de paquets par heure** : il te servira en 5.2 et 7.3.

Ordre de grandeur typique en zone urbaine avec quelques répéteurs actifs : quelques centaines à quelques milliers de paquets par heure. En zone rurale isolée, ça peut tomber à quelques dizaines.

---

# Partie 1 — Chaîne d'outils

## 1.1 ★ — Motif de battement de cœur

```rust
loop {
    for _ in 0..2 {
        led.set_high();
        attendre_ms(100);
        led.set_low();
        attendre_ms(100);
    }
    attendre_ms(1000);
}

fn attendre_ms(ms: u64) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(ms) {}
}
```

Note le `for _ in 0..2` : le `_` signifie « je n'utilise pas la variable d'itération ». Rust émet un avertissement si tu nommes une variable sans t'en servir ; `_` le supprime.

## 1.2 ★★ — Provoque une erreur d'ownership

### Le code fautif

```rust
let mut led = Output::new(peripherals.GPIO8, Level::Low, OutputConfig::default());
let mut led2 = Output::new(peripherals.GPIO8, Level::Low, OutputConfig::default());
```

### L'erreur

```
error[E0382]: use of moved value: `peripherals.GPIO8`
  --> src/main.rs:18:30
   |
17 |     let mut led = Output::new(peripherals.GPIO8, ...);
   |                               ----------------- value moved here
18 |     let mut led2 = Output::new(peripherals.GPIO8, ...);
   |                                ^^^^^^^^^^^^^^^^^ value used here after move
```

### La formulation attendue

*« GPIO8 est une valeur unique. En la passant à `Output::new`, j'en ai transféré la propriété. Elle n'existe plus à l'endroit d'où elle vient, donc je ne peux pas la donner une seconde fois. »*

### Pourquoi c'est important

En C, `pinMode(8, OUTPUT)` appelé depuis deux bibliothèques différentes compile parfaitement et produit un bug matériel intermittent que tu passeras une soirée à traquer à l'oscilloscope.

Ici, le conflit est détecté à la compilation, gratuitement, avec un message qui pointe les deux lignes en cause. C'est le bénéfice concret de l'ownership sur du code embarqué — et c'est la raison pour laquelle `esp_hal::init()` te rend une structure unique contenant tous les périphériques.

## 1.3 ★★★ — Debug contre release

### Ce que tu devrais observer

| Mode | `.text` typique | Facteur |
|---|---|---|
| debug | 40–80 Ko | — |
| release | 8–20 Ko | 4× à 6× plus petit |

### Ce que `cargo bloat` révèle

Dans un simple blink, les plus gros consommateurs sont presque toujours :

1. **Le formatage de chaînes** (`core::fmt`) — tiré par le panic handler et `esp-println`. La machinerie de formatage de Rust est puissante et générique, donc grosse. À elle seule, elle peut représenter la moitié du binaire.
2. **Le panic handler et son backtrace**
3. **Le code de démarrage** (initialisation de la RAM, table de vecteurs)

### La leçon

Même en `no_std`, tu embarques une infrastructure non triviale. Le formatage est le premier poste à surveiller quand la place manque — d'où l'intérêt de `defmt` (annexe B), qui déporte le formatage sur le PC.

Et l'option `panic-immediate-abort` existe pour les cas désespérés, au prix de messages de panique inutilisables.

---

# Partie 2 — Les boutons

## 2.1 ★ — Le second bouton

```rust
let mut deb_a = Debouncer::new();
let mut deb_b = Debouncer::new();

loop {
    if deb_a.update(bouton_a.is_low()) {
        led.set_high();
    }
    if deb_b.update(bouton_b.is_low()) {
        led.set_low();
    }
    attendre_ms(2);
}
```

Rien de subtil, mais note que chaque bouton a **son propre** `Debouncer`. Un état partagé produirait des interférences entre les deux boutons.

## 2.2 ★★ — Détection d'appui long

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
enum Appui {
    Rien,
    Court,
    Long,
}

struct Bouton {
    etat: bool,
    compteur: u8,
    debut: Option<Instant>,
    long_signale: bool,
}

impl Bouton {
    const SEUIL: u8 = 5;
    const SEUIL_LONG_MS: u64 = 800;

    fn new() -> Self {
        Self { etat: false, compteur: 0, debut: None, long_signale: false }
    }

    fn update(&mut self, appuye: bool) -> Appui {
        // signaler l'appui long dès qu'il est atteint, sans attendre le relâchement
        if self.etat && !self.long_signale {
            if let Some(t0) = self.debut {
                if t0.elapsed() >= Duration::from_millis(Self::SEUIL_LONG_MS) {
                    self.long_signale = true;
                    return Appui::Long;
                }
            }
        }

        if appuye == self.etat {
            self.compteur = 0;
            return Appui::Rien;
        }

        self.compteur += 1;
        if self.compteur < Self::SEUIL {
            return Appui::Rien;
        }

        self.compteur = 0;
        self.etat = appuye;

        if appuye {
            self.debut = Some(Instant::now());
            self.long_signale = false;
            Appui::Rien
        } else if self.long_signale {
            Appui::Rien // le long a déjà été signalé
        } else {
            Appui::Court
        }
    }
}
```

### Les deux décisions de conception

**L'appui long est signalé pendant l'appui**, pas au relâchement. C'est ce qu'attend l'utilisateur : il appuie, quelque chose se passe, il relâche. Signaler au relâchement donne une impression de latence.

**Le flag `long_signale` évite le double événement.** Sans lui, un appui long produirait `Long` puis `Court` au relâchement.

Note l'usage de `if let Some(t0) = self.debut` : c'est la façon idiomatique de traiter un `Option` quand un seul cas t'intéresse.

## 2.3 ★★★ — Ta première fonction générique

```rust
use embedded_hal::digital::InputPin;

fn lire_actif_bas<P>(pin: &mut P) -> bool
where
    P: InputPin,
{
    pin.is_low().unwrap_or(false)
}
```

### Le type factice pour tester

```rust
struct FauxBouton {
    appuye: bool,
}

impl embedded_hal::digital::ErrorType for FauxBouton {
    type Error = core::convert::Infallible;
}

impl InputPin for FauxBouton {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(!self.appuye)
    }
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(self.appuye)
    }
}

#[test]
fn faux_bouton_marche() {
    let mut b = FauxBouton { appuye: true };
    assert!(lire_actif_bas(&mut b));
}
```

### Les deux points qui coincent

**`ErrorType` est un trait à part.** `embedded-hal` 1.0 a séparé le type d'erreur dans son propre trait ; tu dois l'implémenter avant `InputPin`. C'est une source classique d'erreur « the trait bound is not satisfied » quand on suit un vieux tutoriel écrit pour la 0.2.

**`Infallible`** est le type qui ne peut pas être construit. L'utiliser comme type d'erreur dit au compilateur « cette opération ne peut jamais échouer », et il peut optimiser en conséquence.

### Ce que tu viens de faire

Tu as découplé ta logique du matériel réel. Le même code fonctionne avec une vraie broche et avec un faux bouton — donc il est **testable sur PC**.

C'est exactement le mécanisme que tu vas retrouver en partie 3 avec `DrawTarget`, et qui te permettra de développer ton UI dans une fenêtre desktop.

---

# Partie 3 — L'écran TFT

## 3.1 ★ — Un cadre et un titre

```rust
use embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle};

let bordure = PrimitiveStyle::with_stroke(Rgb565::WHITE, 2);

Rectangle::new(Point::new(0, 0), Size::new(240, 240))
    .into_styled(bordure)
    .draw(&mut display)?;

Text::with_alignment(
    "MESHCORE",
    Point::new(120, 24),
    MonoTextStyle::new(&FONT_9X15, Rgb565::CYAN),
    Alignment::Center,
)
.draw(&mut display)?;

Line::new(Point::new(8, 34), Point::new(232, 34))
    .into_styled(PrimitiveStyle::with_stroke(Rgb565::WHITE, 1))
    .draw(&mut display)?;
```

`Text::with_alignment` t'évite de calculer toi-même la position pour centrer. Utilise-le systématiquement.

## 3.2 ★★ — Rends ton rendu générique

```rust
use embedded_graphics::{
    mono_font::{ascii::FONT_9X15, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    text::Text,
};

pub fn dessiner_statut<D>(cible: &mut D, compteur: u32) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    let style = MonoTextStyle::new(&FONT_9X15, Rgb565::WHITE);
    let mut buf: heapless::String<32> = heapless::String::new();
    let _ = core::fmt::Write::write_fmt(&mut buf, format_args!("Paquets: {}", compteur));

    Text::new(&buf, Point::new(10, 60), style).draw(cible)?;
    Ok(())
}
```

### Le faux `DrawTarget`

```rust
struct CompteurPixels {
    nb: u32,
}

impl Dimensions for CompteurPixels {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(Point::zero(), Size::new(160, 128))
    }
}

impl DrawTarget for CompteurPixels {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for _ in pixels {
            self.nb += 1;
        }
        Ok(())
    }
}
```

```rust
// appelé depuis main(), sur la vraie carte
let mut faux = CompteurPixels { nb: 0 };
dessiner_statut(&mut faux, 42).unwrap();
println!("pixels dessinés sur le faux écran : {}", faux.nb);
```

### Ce que ça prouve, et pourquoi c'est suffisant à ce stade

`dessiner_statut` ne mentionne à aucun endroit `mipidsi`, `SpiInterface`, ni aucun type lié au SPI. Elle ne connaît que `DrawTarget<Color = Rgb565>`. Tu viens de l'appeler avec deux types qui n'ont **rien en commun** à part ce trait : ton vrai écran physique, et `CompteurPixels`, qui n'écrit dans aucune mémoire, ne parle à aucun bus, et existe uniquement pour compter.

C'est le polymorphisme sans coût à l'exécution : le compilateur génère une version spécialisée de `dessiner_statut` pour chaque type concret (monomorphisation), sans table virtuelle ni indirection — exactement comme pour `lire_actif_bas` en 2.3.

**Point important : ce code compile et s'exécute pour ta cible `riscv32imac-unknown-none-elf`, comme n'importe quel autre code de ce chapitre.** Ce n'est pas un test au sens `#[test]` — c'est un appel de fonction ordinaire, flashé et exécuté sur la carte, dont tu observes le résultat par `println!`. Aucune dépendance externe, aucun souci de cible.

### Pourquoi ce n'est pas encore un vrai test `cargo test`

Tu pourrais être tenté d'enrober ça dans `#[cfg(test)] mod tests { #[test] fn ... }` tout de suite. Essaie — tu vas obtenir une erreur d'exécution, pas de compilation : Cargo compile bien le test, mais tente ensuite de **l'exécuter sur ta cible active**, qui est `riscv32imac-unknown-none-elf`. Un binaire de test compilé pour cette cible ne peut pas tourner directement sur ton PC.

C'est précisément le problème que corrige le duo `lib.rs`/`main.rs` de l'Interlude (voir la correction I.2) : en compilant ta logique dans une lib à part, avec `#![cfg_attr(not(test), no_std)]`, et en pointant `cargo test` vers la cible de ta machine plutôt que vers le C6, `#[test]` redevient utilisable normalement — et c'est là que tu retrouveras `embedded-graphics-simulator` avec une vraie fenêtre desktop, en I.3.

### Le piège du `write!`

`write!` sur une `heapless::String` nécessite `use core::fmt::Write;` (ou l'appel explicite `core::fmt::Write::write_fmt` comme ci-dessus). Sans cet import, la méthode n'est pas visible et l'erreur est déroutante. C'est une particularité de Rust : **les traits doivent être importés pour que leurs méthodes soient utilisables**.

## 3.3 ★★★ — Redessin partiel mesuré

```rust
// Version A — tout effacer
let t0 = Instant::now();
display.clear(Rgb565::BLACK)?;
dessiner_cadre(&mut display)?;
dessiner_statut(&mut display, compteur)?;
let duree_a = t0.elapsed();

// Version B — seulement la zone du compteur
let t0 = Instant::now();
Rectangle::new(Point::new(10, 48), Size::new(140, 16))
    .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
    .draw(&mut display)?;
dessiner_statut(&mut display, compteur)?;
let duree_b = t0.elapsed();
```

### Les chiffres attendus

Sur un écran 240×240 en SPI à 40 MHz :

| | Pixels | Octets SPI | Temps |
|---|---|---|---|
| Plein écran | 57 600 | 115 200 | ~25–30 ms |
| Zone compteur | 2 240 | 4 480 | ~1–2 ms |

**Un facteur 20 à 25.**

### Pourquoi c'est décisif sur ce projet

30 ms de blocage, c'est acceptable pour un affichage de compteur. Ça ne l'est plus quand une réception LoRa peut arriver à tout moment.

Et surtout : c'est ce qui te permet de te passer complètement de framebuffer. Un framebuffer 240×240 en RGB565 coûte 115 Ko de RAM — sur un C6 sans PSRAM, avec la pile radio à alimenter, c'est une dépense que tu ne peux pas te permettre pour un affichage quasi statique.

**La règle générale** : sur un tableau de bord, seuls quelques nombres changent. Redessine-les, pas le décor.

---

# Interlude — Architecture

## I.1 ★ — Extraire le `Debouncer`

```rust
// src/debounce.rs
pub struct Debouncer {
    etat: bool,
    compteur: u8,
}

impl Debouncer {
    const SEUIL: u8 = 5;

    pub fn new() -> Self {
        Self { etat: false, compteur: 0 }
    }

    pub fn update(&mut self, appuye: bool) -> bool {
        if appuye == self.etat {
            self.compteur = 0;
            return false;
        }
        self.compteur += 1;
        if self.compteur >= Self::SEUIL {
            self.compteur = 0;
            self.etat = appuye;
            return appuye;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignore_sous_le_seuil() {
        let mut d = Debouncer::new();
        for _ in 0..4 {
            assert!(!d.update(true));
        }
    }

    #[test]
    fn declenche_au_seuil() {
        let mut d = Debouncer::new();
        for _ in 0..4 {
            d.update(true);
        }
        assert!(d.update(true)); // 5e appel consécutif
    }

    #[test]
    fn ne_redeclenche_pas_tant_que_maintenu() {
        let mut d = Debouncer::new();
        for _ in 0..5 {
            d.update(true);
        }
        assert!(!d.update(true)); // toujours appuyé, pas un nouveau front
    }
}
```

```rust
// src/main.rs
mod debounce;
use debounce::Debouncer;
```

### Ce qu'il faut vérifier

`cargo test` doit faire tourner ces trois tests **sans jamais toucher au port série ni au matériel**. Si `cargo test` tente de compiler pour `riscv32imac-unknown-none-elf` ou réclame une carte connectée, quelque chose ne va pas dans ta configuration de cible par défaut — vérifie `.cargo/config.toml`, la cible par défaut ne doit s'appliquer qu'à `build`/`run`, pas nécessairement à `test` selon comment tu l'as structuré (en pratique, la cible par défaut du fichier s'applique aussi à `test`, donc si `cargo test` échoue à trouver `std`, c'est normal — vois l'exercice I.2 qui résout précisément ce point via `lib.rs`).

C'est en fait la limite de cet exercice pris isolément : avec un `mod debounce;` dans `main.rs` qui reste `#![no_std]`, `cargo test` risque d'échouer à cause de la cible par défaut. C'est exactement le problème que I.2 résout.

## I.2 ★★ — Le duo `lib.rs` / `main.rs`

```toml
# Cargo.toml
[package]
name = "mon-noeud"
version = "0.1.0"
edition = "2021"

[lib]
name = "mon_projet"
path = "src/lib.rs"

[[bin]]
name = "mon-noeud"
path = "src/main.rs"
```

```rust
// src/lib.rs
#![cfg_attr(not(test), no_std)]

pub mod debounce;
```

```rust
// src/debounce.rs — inchangé, toujours no_std-compatible
pub struct Debouncer { /* ... */ }
impl Debouncer { /* ... */ }

#[cfg(test)]
mod tests { /* ... */ }
```

```rust
// src/main.rs
#![no_std]
#![no_main]

esp_bootloader_esp_idf::esp_app_desc!();

use esp_backtrace as _;
use mon_projet::debounce::Debouncer;

#[esp_hal::main]
fn main() -> ! {
    // ... init matériel, utilise Debouncer normalement
    loop {}
}
```

### Le point délicat : la cible par défaut

Si ton `.cargo/config.toml` fixe la cible par défaut sur `riscv32imac-unknown-none-elf` (ce que fait `esp-generate`), alors `cargo test` **sans précision** essaiera de compiler pour cette cible — et ta lib en mode `not(test)` n'est même pas censée s'appliquer, mais l'exécutable de test, lui, a besoin de `std` pour tourner et rapporter ses résultats. Ça ne compile pas.

La solution : dire explicitement à `cargo test` de viser ta machine hôte.

```bash
cargo test --target x86_64-unknown-linux-gnu   # adapte à ton OS
```

Ou, mieux, ajoute un alias dans `.cargo/config.toml` :

```toml
[alias]
test-host = "test --target x86_64-unknown-linux-gnu"
```

```bash
cargo test-host
```

Sur macOS, remplace par `aarch64-apple-darwin` ou `x86_64-apple-darwin` selon ta puce ; sur Windows, `x86_64-pc-windows-msvc`.

### Pourquoi ça marche malgré tout

`#![cfg_attr(not(test), no_std)]` s'applique à **la compilation de la lib elle-même**. Quand tu lances `cargo test --target <ta machine>`, Cargo compile `lib.rs` une seconde fois, pour cette cible-là, avec `cfg(test)` actif — donc `no_std` désactivé, `std` disponible, et tes `#[test]` peuvent utiliser tout ce qu'ils veulent (y compris `println!`, au passage, si tu en as besoin pour déboguer un test).

C'est complètement indépendant de la compilation `--release` pour le C6, qui elle ne définit jamais `cfg(test)` et reste `no_std` de bout en bout.

### Vérification que la séparation est propre

```bash
cargo build --release --target riscv32imac-unknown-none-elf
cargo test-host
```

Les deux doivent réussir indépendamment. Si le build embarqué échoue avec une erreur mentionnant `std`, une dépendance `std`-only s'est glissée dans `lib.rs` — vérifie `Cargo.toml` pour un `default-features` oublié (le motif du tout début du tutoriel).

## I.3 ★★★ — `ui.rs` avec test sur simulateur

```toml
[dev-dependencies]
embedded-graphics-simulator = "0.8"
```

```rust
// src/ui.rs
use embedded_graphics::{
    mono_font::{ascii::FONT_9X15, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    text::Text,
};

pub fn dessiner_statut<D>(cible: &mut D, compteur: u32) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    cible.clear(Rgb565::BLACK)?;
    let style = MonoTextStyle::new(&FONT_9X15, Rgb565::WHITE);

    let mut buf: heapless::String<32> = heapless::String::new();
    let _ = core::fmt::Write::write_fmt(&mut buf, format_args!("Paquets: {}", compteur));

    Text::new(&buf, Point::new(10, 60), style).draw(cible)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics_simulator::SimulatorDisplay;

    #[test]
    fn dessine_sans_erreur() {
        let mut display: SimulatorDisplay<Rgb565> = SimulatorDisplay::new(Size::new(160, 128));
        assert!(dessiner_statut(&mut display, 42).is_ok());
    }

    #[test]
    fn le_fond_est_noir_hors_texte() {
        let mut display: SimulatorDisplay<Rgb565> = SimulatorDisplay::new(Size::new(160, 128));
        dessiner_statut(&mut display, 42).unwrap();

        // un pixel loin du texte doit être resté noir
        let pixel = display.get_pixel(Point::new(5, 5));
        assert_eq!(pixel, Some(Rgb565::BLACK));
    }
}
```

```rust
// src/lib.rs
#![cfg_attr(not(test), no_std)]

pub mod debounce;
pub mod ui;
```

### Vérifier l'étanchéité

```bash
cargo tree --target riscv32imac-unknown-none-elf | grep simulator
```

**Ne doit rien afficher.** Les `dev-dependencies` ne sont considérées par Cargo que pour `cargo test` et `cargo bench` — jamais pour `cargo build`, avec ou sans `--release`, quelle que soit la cible. `embedded-graphics-simulator` dépend de SDL2 et suppose un OS ; si elle se retrouvait liée dans ton firmware, il ne flasherait tout simplement pas.

```bash
cargo test-host
```

Doit faire apparaître une fenêtre SDL2 si tu lances `show_static` quelque part, ou simplement passer silencieusement si tu ne fais que des assertions sur les pixels comme ci-dessus — ce qui est la version utile pour une CI, où il n'y a pas d'écran pour afficher une fenêtre.

### Ce que cet exercice démontre, au fond

Tu viens de faire tourner un test **visuel** sans aucun matériel, aucun flash, et sans même qu'une fenêtre s'affiche si tu ne le demandes pas explicitement. C'est ce qui rend crédible l'idée, répétée depuis la partie 3, de développer une interface embarquée « à côté » plutôt que « sur » la puce — le gain de vitesse d'itération n'est pas anecdotique, il change concrètement la façon dont tu vas travailler sur les parties 5 à 9.

---

# Partie 4 — Embassy

## 4.1 ★ — Une troisième tâche

```rust
#[embassy_executor::task]
async fn tache_led(mut led: Output<'static>) {
    loop {
        led.toggle();
        Timer::after(Duration::from_millis(500)).await;
    }
}

// dans main
spawner.spawn(tache_led(led)).unwrap();
```

Le point à remarquer : cette tâche est **totalement indépendante**. Elle n'a besoin d'aucune coordination avec les autres. C'est ce que permet l'async — chaque tâche décrit son propre comportement dans le temps.

## 4.2 ★★ — Sature le canal

```rust
static EVENEMENTS: Channel<CriticalSectionRawMutex, Evenement, 1> = Channel::new();

// producteur
match EVENEMENTS.try_send(Evenement::BoutonA) {
    Ok(()) => {}
    Err(_) => { perdus += 1; }
}
```

### Ce que tu observes

Avec une tâche écran ralentie à 500 ms et une capacité de 1, marteler les boutons fait grimper `perdus` très vite.

### La réponse attendue

| | `send().await` | `try_send()` |
|---|---|---|
| Canal plein | attend qu'il y ait de la place | abandonne immédiatement |
| Risque | blocage de la tâche productrice | perte de données |
| Bon pour | données qui ne doivent pas être perdues | données périssables |

**Pour des boutons, `send().await` est correct.** Un appui utilisateur ne doit pas être perdu : si l'UI est en retard, il faut que l'événement l'attende, pas qu'il disparaisse. Et le producteur (la scrutation des boutons) peut se permettre d'attendre quelques millisecondes.

**Pour des paquets radio, `try_send()` est souvent meilleur.** Si ton UI n'arrive pas à suivre, il vaut mieux perdre un paquet que bloquer la tâche radio — parce que pendant qu'elle est bloquée, elle ne reçoit plus rien du tout, et tu perds bien davantage.

C'est exactement le raisonnement UDP contre TCP, appliqué à l'intérieur de ton programme.

## 4.3 ★★★ — Casse l'exécuteur, puis répare-le

### Ce que tu observes

Tout se fige : l'écran ne se rafraîchit plus, la LED ne clignote plus, les boutons ne répondent plus. Pendant 500 ms, à chaque tour de boucle.

### L'explication attendue

*« L'exécuteur Embassy est **coopératif**, pas préemptif. Il ne peut donner la main à une autre tâche qu'aux points `.await`. Une boucle d'attente active n'en contient aucun : la tâche garde le CPU du début à la fin, et rien d'autre ne peut s'exécuter. »*

### La différence avec un RTOS

Un ordonnanceur préemptif (FreeRTOS, Linux) interrompt une tâche de force sur un tick de timer. Coût : chaque tâche a besoin de sa propre pile, soit plusieurs kilooctets.

Un exécuteur coopératif ne peut jamais interrompre. Bénéfice : pas de pile par tâche, quelques centaines d'octets suffisent. Contrainte : **tu es responsable de rendre la main**.

Sur ton C6 avec 512 Ko, ce compromis est le bon — mais il t'engage.

### La règle pratique

Dans une tâche async, toute attente doit passer par `.await`. Si un calcul long est inévitable (traitement d'un gros buffer, calcul cryptographique), découpe-le et insère des `Timer::after(Duration::from_ticks(0)).await` pour rendre la main.

### Pourquoi cet exercice compte

Tu vas recréer ce bug en partie 5. Le code de réception LoRa contient des attentes, et il est tentant d'écrire une boucle de scrutation sur un registre. Quand ton écran se figera à ce moment-là, tu sauras immédiatement où regarder.

---

# Partie 5 — Réception LoRa

## 5.1 ★ — Qualité de réception

```rust
fn barre_signal(rssi: i16) -> &'static str {
    match rssi {
        r if r > -80  => "||||",
        r if r > -100 => "|||",
        r if r > -115 => "||",
        r if r > -130 => "|",
        _             => ".",
    }
}

fn couleur_snr(snr: i16) -> Rgb565 {
    if snr > 5 { Rgb565::GREEN }
    else if snr > 0 { Rgb565::YELLOW }
    else { Rgb565::RED }
}
```

Note la syntaxe `r if r > -80 =>` : c'est un **match guard**, une condition attachée à un bras de `match`. Bien plus lisible qu'une cascade de `if`.

### Les ordres de grandeur à connaître

| RSSI | SNR | Interprétation |
|---|---|---|
| > −80 dBm | > 8 dB | très proche |
| −100 dBm | 0 à 5 dB | bon lien |
| −120 dBm | −5 dB | limite, LoRa décode encore |
| < −135 dBm | < −15 dB | sous le plancher |

**Le SNR négatif n'est pas une anomalie.** C'est la propriété remarquable de LoRa : la modulation à étalement de spectre décode des signaux enfouis sous le bruit. À SF12, jusqu'à −20 dB.

## 5.2 ★★ — Débit du maillage

```rust
use heapless::Deque;

struct Compteur {
    horodatages: Deque<Instant, 256>,
}

impl Compteur {
    fn enregistrer(&mut self) {
        if self.horodatages.is_full() {
            self.horodatages.pop_front();
        }
        let _ = self.horodatages.push_back(Instant::now());
    }

    fn purger(&mut self) {
        while let Some(t) = self.horodatages.front() {
            if t.elapsed() > Duration::from_secs(60) {
                self.horodatages.pop_front();
            } else {
                break;  // les suivants sont plus récents
            }
        }
    }

    fn par_minute(&self) -> usize {
        self.horodatages.len()
    }
}
```

### Les deux subtilités

**Le `break` dans `purger`** est ce qui rend la purge efficace. Comme les horodatages sont insérés dans l'ordre, dès qu'on trouve une entrée assez récente, toutes les suivantes le sont aussi. Sans ce `break`, tu parcourrais inutilement toute la file.

**`while let Some(t) = ...`** combine boucle et déstructuration : tant que `front()` retourne `Some`, on continue. C'est le motif idiomatique pour consommer une collection par un bout.

**Dimensionne le plafond avec marge.** Si ton maillage dépasse 256 paquets/minute, tu perds des horodatages et ton compteur ment. Mesure d'abord, dimensionne ensuite.

## 5.3 ★★★ — Scanner de sync word

```rust
const CANDIDATS: [u8; 4] = [0x12, 0x34, 0x2B, 0x14];

async fn scanner<RK, DLY>(lora: &mut LoRa<RK, DLY>, display: &mut impl DrawTarget<Color = Rgb565>)
where
    RK: RadioKind,
    DLY: DelayNs,
{
    let mut buf = [0u8; 255];

    for (i, sw) in CANDIDATS.iter().enumerate() {
        // reconfigurer le sync word, repasser en réception
        let mut recus = 0u32;
        let debut = Instant::now();

        while debut.elapsed() < Duration::from_secs(30) {
            match select(
                lora.rx(&rx_params, &mut buf),
                Timer::after(Duration::from_secs(1)),
            ).await {
                Either::First(Ok(_)) => recus += 1,
                _ => {}
            }
        }

        afficher_ligne(display, i, *sw, recus);
    }
}
```

### Le concept clé : `select`

`embassy_futures::select` fait courir deux futures et rend la main dès que **l'une** se termine. C'est indispensable ici : sans lui, `lora.rx()` attendrait indéfiniment sur un sync word silencieux, et ton scan ne progresserait jamais.

`Either::First` / `Either::Second` te disent laquelle a gagné.

C'est un motif que tu réutiliseras beaucoup : *« attends un paquet, mais pas plus de N secondes »*.

### La valeur réelle de cet outil

Le jour où tu n'entendras rien, tu auras deux hypothèses : mauvais paramètres, ou matériel défaillant. Ce scanner tranche en deux minutes.

S'il capte du trafic sur **au moins un** sync word, ton câblage, ton antenne et ta configuration de modulation sont bons — le problème est uniquement dans la valeur du sync word. S'il ne capte rien nulle part, c'est matériel.

Écris-le pendant que tout marche. C'est un outil de diagnostic, et un outil de diagnostic qu'on écrit en panne est un outil qu'on ne sait pas déboguer.

---

# Partie 6 — Décodage MeshCore

## 6.1 ★ — Type de route

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
enum RouteType {
    Flood,
    Direct,
    TransportFlood,
    TransportDirect,
    Inconnu(u8),
}

impl From<u8> for RouteType {
    fn from(v: u8) -> Self {
        match v & 0b0000_0011 {
            0x01 => RouteType::Flood,
            0x02 => RouteType::Direct,
            autre => RouteType::Inconnu(autre),
        }
    }
}
```

**Le point important : la variante `Inconnu(u8)`.**

Il est tentant de retourner une erreur sur une valeur non reconnue. C'est une mauvaise idée sur un décodeur réseau : le protocole évolue, et une V2 est annoncée. Un décodeur qui rejette ce qu'il ne connaît pas devient inutilisable à la première mise à jour du réseau.

En conservant la valeur brute, tu peux l'afficher, la journaliser, et continuer à traiter le reste du paquet. **Sois strict sur ce que tu émets, tolérant sur ce que tu reçois.**

Vérifie les positions de bits exactes dans `docs/packet_format.md` — le masque `0b11` ci-dessus est une hypothèse à confirmer.

## 6.2 ★★ — Un parseur robuste avec ses tests

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParseError {
    TropCourt { attendu: usize, recu: usize },
    CheminIncoherent { annonce: usize, disponible: usize },
}

#[derive(Debug)]
pub struct Packet<'a> {
    pub route: RouteType,
    pub hash_size: u8,
    pub hash_count: u8,
    pub chemin: &'a [u8],
    pub payload_type: u8,
    pub payload: &'a [u8],
}

impl<'a> TryFrom<&'a [u8]> for Packet<'a> {
    type Error = ParseError;

    fn try_from(o: &'a [u8]) -> Result<Self, Self::Error> {
        if o.len() < 3 {
            return Err(ParseError::TropCourt { attendu: 3, recu: o.len() });
        }

        let route = RouteType::from(o[0]);
        let path_length = o[1];
        let hash_size = (path_length >> 6) + 1;
        let hash_count = path_length & 0b0011_1111;

        let taille_chemin = (hash_size as usize)
            .checked_mul(hash_count as usize)
            .ok_or(ParseError::CheminIncoherent { annonce: 0, disponible: 0 })?;

        let fin_chemin = 2usize
            .checked_add(taille_chemin)
            .ok_or(ParseError::CheminIncoherent { annonce: taille_chemin, disponible: o.len() })?;

        if o.len() < fin_chemin + 1 {
            return Err(ParseError::CheminIncoherent {
                annonce: taille_chemin,
                disponible: o.len().saturating_sub(2),
            });
        }

        Ok(Packet {
            route,
            hash_size,
            hash_count,
            chemin: &o[2..fin_chemin],
            payload_type: o[fin_chemin],
            payload: &o[fin_chemin + 1..],
        })
    }
}
```

### Le lifetime `'a`

`Packet<'a>` contient des slices qui **pointent dans le buffer d'origine**. Le `'a` dit au compilateur : « ce `Packet` ne peut pas survivre au buffer dont il est issu ».

C'est ce qui rend le parsing **zéro-copie** : aucune donnée n'est dupliquée, tu n'as que des vues. Sur un microcontrôleur, c'est exactement ce qu'on veut — et le compilateur garantit que tu ne pourras pas garder un `Packet` après avoir réutilisé le buffer.

Si tu trouves les lifetimes difficiles au début, c'est normal. Retiens simplement : le `'a` relie la durée de vie de la sortie à celle de l'entrée.

### Les tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flood_minimal() {
        let brut = [0x01, 0x00, 0x0F, 0xAA, 0xBB];
        let p = Packet::try_from(&brut[..]).unwrap();
        assert_eq!(p.route, RouteType::Flood);
        assert_eq!(p.hash_count, 0);
        assert_eq!(p.payload_type, 0x0F);
        assert_eq!(p.payload, &[0xAA, 0xBB]);
    }

    #[test]
    fn avec_chemin() {
        // hash_size=1 (bits 00), hash_count=3 → path_length = 0x03
        let brut = [0x01, 0x03, 0xAA, 0xBB, 0xCC, 0x0F, 0x42];
        let p = Packet::try_from(&brut[..]).unwrap();
        assert_eq!(p.chemin, &[0xAA, 0xBB, 0xCC]);
        assert_eq!(p.payload, &[0x42]);
    }

    #[test]
    fn trop_court() {
        assert!(matches!(
            Packet::try_from(&[0x01, 0x00][..]),
            Err(ParseError::TropCourt { .. })
        ));
    }

    #[test]
    fn chemin_menteur() {
        // annonce 63 hash mais le paquet fait 4 octets
        let brut = [0x01, 0x3F, 0x0F, 0x00];
        assert!(Packet::try_from(&brut[..]).is_err());
    }

    #[test]
    fn payload_vide_est_valide() {
        let brut = [0x01, 0x00, 0x0F];
        let p = Packet::try_from(&brut[..]).unwrap();
        assert_eq!(p.payload.len(), 0);
    }
}
```

`matches!` est une macro pratique pour tester une variante sans se soucier de ses champs.

## 6.3 ★★★ — Ton parseur ne doit jamais paniquer

```rust
#[test]
fn jamais_de_panique() {
    let mut graine: u32 = 0x1234_5678;
    let mut buf = [0u8; 255];

    for _ in 0..100_000 {
        // générateur congruentiel linéaire — suffisant pour du fuzzing basique
        graine = graine.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let len = (graine as usize) % 256;

        for i in 0..len {
            graine = graine.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            buf[i] = (graine >> 16) as u8;
        }

        // le seul contrat : ça retourne, ça ne panique pas
        let _ = Packet::try_from(&buf[..len]);
    }
}
```

### Les bugs que ce test attrape

Si tu as écrit le parseur naïvement, ce test échoue sur au moins un de ces cas :

1. **`o[1]` sur un buffer d'un seul octet** → panique d'indexation
2. **`hash_size × hash_count`** avec les valeurs maximales (4 × 63 = 252) qui dépasse la longueur réelle
3. **`&o[2..fin_chemin]`** où `fin_chemin > o.len()` → panique de slicing
4. **`fin_chemin + 1`** qui déborde si `fin_chemin` vaut `usize::MAX`

### Pourquoi `wrapping_mul` et pas `*`

En mode debug, Rust panique sur dépassement arithmétique. Un générateur congruentiel déborde **par construction** — c'est son principe. `wrapping_mul` dit explicitement « le débordement est voulu ici ».

C'est une bonne illustration de la philosophie du langage : le comportement dangereux n'est pas interdit, il doit être demandé explicitement.

### La leçon

Ton nœud traite des données venues de l'extérieur, potentiellement corrompues par le canal radio. Un paquet mal formé ne doit **jamais** faire redémarrer l'appareil.

Ce test coûte vingt lignes et attrape une classe entière de bugs que tu ne découvrirais autrement qu'en production, à 800 m de chez toi, sans débogueur.

Le vrai fuzzing (`cargo-fuzz`) va plus loin en guidant la génération par la couverture de code. Mais pour un parseur de cette taille, cette version rudimentaire couvre l'essentiel.

---

# Partie 7 — Déduplication

## 7.1 ★ — Le cache d'empreintes

```rust
use heapless::Deque;
use sha2::{Digest, Sha256};

pub struct DedupCache {
    vus: Deque<[u8; 8], 32>,
}

impl DedupCache {
    pub fn new() -> Self {
        Self { vus: Deque::new() }
    }

    pub fn empreinte(payload_type: u8, payload: &[u8]) -> [u8; 8] {
        let mut h = Sha256::new();
        h.update([payload_type]);
        h.update(payload);
        let complet = h.finalize();
        let mut court = [0u8; 8];
        court.copy_from_slice(&complet[..8]);
        court
    }

    pub fn deja_vu(&self, e: &[u8; 8]) -> bool {
        self.vus.iter().any(|x| x == e)
    }

    pub fn inserer(&mut self, e: [u8; 8]) {
        if self.vus.is_full() {
            self.vus.pop_front();
        }
        let _ = self.vus.push_back(e);
    }

    /// Retourne true si le paquet est nouveau (et l'enregistre).
    pub fn verifier(&mut self, payload_type: u8, payload: &[u8]) -> bool {
        let e = Self::empreinte(payload_type, payload);
        if self.deja_vu(&e) {
            false
        } else {
            self.inserer(e);
            true
        }
    }
}
```

### Tests

```rust
#[test]
fn detecte_doublon() {
    let mut c = DedupCache::new();
    assert!(c.verifier(0x0F, b"bonjour"));
    assert!(!c.verifier(0x0F, b"bonjour"));
    assert!(c.verifier(0x0F, b"autre"));
}

#[test]
fn evince_les_plus_anciens() {
    let mut c = DedupCache::new();
    for i in 0..40u32 {
        c.verifier(0x0F, &i.to_le_bytes());
    }
    // le message 0 est sorti de la fenêtre
    assert!(c.verifier(0x0F, &0u32.to_le_bytes()));
}
```

**Note bien le second test** : après éviction, un ancien message est vu comme nouveau. C'est le comportement voulu, et c'est ce qui rend le dimensionnement de la fenêtre important.

### Sur `getrandom`

SHA-256 est une fonction pure : mêmes entrées, mêmes sorties, aucun aléa. Tu ne rencontreras donc **pas** le problème `getrandom` ici.

Il ne t'attendrait que si tu implémentais la crypto d'identité MeshCore (Ed25519, ECDH), qui a besoin d'entropie pour générer des clés — ce que ton rôle Companion en `0x0F` t'évite entièrement.

## 7.2 ★★ — Mesure le coût du flooding

Deux compteurs, l'un incrémenté à chaque paquet reçu, l'autre seulement quand `verifier()` retourne `true`.

### Interprétation des chiffres

| Ratio reçus/uniques | Interprétation |
|---|---|
| ~1,0 | tu n'entends qu'un seul répéteur |
| 2 à 4 | maillage sain, redondance normale |
| > 6 | maillage très dense, ou tu es au centre |

Ce ratio est une mesure directe du coût du flooding : c'est le nombre de fois où l'air a été occupé pour t'apporter une seule information.

C'est aussi ce qui explique pourquoi MeshCore apprend des chemins directs après la première découverte — le flooding permanent ne passerait pas à l'échelle.

## 7.3 ★★★ — Dimensionnement et collisions

### Le cache générique

```rust
pub struct DedupCache<const N: usize> {
    vus: Deque<[u8; 8], N>,
}

impl<const N: usize> DedupCache<N> {
    pub fn new() -> Self {
        Self { vus: Deque::new() }
    }
    // ... reste identique
}

// utilisation
let mut cache: DedupCache<64> = DedupCache::new();
```

Le `const N: usize` est un **paramètre de type qui est un nombre**. La taille est fixée à la compilation, donc aucune allocation, et le compilateur peut optimiser en connaissant N.

### Le calcul de collision

Une collision problématique = deux messages **différents** présents simultanément dans la fenêtre avec la même empreinte. Résultat : un message légitime rejeté comme doublon.

Avec une empreinte de 8 octets (64 bits) et une fenêtre de 32 :

```
P(collision pour un paquet) ≈ 32 / 2^64 ≈ 1,7 × 10⁻¹⁸
```

À 1 000 paquets/heure, soit 24 000 par jour :

```
P(au moins une collision par jour) ≈ 24 000 × 1,7 × 10⁻¹⁸ ≈ 4 × 10⁻¹⁴
P(par an)                          ≈ 1,5 × 10⁻¹¹
```

Autrement dit : **jamais**. Tu as plus de chances que la puce tombe en panne.

### Et avec 4 octets ?

```
P(collision par paquet) = 32 / 2^32 ≈ 7,5 × 10⁻⁹
P(par jour)             ≈ 1,8 × 10⁻⁴   → 1 chance sur 5 600
P(par an)               ≈ 6,5 %
```

**6,5 % de risque par an**, pour économiser 128 octets (32 × 4 au lieu de 32 × 8).

### La conclusion — et c'est le vrai objectif de l'exercice

Sur un C6 où il te reste 300 Ko de libre, **économiser 128 octets au prix de 6,5 % de risque annuel est un mauvais échange.** Garde 8 octets.

Cet exercice t'apprend surtout à *poser le calcul avant d'optimiser*. L'intuition dit « 4 octets suffisent sûrement » ; le calcul dit que le gain est négligeable et le coût réel. C'est un réflexe qui vaut bien au-delà de ce projet.

---

# Partie 8 — Émission

## 8.1 ★ — Mesure ta charge utile

```rust
#[derive(Serialize, Deserialize)]
struct Telemetrie {
    seq: u32,
    temperature_c: f32,
    batterie_mv: u16,
}

let msg = Telemetrie { seq: 42, temperature_c: 23.5, batterie_mv: 3820 };
let mut buf = [0u8; 64];
let octets = postcard::to_slice(&msg, &mut buf).unwrap();
// octets.len() ≈ 8
```

### Comparaison

| Format | Taille |
|---|---|
| `{"seq":42,"temperature_c":23.5,"batterie_mv":3820}` | 49 octets |
| postcard | ~8 octets |

**Facteur 6.**

### Version entière

```rust
struct Telemetrie {
    seq: u32,
    temperature_centi: i16,  // 2350 = 23,50 °C
    batterie_mv: u16,
}
// ≈ 7 octets
```

Le gain en octets est modeste (1 octet), mais le gain réel est ailleurs : **tu élimines l'émulation logicielle du flottant**. Le C6 n'a pas de FPU, donc chaque opération sur `f32` passe par des routines logicielles qui coûtent du temps CPU et plusieurs kilooctets de flash.

### En temps d'antenne

En reprenant SF11/250 kHz de l'exercice 0.2 :

| Payload | Temps d'antenne | Messages/heure à 1 % |
|---|---|---|
| 8 octets (postcard) | ~300 ms | ~120 |
| 49 octets (JSON) | ~640 ms | ~56 |

**Le choix du format de sérialisation double le nombre de messages que tu as le droit d'émettre.** Sur LoRa, la compacité n'est pas une coquetterie.

## 8.2 ★★ — Le garde-fou réglementaire

```rust
const FENETRE: Duration = Duration::from_secs(3600);
const BUDGET_MS: u64 = 36_000; // 1 % d'une heure

pub struct DutyCycle {
    emissions: Deque<(Instant, u64), 64>, // (instant, durée_ms)
    consomme_ms: u64,
}

impl DutyCycle {
    pub fn purger(&mut self) {
        while let Some(&(t, d)) = self.emissions.front() {
            if t.elapsed() > FENETRE {
                self.emissions.pop_front();
                self.consomme_ms -= d;
            } else {
                break;
            }
        }
    }

    pub fn autorise(&mut self, duree_ms: u64) -> bool {
        self.purger();
        self.consomme_ms + duree_ms <= BUDGET_MS
    }

    pub fn enregistrer(&mut self, duree_ms: u64) {
        if self.emissions.is_full() {
            if let Some((_, d)) = self.emissions.pop_front() {
                self.consomme_ms -= d;
            }
        }
        let _ = self.emissions.push_back((Instant::now(), duree_ms));
        self.consomme_ms += duree_ms;
    }

    pub fn pourcentage_restant(&self) -> u8 {
        (100 - (self.consomme_ms * 100 / BUDGET_MS).min(100)) as u8
    }
}
```

### Le point de conception

`consomme_ms` est maintenu de façon incrémentale plutôt que recalculé par une somme à chaque appel. Sur 64 entrées la différence est faible, mais c'est le bon réflexe : une fonction appelée avant chaque émission doit être bon marché.

### La formule du temps d'antenne

Tu dois la calculer, pas la deviner. Réutilise celle de l'exercice 0.2, avec la longueur réelle du paquet que tu vas émettre.

### Pourquoi c'est non négociable

Ce n'est pas une bonne pratique, c'est la **loi** (ETSI EN 300 220 en Europe). Et c'est aussi ce qui t'évite de saturer un réseau que tu partages avec d'autres — un nœud bavard dégrade le service pour tout le monde et se fait repérer très vite.

Affiche le pourcentage restant à l'écran. Tu verras concrètement à quel point le budget est serré.

## 8.3 ★★★ — CSMA avec backoff exponentiel

```rust
use esp_hal::rng::Rng;

async fn emettre_avec_csma<RK, DLY>(
    lora: &mut LoRa<RK, DLY>,
    rng: &mut Rng,
    paquet: &[u8],
    seuil_dbm: i16,
) -> Result<(), RadioError>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    let mut fenetre_ms: u32 = 32;

    for _tentative in 0..6 {
        let rssi = lora.get_rssi().await?;

        if rssi < seuil_dbm {
            return lora.tx(paquet).await;  // canal libre
        }

        let attente = (rng.random() % fenetre_ms) as u64;
        Timer::after(Duration::from_millis(attente)).await;
        fenetre_ms = (fenetre_ms * 2).min(2000);
    }

    Err(RadioError::TransmitTimeout)
}
```

### Générer de l'aléa en `no_std`

Trois options, par ordre de préférence :

1. **`esp_hal::rng::Rng`** — le générateur matériel du C6. C'est la bonne réponse. Note qu'il n'est pleinement entropique que si la radio est active ; sinon il dégrade en pseudo-aléatoire.
2. **Générateur congruentiel amorcé par le bruit RSSI** — les bits de poids faible d'une mesure de RSSI sur canal vide sont raisonnablement imprévisibles.
3. **Compteur simple** — à éviter : si tous tes nœuds démarrent ensemble, ils choisiront le même délai et entreront en collision systématiquement. C'est précisément ce que le backoff aléatoire est censé empêcher.

Tu ne peux pas utiliser `getrandom` : il demande une source d'entropie fournie par l'OS, qui n'existe pas ici. C'est le mur évoqué dans le tutoriel.

### Calibrer le seuil

Mesure le RSSI en continu pendant plusieurs minutes sur ton canal. Tu obtiendras un plancher de bruit, typiquement entre −120 et −130 dBm.

Place ton seuil environ 10 dB au-dessus. Trop bas, tu n'émets jamais ; trop haut, tu écrases les transmissions des autres.

### Pourquoi le plafond sur la fenêtre

Sans le `.min(2000)`, six échecs successifs donneraient une fenêtre de 2 secondes... puis 4, 8, 16. Sur un canal durablement occupé, tu attendrais des minutes. Mieux vaut abandonner et réessayer plus tard, au niveau applicatif.

---

# Partie 9 — L'application

## 9.1 ★ — Deux écrans

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
enum Ecran {
    Statut,
    Liste,
}

impl Ecran {
    fn suivant(self, ev: Evenement) -> Self {
        match (self, ev) {
            (Ecran::Statut, Evenement::BoutonA) => Ecran::Liste,
            (Ecran::Liste, Evenement::BoutonA) => Ecran::Statut,
            (e, _) => e,
        }
    }
}
```

## 9.2 ★★ — Navigation complète

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
enum Ecran {
    Statut,
    Liste { index: usize },
    Detail { index: usize },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Evenement {
    A,          // naviguer
    BCourt,     // entrer
    BLong,      // retour
}

impl Ecran {
    fn suivant(self, ev: Evenement, nb_messages: usize) -> Self {
        match (self, ev) {
            // Statut
            (Ecran::Statut, Evenement::A) => Ecran::Statut,
            (Ecran::Statut, Evenement::BCourt) => Ecran::Liste { index: 0 },
            (Ecran::Statut, Evenement::BLong) => Ecran::Statut,

            // Liste
            (Ecran::Liste { index }, Evenement::A) => {
                let n = nb_messages.max(1);
                Ecran::Liste { index: (index + 1) % n }
            }
            (Ecran::Liste { index }, Evenement::BCourt) => {
                if nb_messages == 0 { Ecran::Liste { index } }
                else { Ecran::Detail { index } }
            }
            (Ecran::Liste { .. }, Evenement::BLong) => Ecran::Statut,

            // Détail
            (Ecran::Detail { index }, Evenement::A) => {
                let n = nb_messages.max(1);
                Ecran::Detail { index: (index + 1) % n }
            }
            (Ecran::Detail { index }, Evenement::BCourt) => Ecran::Detail { index },
            (Ecran::Detail { .. }, Evenement::BLong) => Ecran::Liste { index: 0 },
        }
    }
}
```

### Ce que l'exhaustivité t'apporte

Neuf bras, aucun `_` fourre-tout. Si tu ajoutes un écran ou un événement, **le compilateur refuse de compiler** tant que tu n'as pas décidé du comportement de chaque nouvelle combinaison.

C'est la garantie qu'on ne peut pas obtenir en C avec un `switch` : là-bas, un cas oublié tombe silencieusement dans le `default` ou ne fait rien.

Sur une machine à états d'UI, les transitions oubliées sont exactement le genre de bug qui se manifeste comme « parfois l'appareil se bloque sur cet écran ».

### Le détail qui compte

`nb_messages.max(1)` évite une division par zéro dans le modulo quand la liste est vide. Sans ça, `index % 0` panique — et ton nœud redémarre parce que l'utilisateur a appuyé sur un bouton avant d'avoir reçu le premier message.

## 9.3 ★★★ — Historique persistant

```rust
#[derive(Clone, Copy)]
pub struct Entree {
    pub recu_a: u32,           // secondes depuis le boot
    pub taille: u8,
    pub rssi: i16,
    pub apercu: [u8; 24],
}

pub struct Historique {
    entrees: heapless::Vec<Entree, 20>,
}

impl Historique {
    pub fn ajouter(&mut self, e: Entree) {
        if self.entrees.is_full() {
            self.entrees.remove(0);   // O(n), acceptable pour n=20
        }
        let _ = self.entrees.push(e);
    }
}

pub fn age_relatif(recu_a: u32, maintenant: u32) -> heapless::String<16> {
    let d = maintenant.saturating_sub(recu_a);
    let mut s = heapless::String::new();
    let _ = if d < 60 {
        write!(s, "il y a {}s", d)
    } else if d < 3600 {
        write!(s, "il y a {}min", d / 60)
    } else {
        write!(s, "il y a {}h", d / 3600)
    };
    s
}
```

### La question à trancher : que faire survivre ?

C'est le vrai contenu de l'exercice, et il n'y a pas de réponse unique. Voici un raisonnement défendable :

| Donnée | Où | Pourquoi |
|---|---|---|
| Config (paramètres radio, préférences) | **Flash / EEPROM** | doit survivre à une coupure totale ; change rarement |
| Compteur de séquence | **RTC SRAM**, avec sauvegarde flash périodique | change souvent ; un saut après coupure est tolérable |
| Cache de déduplication | **RTC SRAM** | change constamment ; perdre le cache est sans conséquence |
| Historique des messages | **RTC SRAM** | confort utilisateur, pas critique |
| Budget de rapport cyclique | **RTC SRAM** | doit survivre au deep sleep, sinon tu peux dépasser la limite légale en te réveillant |

### Le principe

**Écrire en flash coûte de l'endurance** (~100 000 cycles par secteur) et **bloque le CPU**. La RTC SRAM est gratuite, illimitée en écriture, et survit au deep sleep — mais pas à une coupure d'alimentation.

La règle : *flash pour ce qui doit survivre à une batterie retirée, RTC SRAM pour tout le reste.*

Le budget de rapport cyclique est le cas le plus intéressant : si tu le perds à chaque réveil, un nœud qui dort et se réveille souvent pourrait émettre bien au-delà de 1 % sans le savoir. Il doit donc survivre au sommeil — mais s'il repart à zéro après un changement de batterie, c'est acceptable.

---

# Pour aller plus loin

Si tu as fait tous les ★★★, tu maîtrises l'essentiel du Rust embarqué utile à ce projet. Quelques directions :

**Sur le protocole** — implémenter la crypto d'identité MeshCore (Ed25519 + ECDH + AES-128) pour devenir un vrai client. C'est là que tu rencontreras `getrandom` pour de bon, et que tu devras brancher le générateur matériel du C6 en backend personnalisé.

**Sur la consommation** — mesurer le courant réel et implémenter un cycle de veille. Un nœud LoRa bien optimisé tient des mois sur batterie ; un nœud naïf tient trois jours.

**Sur les tests** — passer au vrai fuzzing avec `cargo-fuzz`, et ajouter des tests d'intégration qui rejouent des captures de trafic réel.

**Sur l'outillage** — mettre en place `defmt` + `probe-rs` pour un vrai débogage pas à pas, plutôt que du `println!`.

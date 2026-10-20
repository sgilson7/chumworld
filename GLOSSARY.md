# Glossary

The source of truth for what each word in this tool means. Strings, tests, the plan and the README use a term in the sense given here. The page shows a definition when a reader hovers the term.

Write each entry as a `### Term` heading, then one or two plain sentences, then an optional `Not:` line listing words that must not be used for it. A number a rule decides is a `{placeholder}` the core fills, so the glossary cannot disagree with the rules. Keep entries in alphabetical order. `crates/core/tests/copy.rs` checks the format, the placeholders and the `Not:` words; `TONE.md` governs the prose.

<!--
An example entry. It sits inside this comment, so it does not count.

### Spout

Where slush comes out. Each line has up to {line.spouts} spouts.

Not: tap, dispenser, nozzle
-->

### Bandage

Wraps a wound and gives back {bandage.heals} health. The merchant sells them.

### Boomerang

A weapon thrown from the free hand. It flies an arc up to {boomerang.reach} metres out and comes back. It lies in the chest at the bandit camp.

### Chest key

The key the bandit leader drops once the camp is cleared. It opens the chest at the camp.

### Coins

What the merchant takes and pays. Bandits carry them.

Not: gold, rupees, money

### Chop

A cut straight down from above the head: hold the left button and move the mouse down.

Not: overhead slash

### Cooldown

The wait after the imbue is cast before it can be cast again: {imbue.cooldown} seconds, counted from the cast.

Not: recharge

### Deflect

Turn one orb back with the edge of an imbued sword. Each orb the edge meets is turned once, back toward whoever cast it; a sword that is not imbued does nothing to an orb.

Not: parry, reflect

### Diagonal cut

A cut along a slant, from high on one side to low on the other or the other way: hold the left button and move the mouse clearly on a slant.

### Dodge-step

A short quick step to one side with Space and a movement key. You cannot dodge-step again straight away.

### Draw

Taking the sword out of its sheath with X. Drawn, the mouse moves the sword hand and the view holds on the nearest foe.

Not: unsheathe

### Equipment

The menu screen of what you wear and hold: the sword in your sword hand and the speedster's armour in its slots. Enter wears or holds the item chosen.

Not: gear screen

### Footing

How well your feet hold the ground. Each speedster piece worn keeps {speedster.grip_pct}% of the grip before it, so footing gets icier.

Not: traction, friction

### Free hand

The hand without the sword. It throws what a quick item readies.

Not: off hand

### Health

How much harm a body can take. A blade takes more the faster it moves when it hits.

Not: HP, hit points

### Imbue

An ability, not magic: Q imbues the sword for {imbue.seconds} seconds so it can deflect magic. The trainer teaches it.

Not: spell, buff

### Invert Y

A setting: moving the mouse down looks up, and moving it up looks down.

### Level cut

A cut across from one side to the other with the blade held flat: hold the left button and move the mouse sideways. It runs high, at the middle or low, by where the sword hand was.

Not: horizontal slash

### Lineup

Every character and prop stood in rows in front of the player, for looking at the models. It changes nothing in play and is off until asked for.

Not: gallery, showcase

### Look

Turning the view with the mouse. The camera stays over the player's right shoulder while it turns.

Not: free look, free camera

### Look speed

A setting: how far the view turns for a movement of the mouse, as a percentage.

### Magic

Shapes a caster throws at you in patterns, many at once. Each one that reaches you takes health. Learning magic is not part of this release.

Not: bullets, projectiles

### Map

The menu screen with the parchment map. It fills in where you have walked. M opens it.

Not: minimap

### Menu

The ring of four screens that Tab opens: Select Item, Map, Quest and Equipment. The world stops while it is open, and Q and E turn it.

Not: inventory

### Mouse sensitivity

A setting: how far the sword hand moves for a movement of the mouse, as a percentage.

### Orb

One shape of magic in flight. Each kind of orb has its own shape and its own colour, and one kind cannot be deflected.

Not: bullet, projectile, missile

### Pause

Stopping the world until you choose Resume. Esc, leaving the tab or letting go of the pointer pauses the game.

### Practice

The trainer's round that teaches the imbue. The trainer imbues your sword for it, and casts orbs for you to turn; turn enough and the imbue is yours.

Not: tutorial, training mode

### Quest

The menu screen for the imbue, your coins and what to do next.

Not: journal

### Quick item

An item put on a number key from Select Item. Pressing the key readies it in the free hand.

Not: hotbar

### Reach

How far from the shoulder the sword hand stretches. The wheel draws it in or sends it out.

### Rising cut

A cut from low in front up over the shoulder: hold the left button and move the mouse up.

### Rock

A throwable from the ground. One that lands on the merchant's cart stops the cart.

Not: stone

### Run

Moving at {run.speed} metres a second while Shift is held with a movement key.

### Select Item

The menu screen of what you carry for the free hand. Choose an item and press a number key to put it on that key.

Not: inventory, bag

### Settings

Look speed, mouse sensitivity, invert Y and volume. They last until you close the page.

Not: options

### Sheathe

Putting the sword back on the left hip with X. Sheathed, the mouse looks and Shift runs.

Not: holster

### Shop

The merchant's cart, once stopped. The merchant sells at an item's value and buys at {merchant.buys_pct}% of it.

Not: store

### Short sword

The sword you start with: {short_sword.blade} cm of blade and no bonus.

### Speedster's armour

Armour in {speedster.pieces} pieces, one for each slot, each found its own way. Each piece worn adds {speedster.speed_pct}% speed and makes footing icier.

Not: set bonus

### Speedster's boots

The speedster's armour for the feet. They wait at the river shrine.

Not: shoes

### Speedster's gloves

The speedster's armour for the hands. The bandit lookout wears them.

Not: gauntlets

### Speedster's hood

The speedster's armour for the head. The merchant sells it.

Not: helmet, hat

### Speedster's jacket

The speedster's armour for the body. It hangs on a scarecrow.

Not: coat, chestplate

### Speedster's trousers

The speedster's armour for the legs. They are hidden in a hollow tree.

Not: pants, leggings

### Swing

Holding the left button while you move the mouse. The way the mouse goes picks the cut: a level cut, a chop, a rising cut, a diagonal cut or a thrust. Once picked, the cut draws back, swings through and carries on past, and the mouse waits until it is over.

### Swing cue

A setting: while the left button is held, a faint line of beads shows where the sword's point will run, and the cut's name shows below the middle of the view.

### Sword

The normal sword, {sword.blade} cm of blade. Its weight lets a hard blade-on-blade hit knock a foe's weapon loose. It is won from the bandit leader.

Not: longsword

### Sword hand

The hand that holds the sword. The other is the free hand.

Not: main hand

### Throw

Sending what the free hand holds ready where you look, with F. The free hand winds back first.

### Throwing knife

A small blade thrown from the free hand.

Not: dagger

### Thrust

A straight stab ahead: hold the left button and turn the wheel away from you.

### Volume

A setting: how loud the game plays, as a percentage. No sound plays yet.

### Walk

Moving at {walk.speed} metres a second with W, A, S or D, the way the camera faces.

### WebGL 2

The browser's interface for drawing with the graphics card. The game draws every frame with it.

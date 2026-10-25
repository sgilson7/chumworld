# Glossary

The source of truth for what each word in this tool means. Strings, tests, the plan and the README use a term in the sense given here. The page shows a definition when a reader hovers the term.

Write each entry as a `### Term` heading, then one or two plain sentences, then an optional `Not:` line listing words that must not be used for it. A number a rule decides is a `{placeholder}` the core fills, so the glossary cannot disagree with the rules. Keep entries in alphabetical order. `crates/core/tests/copy.rs` checks the format, the placeholders and the `Not:` words; `TONE.md` governs the prose.

<!--
An example entry. It sits inside this comment, so it does not count.

### Spout

Where slush comes out. Each line has up to {line.spouts} spouts.

Not: tap, dispenser, nozzle
-->

### Ambush

Bandits hidden behind a tree or a rock beside a trail. They break from cover when you pass close.

Not: trap

### Ant King

The great tree's master, behind the gate at its crown: a giant ant at the foot of a cone of sliding platforms, with five plates of armour on its front. Hook a plate, hang on, and tear it off with W, A, S and D.

### Arena

A room in the great tree's trunk where attack ants wait. Roots hold its doors shut until every one is dead; cleared, it stays cleared.

### Armourer

The town's trader in arms, at a red stall by the lane out of the market. Sells the hammer and the far-step boots.

Not: blacksmith, smith

### Attack ant

An ant on two legs with a blade in each of its two hands, met in the great tree's arenas. It walks at you, winds one blade back, and slashes left, right, left; step out of its reach or raise the shield. Four cuts kill it.

### Autosave

The game saving itself in the browser: when you leave the village, when a waystone is first woken, when a quest step is done, when you walk into or out of a place, and after {autosave.minutes} minutes of play. Never mid-duel or while someone is talking. A small Saved mark shows it.

### Bandage

Wraps a wound and gives back {bandage.heals} health. The merchant sells them.

### Bandit lean-to

A small bandit site off a way, a tent and a fire with a lookout. Once seen, the map marks it.

### Bandits' camp

A bigger camp than a lean-to, on the way north-west toward the town in the bowl: two tents round a fire, a lookout and two bandits. Beat all three for what they keep.

### Blood

What a hit sheds: faceted drops that fly from the cut and mark the ground, then fade. A setting turns it off.

Not: claret

### Boomerang

A weapon thrown from the free hand. It flies an arc up to {boomerang.reach} metres out and comes back. It lies in the chest at the bandit camp.

### Bounty

Coins the town's council pays for work it posts on its notice board or its councillors tell of. Hearing of it starts its quest line; once the work is done, the council's clerk pays.

Not: reward, contract

### Chitin shield

The shield ant's shield, worn on the left arm. Hold C to raise it: blows and magic from in front stop against it, while you walk and cannot swing.

### Chest key

The key the bandit leader drops once the camp is cleared. It opens the chest at the camp.

### Chump stick

A short staff with a gold band, proof of a dungeon done. The Ant King leaves the first; enough of them open the end game.

### Chasm of No Return

The drop under the ravine's rope bridge, at the gorge's foot. A fall into it sends you back to your last save; the way down, by the rope ladder at the bridge's middle, is not open yet.

Not: pit, abyss

### Clearing

An open space in the forest round the camp with a stump and a fallen log. Some hold one or two foes, the rest animals resting. A clearing with foes goes on the map once you have seen it.

Not: grove, glen

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

A short quick step to one side with Space and a movement key. You cannot dodge-step again straight away. Held on a foe or the vine, it goes half as far again.

### Draw

Taking the sword out of its sheath with X. Drawn, the mouse moves the sword hand and the view holds on the nearest foe.

Not: unsheathe

### Dungeon key

A key a dungeon's guardian leaves when it dies. You keep it from the moment you take it, through a fall or a load. Two of them open the great tree's centre door.

Not: boss key

### Equipment

The menu screen of what you wear and hold: the sword in your sword hand and the speedster's armour in its slots. Enter wears or holds the item chosen.

Not: gear screen

### Far-step boots

The armourer's sprung boots, worn on the feet. Worn, the dodge-step goes {boots.pct} percent as far.

### Flute

The trainer's gift once the practice is passed. Played from a quick key or from Select Item, it opens a staff: the arrows and Space play its {flute.keys} notes, and the travel song takes you to a waystone you have woken.

Not: ocarina, whistle, pipe

### Footing

How well your feet hold the ground. Each speedster piece worn keeps {speedster.grip_pct}% of the grip before it, so footing gets icier.

Not: traction, friction

### Free hand

The hand without the sword. It throws what a quick item readies.

Not: off hand

### Great well

Where the river falls into the earth north of the orcs' village: a deep pit, the water pouring from a spout on a stone well-head, and a door on a ledge behind it. A waystone stands at the foot of the run-up.

### Hamlet

A few farms round a yard and a well, out on the plains: the west hamlet past the river, the east hamlet among the low hills to the south-east. Their people talk.

### Heart

What the Ant King leaves where it falls. Taken, it adds {heart.adds} to your most health and fills the bar.

Not: heart container, life

### Hammer

The armourer's heavy two-handed weapon, held in the sword hand. Slow to swing; a blow of {hammer.knock} m/s or more knocks a foe off their feet for a moment.

Not: mallet, maul

### Health

How much harm a body can take. A blade takes more the faster it moves when it hits.

Not: HP, hit points

### Hook plate

A gold plate set in a dungeon's wall. Only a hook plate takes the hookshot's hook.

### Hookshot

A hook on a chain, from the great tree's shrine. Put it on a quick key, hold the key to aim, and click to fire: caught in a hook plate within {hookshot.reach} metres, it pulls you there and you hang until you let the key go.

### Imbue

An ability, not magic: Q imbues the sword for {imbue.seconds} seconds so it can deflect magic. The trainer teaches it.

Not: spell, buff

### Invert Y

A setting: moving the mouse down looks up, and moving it up looks down.

### Lashing vine

A vine on a dungeon's floor whose whip turns round its root. It stings and staggers you if it passes over you; time your way between its passes. It cannot be cut.

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

Shapes a caster throws at you in patterns, many at once. Each one that reaches you takes health. Your own magic is the slowing orb, cast for mana.

Not: bullets, projectiles

### Mana

What your magic is cast with, shown as {mana.pips} pips under your health once you have the slowing orb. A spent pip comes back after {mana.refill} seconds.

Not: MP, energy

### Mending vest

The medic's vest, worn on the body. Worn, it mends a health every {vest.seconds} seconds, in a fight or out.

### Map

The menu screen with the parchment map. It fills in where you have walked. M opens it.

Not: minimap

### Medic

The town's healer, at a blue stall by the lane out of the market. Sells bandages and the mending vest.

Not: doctor

### Menu

The ring of four screens that Tab opens: Select Item, Map, Quest and Equipment. The world stops while it is open, and Q and E turn it.

Not: inventory

### Mouse sensitivity

A setting: how far the sword hand moves for a movement of the mouse, as a percentage.

### Notice board

A board on the town's market where the council posts its notices. Press E by it to read them; a notice can start a quest line.

### Orcs' village

The orcs' home on the low hills south of the square, round a great well that has run dry. On the map once walked near.

### Orc fire

A small fire off a way where two orcs sit. Once seen, the map marks it.

### Orc watch post

A fire and a tent on the old south road, down toward the orcs' village, kept by two orcs. Once seen, the map marks it.

### Orb

One shape of magic in flight. Each kind of orb has its own shape and its own colour, and one kind cannot be deflected.

Not: bullet, projectile, missile

### Padlock

A lock on a dungeon's door. E with its dungeon key opens it; the door leads on once every padlock is open.

### Pause

Stopping the world until you choose Resume. Esc, leaving the tab or letting go of the pointer pauses the game.

### Practice

The trainer's round that teaches the imbue. The trainer imbues your sword for it, and casts orbs for you to turn; turn enough and the imbue is yours.

Not: tutorial, training mode

### Quest

The menu screen for the imbue, your coins and what to do next.

Not: journal

### Quest line

Something to go after that a rumour started: the speedster's armour, the leader's sword, the bandits' chest, the trainer, the waystones. The Quest screen lists each with what you last heard of it and how far you are; it moves on as its items come to hand and is done once they all have.

Not: quest log, mission

### Quick item

An item put on a number key from Select Item. Pressing the key readies it in the free hand.

Not: hotbar

### Reach

How far from the shoulder the sword hand stretches. The wheel draws it in or sends it out.

### Rising cut

A cut from low in front up over the shoulder: hold the left button and move the mouse up.

### Rabid fox

A fox frothing at the jaws on the pass's floor behind the ravine's gate; there are two. It runs at you, crouches, and leaps to bite. It hops clear of every cut until the slowing orb slows it; slowed, it falls to a few cuts.

Not: wolf, dog

### Rock

A throwable from the ground. One that lands on the merchant's cart stops the cart.

Not: stone

### Rope bridge

The bridge of planks and rope across the ravine's gorge, from the ledge at the pass's end to the far ledge. It sags to its middle, where a rope ladder hangs over the drop; its hand ropes keep you on it.

Not: walkway, catwalk

### Rumour

What a person tells you about where something is or how to get it. Talk to someone with E and they tell you one rumour; talk again for the next, until they have no more to tell. A rumour starts or moves on a quest line, and may mark a place on the map.

Not: hint, gossip

### Run

Moving at {run.speed} metres a second while Shift is held with a movement key.

### Save

Your progress kept in this browser: what you carry and wear, your coins, the quest, the waystones woken, the foes beaten and your save point. Download save writes the same save as a file, and Load save file reads one back.

### Save point

Where a save stands you: each waystone once woken, and a lantern post at the crossroads, on the camp trail and on the town's market. Press E by one to save. After a fall you come to at your last save point.

Not: checkpoint

### Select Item

The menu screen of what you carry for the free hand. Choose an item and press a number key to put it on that key.

Not: inventory, bag

### Settings

Look speed, mouse sensitivity, invert Y and volume. They last until you close the page.

Not: options

### Shears

A pair of shears the Ant King leaves, held in the sword hand from Equipment: two blades of {shears.blade} cm, light and short. Swung, they snip a thicket, which no sword cuts; they fight as a light blade.

Not: scissors, clippers

### Sheathe

Putting the sword back on the left hip with X. Sheathed, the mouse looks and Shift runs.

Not: holster

### Shop

The merchant's cart, once stopped. The merchant sells at an item's value and buys at {merchant.buys_pct}% of it.

Not: store

### Shield ant

The guardian of the great tree's right roots: an ant standing upright behind a shield, which it turns to face you and bashes with. Get round the shield, or cut over it, and hit its flesh three times.

### Short sword

The sword you start with: {short_sword.blade} cm of blade and no bonus.

### Slowing orb

Your first magic, left by the Ant King. Its quick key casts it for a pip of mana: it flies along your view, or at the foe you hold on, as far as {orb.reach} metres, and the first foe it meets moves at {orb.slow_pct} percent of its speed, feet and sword hand, for {orb.seconds} seconds.

### Small chest

A little chest on a pillar or a ledge in a dungeon. E opens it once for what it holds: coins, or rocks.

### Song of strafing

A song for the flute, written in the lid of a small chest in the great tree, {strafing.notes} notes long. Played, even in a fight, for {strafing.seconds} seconds your steps to the side with the sword drawn go {strafing.pct} percent faster; walking, running and sprinting stay as they were.

Not: speed song, dance

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

### Sprint

A run that holds itself: tap Shift while moving to start it, and tap it again to stop. After {sprint.after} s it builds over {sprint.ramp} s to {sprint.top} times the run speed. Stopping, drawing the sword, a blow, a dodge-step or a hard turn ends it.

Not: dash, boost, turbo

### Swing

Holding the left button while you move the mouse. The way the mouse goes picks the cut: a level cut, a chop, a rising cut, a diagonal cut or a thrust. Once picked, the cut draws back, swings through and carries on past, and the mouse waits until it is over.

### Swing cue

A setting: while the left button is held, a thin glowing line shows where the sword's point will run, ending in a bead, and the cut's name shows below the middle of the view. Each blade in a cut leaves a short glowing trail while the setting is on.

### Sword

The normal sword, {sword.blade} cm of blade. Its weight lets a hard blade-on-blade hit knock a foe's weapon loose. It is won from the bandit leader.

Not: longsword

### Sword hand

The hand that holds the sword. The other is the free hand.

Not: main hand

### Thicket

A knot of vine grown over a small chest in the great tree. Nothing gets through it to the chest until the shears snip it; a sword only stops in it.

Not: hedge, briar

### Throw

Sending what the free hand holds ready where you look, with F. The free hand winds back first.

### Throwing knife

A small blade thrown from the free hand.

Not: dagger

### Thrust

A straight stab ahead: hold the left button and turn the wheel away from you.

### Town

The town in the bowl north-west of the square: a market round the pond, the council's hall, the inn and the mills, the ravine's gate in the pass behind it. On the map once walked near.

Not: city

### Travel song

The song the trainer teaches with the flute, {song.notes} notes long. Played, it shows the waystones you have woken on the map; choose one and the view fades and you stand beside it.

Not: warp song

### Traveller

Waits with a pack mule at the crossroads. For {ride.least} to {ride.most} coins, more for farther, the traveller takes you to a waystone you have not yet been near and sets you down beside it, woken.

### Vine

The guardian of the great tree's left roots. It lunges its neck at you; dodge-step aside, and cut the neck while it lies stretched past you. Three cuts kill it. Nothing else hurts it.

### Vine circlet

A head piece of the vine's green. Worn, the hookshot takes hold of a foe: it pulls you to them and stuns them, then needs {circlet.wait} seconds before it will again. With the boomerang ready and the sword away, it lets the boomerang lock on a foe and fly to them.

### Volume

A setting: how loud the game plays, as a percentage. No sound plays yet.

### Walk

Moving at {walk.speed} metres a second with W, A, S or D, the way the camera faces.

### Waystone

A carved standing slab at a travel spot: by the village, by the bridge, on the trainer's hill, at the shrine, below the bandit camp, on the great tree's crown, on the town's market and by the great well. It sleeps until you press E beside it; woken, its marks glow and the map shows it.

Not: warp point, teleporter

### Windlass

A crank and drum on the pass's floor behind the ravine's gate. Once the rabid foxes are beaten, press E by it to lift the gate's bar from inside; the gate swings open and stays open.

Not: winch, lever

### WebGL 2

The browser's interface for drawing with the graphics card. The game draws every frame with it.

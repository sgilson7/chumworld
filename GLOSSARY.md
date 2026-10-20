# Glossary

The source of truth for what each word in this tool means. Strings, tests, the plan and the README use a term in the sense given here. The page shows a definition when a reader hovers the term.

Write each entry as a `### Term` heading, then one or two plain sentences, then an optional `Not:` line listing words that must not be used for it. A number a rule decides is a `{placeholder}` the core fills, so the glossary cannot disagree with the rules. Keep entries in alphabetical order. `crates/core/tests/copy.rs` checks the format, the placeholders and the `Not:` words; `TONE.md` governs the prose.

<!--
An example entry. It sits inside this comment, so it does not count.

### Spout

Where slush comes out. Each line has up to {line.spouts} spouts.

Not: tap, dispenser, nozzle
-->

### Dodge-step

A short quick step to one side with Space and a movement key. You cannot dodge-step again straight away.

### Draw

Taking the sword out of its sheath with X. Drawn, the mouse moves the sword hand and the view holds on the nearest foe.

Not: unsheathe

### Health

How much more you can take. A blow takes health by how fast the blade was moving when it landed; at none you fall.

### Lineup

Every character and prop stood in rows in front of the player, for looking at the models. It changes nothing in play and is off until asked for.

Not: gallery, showcase

### Look

Turning the view with the mouse. The camera stays over the player's right shoulder while it turns.

Not: free look, free camera

### Pause

Stopping the world until you choose Resume. Esc, leaving the tab or letting go of the pointer pauses the game.

### Reach

How far from the shoulder the sword hand stretches. The wheel draws it in or sends it out.

### Run

Moving at {run.speed} metres a second while Shift is held with a movement key.

### Sheathe

Putting the sword back on the left hip with X. Sheathed, the mouse looks and Shift runs.

Not: holster

### Swing

Holding the left button while you move the mouse. The arm pushes harder, so the sword moves faster and hits harder.

### Sword hand

The hand that holds the sword, which the mouse moves while the sword is drawn. A small light shows where it is going.

### Throw

Sending a rock or a knife where you look with F. The free hand winds back first.

### Walk

Moving at {walk.speed} metres a second with W, A, S or D, the way the camera faces.

### WebGL 2

The browser's interface for drawing with the graphics card. The game draws every frame with it.

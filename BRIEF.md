# Brief

Stage 1. The person's own words come first; the rest can be written by the agent from them and read by the person. `C1 Frame the problem and set the rules`, `B1 Establish context`.

## The description, word for word

> get yourself familiarized with the builder repository and clone it from https://github.com/sgilson7/builder-setting into this repo then remove the .git. You will be using builder setting to help constrain and direct your actions as you help act as as a video game  creator and co-designer / developer, with the goal of deploying it to a github pages so its freely browser accessible, with an emphasis on optimization for running in the browser.  Please also see https://sgilson7.github.io/projects/ for examples of other browser based tools / games built in builder like the PDF redactor, youtube to mp3, long time coming, extremely lifelike fishing  etc. Also see ptonkle.github.io for games built using builder by my good friend. First thing is I want you to do is interpret builder, and write an analysis of how you will use it to help you develop this game, and write into the first tab of the design journal, the design journal process is described in builder; when you write the design journal, use the local ape-race directories design journal as a representative of a good journaling process. you may access the other builder made games within the documents directory on this host machine. the game I want to build is a 3D open world exploration / combat game inspired by kenshi for its openworld, and using vagrancy's fully controllable physics simulated arms to control a sword being held by those arms, and most combat takes the form of 1v1 sword duels in the open world, where all the enemies also use vagrancy type arms, weapons, throwables, and behavior trees. for the gameplay, I want to use blade symphony's style and wasd movement + mouse combination as an inspiration for this game, but with the vagrancy arm physics. For the game plot and context, I want the game starting with you being awoken to your village being raided by orcs, and if you are killed by one, you wake up back in bed and think huh what a funny dream, the correct behavior is to run away from the village, and that is how the game starts, so after you run away you are on your own; so the initial plains in which your village lies must be made as an mvp, alongside the village, one little forest encapment of bandits you can fight, and a shopkeeper being pulled by a donkey that you can stop by flinging something at their cart, then which you can buy stuff. all the items in the game should be kept in a ocarina of time / majoras mask style inventory menu, and map menu as well. every item in the game in terms of weapons and armor should be unique in the way in which you acquire them (i.e defeating this encampment will drop a key for a chest that opens to a boomerang), and each weapon and armor piece should have a unique mechnical use, like a boomerang which flies in an arc when thrown and always come back, to pants that let you go invisible by standing still, to boots of sprinting that double your speed but make you run on ice physics, etc. there should be 1 starting short sword you have, 1 normal sized sword, and 1 boomerang in the mvp, and your starting gear gives no bonuses, and you should be able to find the individual pieces (like original fable) of armor pieces, and you should be able to find the speedsters armor, that for each piece you put on, makes you go 20% faster put applies a worse and worse stacking ice physics debuff to make movement really hard. any time magic is cast in this game, it should be like a bullet pattern from long time comin, and be represented by various multi-colored and multi-shaped polygons / spheres traveling in patterns on the screen, so that you can deflect individual orbs with your sword if you have a sword / ability that lets you do that. so add 1 spell in the mvp that imbues your sword to deflect magic for 10 seconds, with a 20 second cooldown. you can gain access to magic but need to be taught formally, and wont be in the mvp; all you get access to is the one ability, seperate to magic, that only uses cooldowns. use the style of sgilson7 protection of the spheres when designing the world and 3d assets, and use any tech from iron vector for making the world have a really large draw distance; i want the game to have a huge draw distance, with dithering / simplification in the distance like crimson desert does in order to improve performance when drawing the landscape. so start with the design log tab on builder, then a tab interpreting all of what i have discussed here one at a time, with a mockup / map / flow diagram for each point to help me inspect what you will build.

## The tool

A 3D open-world exploration and sword-duel game that runs in a desktop browser from GitHub Pages. The player controls a physics-simulated arm holding a sword with the mouse while moving with WASD; enemies use the same arms, weapons, throwables and behaviour trees. The first release (the MVP) is one region: the plains around the player's village, the village during an orc raid, a bandit camp in a forest, and a travelling merchant with a donkey cart. The design journal's "Your description" page reads each point back with a picture; the questions there (CW-Q1 …) settle what this paragraph leaves open.

## Done means

Draft conditions, to be settled with the person through the journal's questions before `PLAN.md`.

- The game opens in bed during the raid. Death to an orc returns the player to bed with the dream line; leaving the village boundary ends the opening and the player is free.
- The player can move with WASD and steer the sword with the mouse through a simulated arm, and a 1v1 duel with an orc or a bandit resolves through blade and body contact, not dice rolls.
- Enemies duel with the same arm model, carry weapons, throw objects, and pick actions from behaviour trees.
- The plains, village, bandit camp and merchant route exist and can be walked between without a loading screen.
- A throwable that hits the merchant's cart stops it and opens the shop.
- The items are the short sword (start, no bonus), a normal sword, a boomerang (arc flight, always returns) and the speedster's armour pieces, each acquired in its own way, and all shown in an Ocarina-of-Time-style inventory menu beside a map menu.
- The camera is over the shoulder, third person, at all times.
- The deflect (imbue) is not known at the start: a trainer teaches it and makes the player practise it with them before the player can use it alone.
- The water uses a solver like Extremely Lifelike Fishing's.
- Tests show there is no flickering: no z-fighting, no popping between detail levels, no frame-to-frame shimmer on a still or slowly moving view.
- The look and feel are clean and complete to the standard of Ape Race and Protection of the Spheres.
- Each speedster piece worn adds 20% to movement speed and makes footing more like ice, worse with each piece.
- The sword-imbue ability lasts 10 seconds, has a 20 second cooldown, and lets the sword deflect individual magic projectiles, which fly in bullet patterns of coloured shapes.
- The landscape is visible to a large distance, with distant terrain simplified and dithered, and the game holds its frame rate on the person's machine (target in `MEASUREMENTS.md`).
- `./scripts/check.sh` passes, and the game is published on GitHub Pages once the person grants it.

## Decisions that stay with the person

- The game's name, and the names of the village, places and characters (agent-generated lists are worklist rows).
- Which items and armour pieces exist beyond the MVP, and how each is acquired.
- Whether the game keeps a save between visits (D17 forbids browser storage unless this brief allows it).
- The look, from Protection of the Spheres; whether a picture matches it is the person's judgement.
- Any third-party asset, sound or likeness.

## Accessibility

Magic projectiles differ by shape as well as colour. Health, cooldown and the ice debuff are shown as bars with text, not colour alone. Input: keyboard and mouse on a desktop browser with pointer lock; phones are out of scope for the MVP unless the person says otherwise.

## Style

Protection of the Spheres (sgilson7), as read in the journal's style entry. The 2D art pipeline's default pack is not used for the world.

## Words

Terms from the description: *duel*, *arm* (the simulated arm), *throwable*, *behaviour tree*, *imbue* (the ability), *cooldown*, *speedster's armour*, *ice physics* (the footing debuff), *bullet pattern*, *deflect*, *draw distance*, *dithering*. They go into `GLOSSARY.md` before any string uses them.

## Autonomy

The person's grant, word for word (2026-10-20):

> publish to a public github repo, and a public github pages for the deployment, i give approval for all make deploy, now follow the journal to build the mvp

The plan is approved in advance. The agent may create the public repository `sgilson7/chumworld`, push to it, and publish it to GitHub Pages at `https://sgilson7.github.io/chumworld/` with `./scripts/publish.sh` once `./scripts/check.sh` passes, as often as each milestone warrants. Where this brief is silent, the agent decides, records the decision in `SECOND-ORDER.md`, and continues. The agent may not loosen a Builder requirement or a boundary test, and may not add a server, an account, analytics or any off-origin request. The work is split among subagents, each bound by this project's `CLAUDE.md`; the main agent coordinates, reviews their work and keeps the design journal.

## Journal

`default`

## Allowed core dependencies

`serde`, `serde_json` (reference default).

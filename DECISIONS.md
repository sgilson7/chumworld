# Decisions

Dated entries. Each quotes the person's request word for word, then says what the agent decided and where it is recorded. Grants of autonomy go here too ("you may publish"), with their scope: this run only, or until withdrawn.

## YYYY-MM-DD

> The person's words.

The agent decided:

## 2026-10-20 (after playing the published build)

> looking great so far, there should be roads in and out of the village that you run a lil faster along, and generally your overall movement speed should be faster. also, the pop-in on things that are close like grass and assets like that is too much, the game looks really weird to walk around cause things are always popping in, theres never a moment where all the grass within the immediate viewable area with your eyes is rendered

The agent decided: triage rows T6 (roads out of the village, faster on a road), T7 (walk about 3 m/s, run about 7 m/s, speedster's 20% over the new base) and T8 (everything within 25–30 m always drawn, fading per instance beyond). All three go to the M7 land part (`m7-land`), which now owns movement speed. Journal entry `2026-10-20-t6`.

> also there needs to be a lot more visual indicator / gameplay reaction to hitting enemies, like they / the body part should flash red, there can be some blood, if enough damage is done then they loose the limb and start crawling / moving weirdly, etc

The agent decided: triage row T9. This reverses the earlier answer to CW-Q12 ("for now keep limb cutting out, we'll add it later"); the person now asks for it, so CW-16 is rewritten in `CLAUDE.md` and `AGENTS.md`. It is built by a new M7 part, "wounds" (`m7-wounds`), which also takes the hit flash over from the feel part. It applies to every body, the player's included; R9.120 records how the player loses a limb and what the dream does with it. Journal entry `2026-10-20-t9`.

> also the combat should be made easier to decide between horizontal and vertical swings, right now all i do are these like limp diagonal slashes

The agent decided: triage row T10, built by a fifth M7 part, "swings" (`m7-swings`), which owns how the mouse and the swing button become a cut (the hand's target and the swing's commitment in `player.rs` and `arm.rs`). Journal entry `2026-10-20-t10`.

> you should definitly be able to get an instrument you can play to return to fast travel points like ocarina of time / majoras mask, or certain npcs can fast travel you to far off places for the first time for a price

The agent decided: triage row T11, built by a sixth M7 part, "travel" (`m7-travel`). It owns the instrument, the travel points, the song screen and the paid traveller. Journal entry `2026-10-20-t11`.

> make sure there is stuff to do see roads on the sides of the village you are most likely to run out of at theb eginning of the game

The agent decided: triage row T12, built by a seventh M7 part, "roadside" (`m7-roadside`). The likely exits come from the opening's own data (`data/story.json`): the raid enters from the south, and the four villagers flee north-west, north-east, north and west. The roadside part places its stops along the roads the land part (P2) lays for T6 on those sides; the land part is told which sides, and the travel part (P6) puts the village-edge stone on the north road. Journal entry `2026-10-20-t12`.

> the grass should be rendered way farther into the distance, like it should appear in some way everywhere when its there, it still dissappears wayy to close to you; onstead of the ground in the distance being flat, it should have some grass rippling / moving texture maybe using some tech from extremely lifelike fishing

The agent decided: triage row T13, added to the land part (`m7-land`), which already owns grass reach and pop-in (T8). The fishing game has no grass; the technique taken from it is its water's wind-like motion: a few summed travelling waves and a periodic noise whose drift is sent wrapped (R4.9). Here it becomes one wind field shared by the near blades and a grass shading on the far terrain, so gusts roll across the whole field. Journal entry `2026-10-20-t13`.

> there needs to be a bit more in the overworld in between these major areas, or you need some way to move a lot faster than you currently move. i would say that you should be able to set yourself to sprinting, and if you've been sprinting for 5seconds you start to go faster and faster until you are running at triple speed. plus, add more random enemies in the overworld, and add areas to save your game to browser / to a save file json so you stop reviving in the village, the game should auto save as well in the browser.

The agent decided: three triage rows. T14, the sprint that builds to triple speed, goes to a new part, "sprint" (`m7-sprint`), which builds over whatever run speed the land part sets. T15, more enemies between the major areas, goes to "wanderers" (`m7-wanderers`). T16, saving, goes to "save" (`m7-save`). The person's earlier answer to CW-Q3 ("go with defaults", so no save) is reversed by this request. Builder's D17 (no browser storage) is a default the brief may change, so the brief now allows one save in the browser and a JSON save file, CW-12 is rewritten in `CLAUDE.md` and `AGENTS.md`, and the change is entered in `BUILDER-SPEC.md`'s table of changed defaults. The core decides what a save holds and checks a loaded one; the page only stores and reads the core's text. Journal entry `2026-10-20-t14`.

> you should also be able to talk to people to get rumors / start quest lines, so you should be able to talk to some people who point out the speedsters outfit, where to get the long sword, boomerang, imbue, etc

The agent decided: triage row T17, built by a new part, "rumours" (`m7-rumours`), on main after the roadside part merged, because the roadside villagers already point the way and become the first rumour-tellers. "The long sword" is read as the sword the bandit leader carries, won in a duel (`data/items.json`: `sword`, route `leader_duel`). Quest lines join the Quest screen; P10's save is told so it can keep them. Journal entry `2026-10-20-t17`.

## 2026-10-21

> override the rule just this once and deploy live

> you do the push

The agent decided: main 7eb7d8e was pushed to GitHub Pages (gh-pages d1d4342) with `./scripts/check.sh` steps 1–8 passed and step 9, the browser check, still running. The waiver is for this deploy only: CW-14 (publish after `check.sh` passes) stands for every later one. The browser check's result is recorded when it finishes.

> change the sprint time to 4 seconds, and make it go up to 5x, and the speedster can make you go 10x. also the bandit camp is really hard to find in the woods right now without the waypoint, and the rest of the woods are completely barren. add a few extra clearings with an enemy or two to fight / wildlife stitting there in the forest, and add a little campfire or big flag they have raised in order to help find the bandit camp. also spruce up the rest of the woods with some wandering animals, foliage, different types of trees, etc. feel free to look at other games in the directory tab for ideas / assets to use

The agent decided: two triage rows. T18 (the sprint's numbers) goes to a small part, "sprint2" (`m7-sprint2`). "The speedster can make you go 10x" is read as the pieces multiplying the sprint, so the top is 5× bare and 10× in all five pieces (1 + 5 × 20% = 2); CW-8's 20% a piece is unchanged. At 70 m/s a body moves 1.2 m a tick, so the part must show nothing tunnels through a wall. T19 (the forest) goes to "forest" (`m7-forest`). The person's "other games in the directory" is read as the game folders in ~/Documents. The part may look at any of them for ideas, but may copy code or assets only from projects whose own files show the person as the author, each recorded in `LICENSES.md`; anything else is a person row (BUILDER-SPEC: a third-party asset is the person's decision). It reads nothing outside the game folders. The earlier rule that the wanderers use only existing kinds does not bind the forest: animals are asked for. Journal entry `2026-10-21-t18`.

> ok, now, without any subagent processes (cause they're destroying my usage rn), read the transcript I attached from me today describing a dungeon and more stuff, then write a tab in the design journal detailing your interprettion of every point, with mockup images, flow diagrams, maps, animations, assets, etc. don't write any code for the game yet besides what helps you create these mockups

The agent decided: no subagents for this round. The transcript (`../dungeons_transcript.vtt`) is read back in a new journal tab, "Dungeons", with 20 hand-made SVG mockups (four of them animated) and questions CW-Q15 to CW-Q33. The first dungeon is designed in full, following the person's "for now, I just want to work on the first one"; the rest is sketched. No game code changed. The names the person gave (the Ant King, the chump stick, the Chasm of No Return, Chumbles) are recorded for CW-20; the tree's name waits on CW-Q15. Triage row T20.

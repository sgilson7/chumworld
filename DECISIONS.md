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

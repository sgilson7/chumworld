"""The project's own browser interactions, run by check.py in every engine.

check.py has already loaded the page, waited for it to report ready, and let
it idle. This opens the game again at the URL a reader can open with
`?pointer=free` (the realtime kit's mode without pointer lock, since a driven
browser may grant the lock and drop it at once), then plays it the way a
person would: reads the title, clicks Play, stands still, turns, presses Esc.

Expected values come from data/strings.json and from the brief, never from
the game's own output. Waits are on selectors, data-* attributes and display
frames the page draws, never on a fixed time: on CI the page draws with a CPU.

The flicker checks (T3, CW-18) read the canvas's pixels with readPixels in
a requestAnimationFrame callback queued after the game's own, so they see
each frame as it was drawn. That code runs here, in the check; the game's
code is not changed for it.
"""

import json
import pathlib
import sys

# M7: the layout checks beside this file (tests/browser/layout.py)
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

ROOT = pathlib.Path(__file__).resolve().parents[2]
STRINGS = json.loads((ROOT / "data" / "strings.json").read_text())

# Waits for `frames` display frames and at least `ms` of the page's clock.
WAIT_FRAMES = """([frames, ms]) => new Promise((done) => {
    const start = performance.now();
    let n = 0;
    const step = (now) => {
        n += 1;
        if (n >= frames && now - start >= ms) done(n); else requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
})"""

# Reads `count` consecutive frames' pixels, dispatching `px` of mouse
# movement before each; returns for each frame a hash, the number of
# distinct colours among sampled pixels, and the pixels themselves when
# `keep` is set (for comparing three frames).
GRAB = """([count, px, keep]) => new Promise((done) => {
    const gl = document.getElementById('view').getContext('webgl2');
    const w = gl.drawingBufferWidth, h = gl.drawingBufferHeight;
    const out = [];
    const grab = () => {
        const p = new Uint8Array(w * h * 4);
        gl.readPixels(0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, p);
        let hash = 2166136261;
        for (let i = 0; i < p.length; i++) { hash ^= p[i]; hash = Math.imul(hash, 16777619) >>> 0; }
        const colours = new Set();
        for (let i = 0; i < p.length; i += 4 * 997) colours.add((p[i] << 16) | (p[i + 1] << 8) | p[i + 2]);
        out.push({hash, colours: colours.size, pixels: keep ? p : null});
        if (out.length === count) {
            if (!keep) { done({w, h, frames: out}); return; }
            // A-B-A: a pixel that changes by more than a step and changes back
            const [a, b, c] = out.map((f) => f.pixels);
            let flips = 0;
            const step = 24, back = 6;
            for (let i = 0; i < a.length; i += 4) {
                let ab = 0, ac = 0;
                for (let k = 0; k < 3; k++) {
                    ab = Math.max(ab, Math.abs(a[i + k] - b[i + k]));
                    ac = Math.max(ac, Math.abs(a[i + k] - c[i + k]));
                }
                if (ab > step && ac <= back && Math.abs(b[i] - c[i]) > step) flips += 1;
            }
            done({w, h, frames: out.map((f) => ({hash: f.hash, colours: f.colours})), flips, pixels: w * h});
            return;
        }
        next();
    };
    const next = () => {
        if (px) window.dispatchEvent(new MouseEvent('mousemove', {movementX: px, movementY: 0}));
        requestAnimationFrame(grab);
    };
    next();
})"""


# `frames` display frames of `px` mouse movement each: a turn as a reader makes it.
TURN = """([px, frames]) => new Promise((done) => {
    let n = 0;
    const step = () => {
        window.dispatchEvent(new MouseEvent('mousemove', {movementX: px, movementY: 0}));
        n += 1;
        if (n >= frames) done(n); else requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
})"""


def run(page, check, engine):
    base = page.url.split("?")[0]
    # the look speed a reader can set from the address: a quarter, so the slow
    # pan below moves the picture about a fifth of a pixel a frame
    # M5: standing on the green looking south-west (?go=), out of bed, with the raid's
    # orcs and the villagers round about and the merchant on the road, all
    # held still (`story.still`) for the still and slow-pan views; the
    # opening itself is checked from a plain load below. (In bed the view
    # looks steeply down over the near grass's dithered edge, which a slow
    # pan sees pass: motion, not flicker, R2.9 and R7.9.)
    page.goto(base + "?pointer=free&look=25&go=1995,2790,140&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=30000)

    if page.get_attribute("body", "data-webgl") == "none":
        # the reader is told what is in the way and what to do
        check(page.inner_text("#message") == STRINGS["no_webgl"], f"[{engine}] no WebGL 2: the message from data/strings.json is shown")
        return

    check(page.title() == STRINGS["title"], "the page's title comes from data/strings.json")
    check(page.get_attribute("body", "data-screen") == "title", "the game opens on the title screen")
    check(page.inner_text("#title h1") == STRINGS["title"], "the title screen shows the title")
    button = page.locator("#title button")
    check(button.inner_text() == STRINGS["play"], "the title screen's button says Play")
    rows = page.locator("#title table.controls tr")
    # M0's four controls and M1's eight (the sword, its hand, the buttons,
    # the wheel, F and Space)
    # M6 added Q (the imbue) to M1's twelve; M7 (T14) the tap of Shift
    check(rows.count() == 14, "the title screen lists the fourteen controls")
    check(rows.nth(1).inner_text().split() == [STRINGS["key_run"], STRINGS["control_run"]], "Shift runs, as the controls table says")
    sprint_row = [c.inner_text() for c in rows.nth(2).locator("td").all()]
    check(sprint_row == [STRINGS["key_sprint"], STRINGS["control_sprint"]], f"a tap of Shift sprints, as the controls table says: {sprint_row}")
    term = page.get_attribute("#title td span[data-term='Run']", "title") or ""
    # 7 m/s: the person asked for faster movement (SECOND-ORDER.md R9.40)
    check("7 metres a second" in term, f"hovering Run shows its definition with the core's number: {term!r}")
    check(page.locator("#view").is_visible(), "the canvas is shown")

    check(page.get_attribute("body", "data-sound") is None, "no sound is set up before the reader presses anything")
    button.click()
    page.wait_for_selector("body[data-screen='play']", state="attached")
    check(page.locator("#overlay").is_hidden(), "nothing covers the game in play")
    # M7: the click on Play sets the sound up (web/sound.js), where the
    # browser has Web Audio; it changes nothing the checks below compare
    if page.evaluate("() => Boolean(window.AudioContext || window.webkitAudioContext)"):
        page.wait_for_selector("body[data-sound='on']", state="attached", timeout=10000)
        check(True, "Play sets the sound up, after the reader's click")
    hint = page.locator("#hud-hint")
    check(hint.inner_text() == STRINGS["hint_pause"], "the line over the game tells the reader how to pause")

    # let the camera's ease from the title finish (45 ticks, 0.75 s)
    page.evaluate(WAIT_FRAMES, [30, 1500])

    # CW-18: a still view draws the same pixels on consecutive frames
    still = page.evaluate(GRAB, [2, 0, False])
    a, b = still["frames"]
    check(a["colours"] > 20, f"the frame is a picture, not a blank ({a['colours']} colours sampled)")
    check(a["hash"] == b["hash"], f"two consecutive frames with the camera still are identical, pixel for pixel ({still['w']}x{still['h']})")

    # the comparison can see a change: one turn of the mouse changes the frame
    moved = page.evaluate(GRAB, [1, 160, False])
    check(moved["frames"][0]["hash"] != a["hash"], "a turn of the mouse changes the frame (the comparison sees change)")

    # a slow pan (CW-18, PLAN.md Flicker 5): no pixel flips one way and
    # straight back across three frames. A pan faster than the thinnest
    # things drawn are wide (a far trunk, a cone's tip) sees them pass a pixel
    # and leave it, which is motion, not flicker (SECOND-ORDER.md R2.9)
    pan = page.evaluate(GRAB, [3, 1, True])
    check(pan["frames"][0]["hash"] != pan["frames"][2]["hash"], "the slow pan moves the picture")
    share = pan["flips"] / pan["pixels"]
    check(share < 0.0001, f"a slow pan shows no shimmer: {pan['flips']} of {pan['pixels']} pixels flipped and back ({share:.5%})")
    print(f"  ({engine}: slow pan, {pan['flips']} A-B-A pixels of {pan['pixels']})")

    page.keyboard.press("Escape")
    page.wait_for_selector("body[data-screen='paused']", state="attached")
    check(page.inner_text("#paused h1") == STRINGS["paused"], "Esc pauses, and the pause screen says so")
    resume = page.locator("#paused button")
    check(resume.inner_text() == STRINGS["resume"], "the pause screen offers Resume")
    paused = page.evaluate(GRAB, [2, 0, False])
    check(paused["frames"][0]["hash"] == paused["frames"][1]["hash"], "a paused world draws the same frame")
    resume.click()
    page.wait_for_selector("body[data-screen='play']", state="attached")
    check(page.inner_text("#message") == "", "no error message is shown")

    # M5: a plain load, no query string at all, plays the real opening: in
    # bed during the raid, up after a moment, and the line over the game
    # says the orcs are in the village (data/strings.json)
    page.goto(base)
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=30000)
    page.locator("#title button").click()
    page.wait_for_selector("body[data-screen='play']", state="attached")
    page.wait_for_selector("#hud-story.shown", state="attached", timeout=20000)
    check(page.inner_text("#hud-story") == STRINGS["story_wake"], "a plain load opens in bed during the raid, and getting up the line says the orcs are in the village")
    page.keyboard.press("Escape")

    # M1: a test duel through the address (?duel=orc, forwarded through act):
    # drawn, the line over the game names the orc held on and its health,
    # and the player's own health is there from the start
    page.goto(base + "?pointer=free&duel=orc&foe=still&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=30000)
    page.locator("#title button").click()
    page.wait_for_selector("body[data-screen='play']", state="attached")
    page.keyboard.press("KeyX")
    page.evaluate(WAIT_FRAMES, [30, 600])
    foe = page.locator("#play-hud .hud-foe.shown .bar-label")
    check(foe.count() == 1 and foe.text_content() == STRINGS["who_orc"], "drawn, the orc held on gets a health bar with its name")
    check(page.inner_text("#message") == "", "the test duel's flags are taken without an error")
    # CW-18 with fighters on screen: two still frames, sword drawn, identical
    page.evaluate(WAIT_FRAMES, [90, 1500])
    duel = page.evaluate(GRAB, [2, 0, False])
    check(duel["frames"][0]["hash"] == duel["frames"][1]["hash"], "a still duel draws the same frame twice: nothing jitters")

    # M7 (P4, wounds): an orc's forearm taken off and its torso cut from the
    # address, the duel held still; once the part has come to rest, the
    # cleaver lies where it fell and the wounds are frozen (wounds.freeze:420,
    # ticks from the start of play), a still view with the stump, the piece,
    # the dropped weapon and the blood on the ground draws the same pixels
    # on consecutive frames (CW-18)
    page.goto(base + "?pointer=free&duel=orc&foe=still&gap=260&act=story.still:on&act=wounds.take_off:forearm_r&act=wounds.cut:torso,14&act=wounds.freeze:420")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=30000)
    page.locator("#title button").click()
    page.wait_for_selector("body[data-screen='play']", state="attached")
    page.keyboard.press("KeyX")
    page.evaluate(WAIT_FRAMES, [480, 8000])
    check(page.inner_text("#message") == "", "the wounds' acts on the address are taken without an error")
    cut = page.evaluate(GRAB, [2, 0, False])
    check(cut["frames"][0]["hash"] == cut["frames"][1]["hash"], f"a frozen view after a part is taken off draws the same pixels on consecutive frames ({cut['w']}x{cut['h']})")
    menus(page, check, engine, base)
    travel(page, check, engine, base)
    save(page, check, engine, base)


ITEMS = json.loads((ROOT / "data" / "items.json").read_text())


def menus(page, check, engine, base):
    """M4: the menu ring opens on Tab, turns with E and Q, takes a quick
    item onto a key, and closes on Tab; the canvas behind it is unchanged.
    The full test inventory comes from the address (?act=test.give_all), as
    a reader could open it."""
    page.goto(base + "?pointer=free&look=25&act=test.give_all&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=30000)
    if page.get_attribute("body", "data-webgl") == "none":
        return
    check(page.locator("#title-more button").inner_text() == STRINGS["settings"], "the title screen offers Settings")
    page.click("#title button")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    page.evaluate(WAIT_FRAMES, [30, 1500])
    check(page.locator("#play-hud.shown").count() == 1, "the HUD shows in play")
    health = page.text_content("#play-hud .bar-health .bar-label")
    check(health == STRINGS["health"], f"the HUD names health in words, not colour alone: {health!r}")
    before = page.evaluate(GRAB, [1, 0, False])["frames"][0]["hash"]

    page.keyboard.press("Tab")
    page.wait_for_selector("#menu.shown")
    check(page.get_attribute("body", "data-screen") == "menu", "Tab opens the menu ring")
    check(page.get_attribute("#menu", "data-face") == "select", "the ring opens at Select Item")
    check(page.text_content("#menu .face-select .face-heading") == STRINGS["select_item"], "Select Item is headed from data/strings.json")
    paused = page.evaluate(GRAB, [2, 0, False])["frames"]
    check(paused[0]["hash"] == paused[1]["hash"], "the world behind the ring holds still")

    page.keyboard.press("KeyE")
    page.wait_for_selector("#menu[data-face='map']")
    check(True, "E turns the ring to the map")
    check(page.locator("#menu .face-map canvas").evaluate("(c) => c.width") > 0, "the map's parchment is drawn")
    page.keyboard.press("KeyQ")
    page.wait_for_selector("#menu[data-face='select']")
    check(True, "Q turns it back to Select Item")

    # the rock's cell in Select Item's grid, from data/items.json
    rock = next(i for i in ITEMS["items"] if i["id"] == "rock")
    for _ in range(rock["grid"]):
        page.keyboard.press("ArrowRight")
    chosen = page.locator("#menu .face-select .slot.chosen")
    chosen.wait_for()
    check(chosen.get_attribute("data-item") == "rock", "the arrows move the cursor to the rock")
    page.keyboard.press("Digit3")
    page.wait_for_selector("#menu .menu-foot .quick[data-key='3'][data-item='rock']", state="attached")
    badge = chosen.locator(".badge").text_content()
    check(badge == STRINGS["key_3"], f"pressing 3 over the rock puts it on key 3, and the slot says so: {badge!r}")

    page.keyboard.press("Tab")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    check(page.locator("#menu.shown").count() == 0, "Tab closes the ring")
    check(page.locator("#play-hud .hud-quick .quick[data-key='3'][data-item='rock']").count() == 1, "the HUD shows the rock on 3")
    page.evaluate(WAIT_FRAMES, [10, 300])
    after = page.evaluate(GRAB, [2, 0, False])["frames"]
    check(after[0]["hash"] == after[1]["hash"], "after the ring closes, a still view draws the same pixels on consecutive frames")
    check(after[0]["hash"] == before, "the canvas draws as it did before the ring opened")

    page.keyboard.press("Tab")
    page.wait_for_selector("#menu.shown")
    page.keyboard.press("Escape")
    page.wait_for_selector("body[data-screen='paused']", state="attached")
    check(True, "Esc closes the ring to the pause screen")
    page.click("#paused-more button")
    page.wait_for_selector("#settings.shown")
    labels = page.locator("#settings .control-label").all_text_contents()
    # (M7 P5: the swing cue; P4: and Blood)
    check(labels == [STRINGS["look_speed"], STRINGS["mouse_sensitivity"], STRINGS["invert_y"], STRINGS["volume"], STRINGS["swing_cue"], STRINGS["blood"]], f"settings lists its six settings: {labels}")
    focused = page.evaluate("() => document.activeElement && document.activeElement.id")
    check(focused == "setting-look_speed", f"settings opens with its first control focused for the keyboard: {focused!r}")
    page.keyboard.press("ArrowRight")
    page.wait_for_function("() => document.querySelector('#setting-look_speed').value !== '25'")
    check(page.text_content("#settings .control[data-key='look_speed'] .control-value") != "25%", "the arrow keys change the look speed")
    page.keyboard.press("Escape")
    page.wait_for_selector("body[data-screen='paused']", state="attached")
    check(page.inner_text("#message") == "", "no error message is shown after the menus")

    # M2: the world's terrain, trees and haze, seen from the trainer's hill
    # west over the plains and the ridges to the horizon (?go=: the core's
    # places.go), away from the river, whose waves move every tick. A still
    # view draws the same pixels on consecutive frames, through every detail
    # level's dither band in sight; a slow pan shows no shimmer on the far
    # terrain (CW-18, PLAN.md flicker 5)
    page.goto(base + "?pointer=free&look=25&go=352,1772,100,-1&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    page.click("#title button")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    page.evaluate(WAIT_FRAMES, [120, 2500])
    hill = page.evaluate(GRAB, [2, 0, False])
    a, b = hill["frames"]
    check(a["colours"] > 20, f"the view from the hill is a picture ({a['colours']} colours sampled)")
    check(a["hash"] == b["hash"], f"from the hill, a still view over the terrain to the horizon draws the same pixels on consecutive frames ({hill['w']}x{hill['h']})")
    pan = page.evaluate(GRAB, [3, 1, True])
    check(pan["frames"][0]["hash"] != pan["frames"][2]["hash"], "the slow pan over the terrain moves the picture")
    share = pan["flips"] / pan["pixels"]
    check(share < 0.0001, f"a slow pan over the terrain to the horizon shows no shimmer: {pan['flips']} of {pan['pixels']} pixels flipped and back ({share:.5%})")
    print(f"  ({engine}: slow pan from the hill, {pan['flips']} A-B-A pixels of {pan['pixels']})")
    check(page.inner_text("#message") == "", "no error message is shown on the hill")

    # M7 (R9.44): nothing near pops. Running across the meadow and turning a
    # whole turn, every frame draws every grass, tree and rock chunk within
    # 25 m of the player: the core's probe (?act=lod.probe) counts, each
    # frame, the chunks there left undrawn, and keeps the most
    page.goto(base + "?pointer=free&go=1500,2650,80,-3&act=story.skip&act=lod.probe")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    page.click("#title button")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    page.evaluate(WAIT_FRAMES, [60, 1000])
    page.keyboard.down("ShiftLeft")
    page.keyboard.down("KeyW")
    page.evaluate(WAIT_FRAMES, [180, 3000])
    page.evaluate(TURN, [24, 150])
    page.keyboard.up("KeyW")
    page.keyboard.up("ShiftLeft")
    page.evaluate(WAIT_FRAMES, [2, 0])
    probe = (page.get_attribute("body", "data-probe") or "").split()
    check(len(probe) == 2 and probe[0] == "0" and int(probe[1]) > 100, f"running and turning on the meadow, no grass, tree or rock within 25 m is ever left undrawn (most undrawn in a frame, chunks looked at: {probe})")
    # M7 save (T16): that run, past the opening, crossed areas, but a game set
    # up from the address never saves itself, so a test link leaves a
    # reader's own save as it was and every page after this opens on Play
    check(page.evaluate("() => Object.keys(localStorage).length") == 0, "a game set up from the address leaves no save in the browser")

    # M6: the leader's volley staged from the address (the core's
    # magic.leader, then magic.freeze:150: frozen 150 ticks into play), the
    # view held on the leader. Orbs are glowing, blended shapes: a frozen
    # volley in sight draws the same pixels on consecutive frames (CW-18)
    page.goto(base + "?pointer=free&foe=still&act=magic.leader&act=magic.freeze:150&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    page.click("#title button")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    page.keyboard.press("KeyX")
    page.evaluate(WAIT_FRAMES, [240, 4000])
    volley = page.evaluate(GRAB, [2, 0, False])
    a, b = volley["frames"]
    check(a["hash"] == b["hash"], f"a frozen volley of orbs in sight draws the same pixels on consecutive frames ({volley['w']}x{volley['h']})")
    check(page.inner_text("#message") == "", "the magic acts on the address are taken without an error")
    flicker_audit(page, check, engine, base)
    menus_m7(page, check, engine, base)


# ---------------------------------------------------------------- M7 (P3)

# GRAB on another canvas: the same three frames and A-B-A count, read from
# the canvas `selector` names.
GRAB_ON = GRAB.replace(
    "([count, px, keep]) => new Promise((done) => {\n    const gl = document.getElementById('view').getContext('webgl2');",
    "([count, px, keep, selector]) => new Promise((done) => {\n    const gl = document.querySelector(selector).getContext('webgl2');",
)
assert GRAB_ON != GRAB, "GRAB's opening line moved: GRAB_ON no longer reads another canvas"

# The places of the flicker audit (CW-18, T3): each opened from the address
# as a reader could (?go=: metres east and south, yaw and pitch in degrees),
# the story held still. `still`: two frames identical; `pan`: a slow pan
# shows no shimmer; the river's water moves every tick, so there a still
# view is held to the A-B-A bound instead of identity.
#
# `pan`: "check" fails the run on shimmer; "measure" prints the count only,
# for a place whose shimmer is known and routed to its owner in
# SECOND-ORDER.md (R9.98): in bed and at the camp the slow pan passes over
# the near grass's and the forest's dithered edges (M2's scatter, P2's T8);
# at the river, over moving water.
AUDIT = [
    ("the bed", "act=story.still:on", "identical", "measure"),
    ("the village street", "go=1996,2838,18,-7&act=story.still:on", "identical", "check"),
    ("the bridge", "go=609,2236,242,-10&act=story.still:on", "aba", "check"),
    # looking down on the river, most of the picture is water moving under
    # the pan, which turns ripples A-B-A (85 to 100 of 921 600): measured
    ("the river", "go=636,2262,20,-28&act=story.still:on", "aba", "measure"),
    ("the camp", "go=3196,1012,300,-9&act=story.still:on", "identical", "measure"),
    ("the hut", "go=314,1774,322,-5&act=story.still:on", "identical", "check"),
    # west from the hill the river's water is in view: the A-B-A bound
    ("the horizon", "go=352,1772,282,-1&act=story.still:on", "aba", "check"),
    ("the merchant's cart", "act=merchant.beside&act=merchant.stop&act=story.still:on", "identical", "check"),
]


def flicker_audit(page, check, engine, base):
    """M7: a still view and a slow pan at every place of the audit, with the
    menus open over the game, with dialogue open, and the Equipment
    screen's turning figure (CW-18)."""
    for name, query, still_rule, pan_rule in AUDIT:
        page.goto(base + "?pointer=free&look=25&" + query)
        page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
        if page.get_attribute("body", "data-webgl") == "none":
            return
        page.click("#title button")
        page.wait_for_selector("body[data-screen='play']", state="attached")
        page.evaluate(WAIT_FRAMES, [120, 2500])
        # on a software renderer (Chromium here) the page draws at half the
        # pixels, so a thin grass blade or a ripple is a larger share of the
        # picture (R2.9, R7.9): there these places' shimmer counts are
        # measured, and the green's and the hill's, above, stay held to the
        # bound in every engine; identical still frames are held everywhere
        software = page.get_attribute("body", "data-software") == "true"
        if still_rule == "identical":
            still = page.evaluate(GRAB, [2, 0, False])
            a, b = still["frames"]
            check(a["colours"] > 20 and a["hash"] == b["hash"], f"at {name}, a still view draws the same pixels on consecutive frames ({a['colours']} colours)")
        else:
            still = page.evaluate(GRAB, [3, 0, True])
            share = still["flips"] / still["pixels"]
            if not software:
                check(share < 0.0001, f"at {name} (moving water), a still view shows no shimmer: {still['flips']} of {still['pixels']} pixels flipped and back")
            print(f"  ({engine}: {name}, still with moving water {still['flips']} A-B-A pixels of {still['pixels']})")
        pan = page.evaluate(GRAB, [3, 1, True])
        share = pan["flips"] / pan["pixels"]
        check(pan["frames"][0]["hash"] != pan["frames"][2]["hash"], f"at {name}, the slow pan moves the picture")
        if pan_rule == "check" and not software:
            check(share < 0.0001, f"at {name}, a slow pan shows no shimmer: {pan['flips']} of {pan['pixels']} pixels flipped and back")
        print(f"  ({engine}: {name}, slow pan {pan['flips']} A-B-A pixels of {pan['pixels']})")
        check(page.inner_text("#message") == "", f"at {name}, no error message")

    # the menus over the game: each face still; the figure turning slowly
    # shows no shimmer on its own canvas
    page.goto(base + "?pointer=free&act=test.give_all&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    page.click("#title button")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    page.evaluate(WAIT_FRAMES, [60, 1500])
    page.keyboard.press("Tab")
    page.wait_for_selector("#menu.shown")
    for face in ["select", "map", "quest", "equipment"]:
        page.wait_for_selector(f"#menu[data-face='{face}']")
        page.evaluate(WAIT_FRAMES, [40, 800])
        held = page.evaluate(GRAB, [2, 0, False])["frames"]
        check(held[0]["hash"] == held[1]["hash"], f"with the menu open at {face}, the world behind draws the same pixels on consecutive frames")
        if face == "equipment":
            fig = page.evaluate(GRAB_ON, [3, 0, True, "#menu .equip-figure canvas.figure"])
            share = fig["flips"] / fig["pixels"]
            # GRAB samples one pixel in 997: on the software renderer's
            # half-size canvas (about 15 000 pixels) that is 15 samples, so the
            # figure there is shown by more than its background's one colour
            software = page.get_attribute("body", "data-software") == "true"
            least = 2 if software else 8
            check(fig["frames"][0]["colours"] > least, f"the Equipment screen draws the player's figure on its canvas ({fig['frames'][0]['colours']} colours)")
            check(fig["frames"][0]["hash"] != fig["frames"][2]["hash"], "the figure turns")
            if not software:
                check(share < 0.0005, f"the turning figure shows no shimmer: {fig['flips']} of {fig['pixels']} pixels flipped and back")
            print(f"  ({engine}: the Equipment figure, {fig['flips']} A-B-A pixels of {fig['pixels']})")
        page.keyboard.press("KeyE")
    page.keyboard.press("Tab")

    # dialogue open over the game
    page.goto(base + "?pointer=free&act=test.dialogue&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    page.click("#title button")
    page.wait_for_selector("#talk.shown")
    page.wait_for_function("() => document.querySelector('#talk .talk-next.done')")
    talk = page.evaluate(GRAB, [2, 0, False])["frames"]
    check(talk[0]["hash"] == talk[1]["hash"], "with dialogue open, the world behind draws the same pixels on consecutive frames")


def menus_m7(page, check, engine, base):
    """M7: the ring's side names change when it stops turning; the map's
    marks stand clear of the arrow; a quick key pointed at is described; the
    shop refuses the sword in hand with its line; every screen at three
    window sizes keeps its words inside it and apart, and the keyboard's
    focus shows."""
    import layout

    page.set_viewport_size({"width": 1920, "height": 1080})
    page.goto(base + "?pointer=free&act=test.give_all&act=test.explore&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    page.click("#title button")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    page.keyboard.press("Tab")
    page.wait_for_selector("#menu[data-face='select']")
    layout.settle(page)
    right = "#menu .turn-right span"
    check(page.text_content(right) == STRINGS["map"], "at Select Item the right turn names the map")
    page.keyboard.press("KeyE")
    page.wait_for_selector("#menu[data-face='map']")
    turning = page.evaluate("() => [document.querySelector('#menu').classList.contains('turning'), document.querySelector('#menu .turn-right span').textContent]")
    check(turning == [True, STRINGS["map"]], f"as the ring starts to turn, the side name stays and fades: {turning}")
    page.wait_for_selector("#menu:not(.turning)")
    check(page.text_content(right) == STRINGS["quest"], "once the ring stops at the map, the right turn names Quest")

    # the map: the village's mark stands clear of the arrow at the player
    layout.settle(page)
    boxes = page.evaluate("""() => {
        const r = (el) => { const b = el.getBoundingClientRect(); return [b.left, b.top, b.right, b.bottom]; };
        const you = document.querySelector('#menu .face-map .you .you-arrow');
        const marks = [...document.querySelectorAll('#menu .face-map .marks .mark')];
        return {you: r(you), marks: marks.map((m) => [r(m.querySelector('img')), r(m.querySelector('.mark-label'))])};
    }""")
    meet = lambda a, b: max(0, min(a[2], b[2]) - max(a[0], b[0])) * max(0, min(a[3], b[3]) - max(a[1], b[1]))
    check(len(boxes["marks"]) > 0, "the explored map shows marks")
    covered = [m for m in boxes["marks"] if meet(boxes["you"], m[0]) > 0 or meet(boxes["you"], m[1]) > 0]
    check(not covered, f"the arrow covers no mark's icon or name ({len(covered)} covered)")

    # a quick key pointed at is described over the keys, and not otherwise
    page.keyboard.press("KeyQ")
    page.wait_for_selector("#menu[data-face='select']")
    check(page.locator("#menu .quick-info").is_hidden(), "no quick key is described until one is pointed at")
    page.hover("#menu .menu-foot .quick-row .quick >> nth=1")
    page.wait_for_selector("#menu .quick-info:not([hidden])")
    check(page.text_content("#menu .quick-info .info-name") == STRINGS["item_rock"], "pointing at key 2 describes the rock on it")
    page.mouse.move(2, 2)
    page.wait_for_selector("#menu .quick-info", state="hidden")
    check(True, "moving off the keys puts the description away")
    page.keyboard.press("Tab")

    # the shop: the sword in hand is kept, with the refusal line
    page.goto(base + "?pointer=free&act=test.shop&act=inventory.give:sword&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    page.click("#title button")
    page.wait_for_selector("#shop.shown")
    row = "#shop .shop-row[data-key='short_sword']"
    check(page.text_content(row + " .row-price") == STRINGS["kept_hand"], "the short sword in hand is listed as in your hand")
    coins = page.text_content("#shop .shop-head .count-value")
    page.click(row + " .row-button")
    page.wait_for_selector("#shop .shop-note:not(:empty)")
    line = STRINGS["refuse_sell_hand"].replace("{item}", STRINGS["item_short_sword"])
    check(page.text_content("#shop .shop-note") == line, f"selling the sword in hand is refused with its line: {page.text_content('#shop .shop-note')!r}")
    check(page.text_content("#shop .shop-head .count-value") == coins, "no coin moved")

    # every screen at three sizes; on a software renderer (Chromium here,
    # SwiftShader) at the smallest only, the hardest to fit, since each
    # screen's page draws the world with the CPU (R9.86); Firefox and WebKit
    # draw with the GPU and take all three
    software = page.get_attribute("body", "data-software") == "true"
    for w, h in layout.SIZES[:1] if software else layout.SIZES:
        page.set_viewport_size({"width": w, "height": h})
        for name, roots, tab in layout.screens(page, base):
            found = layout.problems(page, roots)
            check(not found, f"{name} at {w}x{h}: its words stay inside the window and apart {found[:4]}")
            if tab:
                bad = [f["what"] for f in layout.tab_focus(page, roots) if not f["ring"]]
                check(not bad, f"{name} at {w}x{h}: the keyboard's focus shows on every control {bad[:4]}")
    page.set_viewport_size({"width": 1280, "height": 720})

    # M7: a burning house in the raid, its flames and smoke held by the
    # story's still (`story.still`): flames are unlit solids and smoke is
    # dithered, nothing blended to sort, so a still view of them draws the
    # same pixels on consecutive frames (CW-18)
    page.goto(base + "?pointer=free&go=1972,2800,60,-2&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    page.click("#title button")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    page.evaluate(WAIT_FRAMES, [90, 1500])
    fire = page.evaluate(GRAB, [2, 0, False])
    a, b = fire["frames"]
    check(a["colours"] > 20, f"the burning house is a picture ({a['colours']} colours sampled)")
    check(a["hash"] == b["hash"], f"a burning house held still, flames and smoke, draws the same pixels on consecutive frames ({fire['w']}x{fire['h']})")
    page.keyboard.press("Escape")
    page.wait_for_selector("body[data-screen='paused']", state="attached")
    paused = page.evaluate(GRAB, [2, 0, False])
    check(paused["frames"][0]["hash"] == paused["frames"][1]["hash"], "paused by the fire, the frame holds still")

    # M7: a spark staged between the blades and the world frozen on it (the
    # core's feel.stage_spark): its glow and core draw the same pixels on
    # consecutive frames
    page.goto(base + "?pointer=free&duel=orc&foe=still&gap=150&act=feel.stage_spark:3&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    page.click("#title button")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    page.evaluate(WAIT_FRAMES, [90, 1500])
    spark = page.evaluate(GRAB, [2, 0, False])
    check(spark["frames"][0]["hash"] == spark["frames"][1]["hash"], "a frozen spark draws the same pixels on consecutive frames")
    check(page.inner_text("#message") == "", "the feel acts on the address are taken without an error")

def travel(page, check, engine, base):
    """M7 travel (T11): a woken waystone in view holds still frame to frame
    and on a slow pan (CW-18), and the flute plays from Select Item: its
    song screen opens, a note shows on the staff, Esc puts it away."""
    # M2's view from the trainer's hill, with the hill's waystone (28 m west,
    # in sight) woken. (By the village's waystone, the near grass and the
    # horizon alone flip 42 pixels on this pan, the waystone's light 4 more:
    # SECOND-ORDER.md R9.208.)
    page.goto(base + "?pointer=free&look=25&go=352,1772,100,-1&act=travel.wake:hill&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    if page.get_attribute("body", "data-webgl") == "none":
        return
    page.click("#title button")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    page.evaluate(WAIT_FRAMES, [120, 2500])
    still = page.evaluate(GRAB, [2, 0, False])
    a, b = still["frames"]
    check(a["colours"] > 20, f"the woken waystone's view is a picture ({a['colours']} colours sampled)")
    check(a["hash"] == b["hash"], f"a woken waystone in view draws the same pixels on consecutive frames ({still['w']}x{still['h']})")
    pan = page.evaluate(GRAB, [3, 1, True])
    share = pan["flips"] / pan["pixels"]
    check(share < 0.0001, f"a slow pan past a woken waystone shows no shimmer: {pan['flips']} of {pan['pixels']} pixels flipped and back ({share:.5%})")
    print(f"  ({engine}: slow pan by the woken waystone, {pan['flips']} A-B-A pixels of {pan['pixels']})")
    check(page.inner_text("#message") == "", "the travel acts on the address are taken without an error")

    # the flute, from Select Item: its cell from data/items.json
    page.goto(base + "?pointer=free&act=story.skip&act=travel.give&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    page.click("#title button")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    page.keyboard.press("Tab")
    page.wait_for_selector("#menu.shown")
    flute = next(i for i in ITEMS["items"] if i["id"] == "flute")
    for _ in range(flute["grid"]):
        page.keyboard.press("ArrowRight")
    page.wait_for_selector("#menu .face-select .slot.chosen[data-item='flute']")
    page.keyboard.press("Enter")
    page.wait_for_selector("#song.shown", state="attached")
    check(page.get_attribute("body", "data-screen") == "song", "Enter on the flute opens its song screen")
    check(page.text_content("#song .song-heading") == STRINGS["flute_heading"], "the song screen is headed from data/strings.json")
    page.keyboard.press("ArrowUp")
    page.wait_for_selector("#song .staff-note", state="attached")
    check(page.locator("#song .staff-note").count() == 1, "a note played shows on the staff")
    page.keyboard.press("Escape")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    check(page.locator("#song.shown").count() == 0, "Esc puts the flute away")
    check(page.inner_text("#message") == "", "no error message is shown after the flute")


def save(page, check, engine, base):
    """M7 save (T16): a save writes one key in the browser's storage, a
    reload offers Continue and plays it, the pause's Download save gives the
    same text as a file and Load save file reads it back, a changed file is
    refused with its line, and a corrupt stored value starts a new game with
    a line. The storage is read and cleared here, in the check; the page's
    own storage code is web/save.js alone."""
    import tempfile
    page.goto(base + "?pointer=free&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    if page.get_attribute("body", "data-webgl") == "none":
        return
    page.evaluate("() => localStorage.clear()")
    # a world past the opening, saved at once (the test act save.now)
    page.goto(base + "?pointer=free&act=story.skip&act=story.still:on&act=save.now")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    check(page.inner_text("#message") == "", "the save acts on the address are taken without an error")
    keys = page.evaluate("() => Object.keys(localStorage)")
    check(len(keys) == 1, f"saving writes one key in the browser's storage: {keys}")
    if len(keys) != 1:
        return
    stored = json.loads(page.evaluate("(k) => localStorage.getItem(k)", keys[0]))
    check(stored.get("format") == "chumworld-save" and stored.get("version") == 1, "the stored text is the core's save")

    page.goto(base + "?pointer=free")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    check(page.inner_text("#title button") == STRINGS["continue"], "a reload offers Continue")
    check(page.locator("#title-save button").all_text_contents() == [STRINGS["new_game"], STRINGS["load_file"]], "and New game and Load save file")
    page.click("#title button")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    check(page.inner_text("#message") == "", "Continue plays the save without an error")

    page.keyboard.press("Escape")
    page.wait_for_selector("body[data-screen='paused']", state="attached")
    with page.expect_download() as dl:
        page.click("#paused-save button[data-action='save.export']")
    path = dl.value.path()
    text = pathlib.Path(path).read_text()
    check(json.loads(text) == stored, "Download save gives the browser's save as a file")
    check(dl.value.suggested_filename == STRINGS["save_file_name"], f"named {dl.value.suggested_filename!r}")
    with page.expect_file_chooser() as fc:
        page.click("#paused-save button[data-action='save.load']")
    fc.value.set_files(path)
    page.wait_for_selector("#paused-save p", state="attached")
    check(page.inner_text("#paused-save p") == STRINGS["save_loaded"], "Load save file reads the downloaded file back")
    with tempfile.TemporaryDirectory() as d:
        bad = pathlib.Path(d) / "changed.json"
        bad.write_text(text.replace('"coins": 0', '"coins": 500', 1))
        with page.expect_file_chooser() as fc:
            page.click("#paused-save button[data-action='save.load']")
        fc.value.set_files(str(bad))
        page.wait_for_function("() => (document.querySelector('#paused-save p') || {}).textContent !== " + json.dumps(STRINGS["save_loaded"]))
    line = page.inner_text("#paused-save p")
    check(line.startswith("That file was refused") and STRINGS["save_why_checksum"] in line, f"a changed file is refused with its line: {line!r}")

    # a corrupt stored value: a new game, and the line
    page.evaluate("(k) => localStorage.setItem(k, '{broken')", keys[0])
    page.goto(base + "?pointer=free")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    check(page.inner_text("#title button") == STRINGS["play"], "a corrupt stored save offers Play, a new game")
    line = page.inner_text("#title-save p")
    check(line.startswith("The saved game was refused") and line.endswith("A new game starts."), f"and says so: {line!r}")
    page.evaluate("() => localStorage.clear()")
    check(page.inner_text("#message") == "", "no error message is shown after the saves")

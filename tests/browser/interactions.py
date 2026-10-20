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
    # M6 added Q (the imbue) to M1's twelve
    check(rows.count() == 13, "the title screen lists the thirteen controls")
    check(rows.nth(1).inner_text().split() == [STRINGS["key_run"], STRINGS["control_run"]], "Shift runs, as the controls table says")
    term = page.get_attribute("#title td span[data-term='Run']", "title") or ""
    check("5 metres a second" in term, f"hovering Run shows its definition with the core's number: {term!r}")
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
    menus(page, check, engine, base)


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
    check(labels == [STRINGS["look_speed"], STRINGS["mouse_sensitivity"], STRINGS["invert_y"], STRINGS["volume"], STRINGS["swing_cue"]], f"settings lists its five settings: {labels}")
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

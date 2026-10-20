#!/usr/bin/env python3
"""Screenshots of named views of the built game, at 1920 x 1080, for looking
at it and for the design journal. Not a check: it asserts nothing about how
the game looks. It also measures, with --measure.

    .venv/bin/python tools/shoot.py OUT_DIR [VIEW ...] [--soft] [--measure]

Views (all of them when none is named):

    loading   the loading state (the WebAssembly module held back 1.5 s)
    title     the title screen over the drifting field
    standing  just after Play, standing on the field
    turn      the camera mid-turn, the mouse moving
    run       running (Shift + W) with a gentle turn
    paused    the pause screen after Esc

M1's duel views, each on a page of its own opened with the test flags the
page forwards to the core (?duel=, ?foe=still, ?gap=, ?speedster=, and
?sticks=1 for the physics view while the models' weapon door draws nothing,
SECOND-ORDER.md R3.4):

    sheathed  an orc ahead, the sword in its scabbard on the left hip
    drawn     X: the sword at the guard, the view held on the orc
    swing     a committed swing (left button and a flick left) mid-arc
    block     the swing met by the still orc's blade, the two in contact
    hit       a low swing that lands on the orc, its health down
    rock      F: a rock in flight toward a bandit
    orc       a duel with an orc that fights back
    slide     the speedster's five pieces: a run let go, sliding on ice

M4's views (the menus), opened with the test inventory and HUD values the
core gives through `?act=test.give_all&act=test.hud&act=test.explore`:

    hud            play with health, the foe's bar, the imbue's cooldown,
                   the footing bars, quick items, coins and a toast
    menu_select    the ring at Select Item
    menu_turn      the ring mid-turn, Select Item to the map
    menu_map       the ring at the map, filled in along a walk
    menu_quest     the ring at Quest
    menu_equipment the ring at Equipment
    dialogue       the dialogue box, text half shown
    shop           the merchant's shop
    settings       the settings screen, from the title
    pause_menu     the pause screen with its settings button

Model views (M3), each the core's lineup (`act("models.lineup", ...)`)
seen from the player's shoulder, the camera pitched to frame it:

    models-front      every figure in a row, facing the camera
    models-quarter    the same row turned 40 degrees
    models-player     the player close, at 3.5 m
    models-player-q   the player close, turned 40 degrees
    models-orc        an orc close
    models-leader     the bandit leader close
    models-merchant   the merchant close
    models-trainer    the trainer close
    models-props      every prop in two rows
    models-weapons    every held item, stacked
    models-speedster  the speedster pieces worn, and lying on the ground
    models-cart       the merchant's cart and its donkey

M2's views (the world), each on a page of its own opened at a place with
?go=<x>,<z>,<yaw>,<pitch> (metres east and south, degrees; the core's
`places.go`), Play pressed, the camera's ease and the detail levels' first
frames done:

    village   the village from beside the start, looking into the green
    plains    the plains west of the village toward the river
    river     the river's water up close by the bridge
    bridge    the bridge from the west bank
    camp      the bandit camp in its forest clearing
    hut       the trainer's hut on its hill
    horizon   east from the trainer's hill over the plains to the 16 km horizon
    dither    a still frame on a detail level's hand-over (the forest's whole
              trees handing over to cones, 128 to 150 m off; grass at 44 to 58 m)
    map       the map menu filled in (?act=test.explore), the terrain shaded

With --measure, M2 also measures three walks at 1920 x 1080: standing in
the village, running across the plains (Shift + W, turning), and standing on
the hill turning slowly toward the horizon; for each, the frames a second
and the time the page's own frame callback takes (the core's advance, the
meshes it hands over and the draw calls), timed by wrapping
requestAnimationFrame in this tool's page, not the game's code.

The page has no reader-facing way to send a lineup act (R5.8), so for these
views this tool serves app.js with one line added, `window.__act = act;`,
and calls it; everything else the page does is unchanged.

The page is driven the way a reader drives it: the URL a reader can open
(?pointer=free, the realtime kit's mode without pointer lock), clicks on the
core's own buttons, key presses, and mouse movement dispatched as the kit's
README shows (headless Chromium gives synthetic moves no movementX). The page
does not know it is being driven.

Chromium is launched with a hardware GPU (--use-angle=metal on a Mac) unless
--soft asks for SwiftShader. --measure prints the WebGL renderer, the load
time, the frames a second and the frame times while running, as numbers for
MEASUREMENTS.md.
"""
import functools
import http.server
import json
import pathlib
import sys
import threading
import time

from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[1]
DIST = ROOT / "dist"
VIEWS = ["loading", "title", "standing", "turn", "run", "paused"]
DUEL_VIEWS = ["sheathed", "drawn", "swing", "block", "hit", "rock", "orc", "slide"]
M4_VIEWS = ["hud", "menu_select", "menu_turn", "menu_map", "menu_quest", "menu_equipment", "dialogue", "shop", "settings", "pause_menu"]
TEST_ACTS = "&act=test.give_all&act=test.hud&act=test.explore"
# M2: view: the place (?go=), and extra address flags
M2_VIEWS = {
    "village": ("2014,2862,12,-7", ""),
    "plains": ("1500,2650,80,-3", ""),
    "river": ("636,2262,20,-28", ""),
    "bridge": ("598,2228,300,-8", ""),
    "camp": ("3196,1012,300,-9", ""),
    "hut": ("300,1800,325,-4", ""),
    "horizon": ("352,1772,282,-1", ""),
    "dither": ("2905,1755,335,-6", ""),
    "map": ("2000,2800,20,-8", TEST_ACTS),
}
# M2: the time each frame callback of the page takes, and the frames
CALLBACK_TIMES = """() => {
    const raf = window.requestAnimationFrame.bind(window);
    window.__cb = [];
    window.requestAnimationFrame = (f) => raf((t) => { const a = performance.now(); f(t); window.__cb.push(performance.now() - a); });
}"""

# view: (lineup value, camera pitch change in CSS pixels, + looks down)
MODEL_VIEWS = {
    "models-front": ("all", 70),
    "models-quarter": ("all:40", 70),
    "models-player": ("player", 75),
    "models-player-q": ("player:40", 75),
    "models-orc": ("orc", 70),
    "models-leader": ("leader", 72),
    "models-merchant": ("merchant", 72),
    "models-trainer": ("trainer", 72),
    "models-props": ("props", -20),
    "models-weapons": ("weapons", 40),
    "models-speedster": ("speedster", 150),
    "models-cart": ("cart", 20),
}
SIZE = {"width": 1920, "height": 1080}
GPU = ["--use-angle=metal", "--enable-gpu", "--ignore-gpu-blocklist"]
SOFT = ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"]


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
        ".json": "application/json",
    }

    def log_message(self, *args):
        pass


def serve():
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Handler, directory=str(DIST)))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, f"http://127.0.0.1:{server.server_address[1]}"


def turn(page, px, steps):
    """Mouse movement as a reader's would arrive: `steps` moves of `px` CSS
    pixels, one a display frame."""
    page.evaluate(
        """([px, steps]) => new Promise((done) => {
            let k = 0;
            const step = () => {
                window.dispatchEvent(new MouseEvent('mousemove', {movementX: px, movementY: 0}));
                k += 1;
                if (k < steps) requestAnimationFrame(step); else done();
            };
            requestAnimationFrame(step);
        })""",
        [px, steps],
    )


def frame_times(page, ms):
    """Frames a second over `ms`, and the frame intervals' median, 95th
    percentile and worst, from requestAnimationFrame timestamps."""
    return page.evaluate(
        """(ms) => new Promise((done) => {
            const t = [];
            const tick = (now) => {
                t.push(now);
                if (now - t[0] < ms) requestAnimationFrame(tick);
                else {
                    const d = t.slice(1).map((x, i) => x - t[i]).sort((a, b) => a - b);
                    const at = (q) => d[Math.min(d.length - 1, Math.floor(q * d.length))];
                    done({fps: (t.length - 1) * 1000 / (t[t.length - 1] - t[0]), median_ms: at(0.5), p95_ms: at(0.95), worst_ms: d[d.length - 1], frames: t.length - 1});
                }
            };
            requestAnimationFrame(tick);
        })""",
        ms,
    )


def renderer_name(page):
    return page.evaluate(
        """() => {
            const gl = document.getElementById('view').getContext('webgl2');
            if (!gl) return 'none';
            const info = gl.getExtension('WEBGL_debug_renderer_info');
            return info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
        }"""
    )


def wait_frames(page, n):
    """`n` display frames, so a CSS transition or the core's next frame has run."""
    page.evaluate(
        """(n) => new Promise((done) => { let k = 0; const f = () => (++k >= n ? done() : requestAnimationFrame(f)); requestAnimationFrame(f); })""",
        n,
    )


def m4_views(browser, base, out, views, logs):
    """The menus, driven by keys and clicks as a reader drives them."""
    page = browser.new_page(viewport=SIZE, device_scale_factor=1)
    page.on("console", lambda m: logs.append(f"{m.type}: {m.text}") if m.type in ("error", "warning") else None)
    page.on("pageerror", lambda e: logs.append(f"pageerror: {e}"))
    page.goto(base + "/index.html?pointer=free" + TEST_ACTS, wait_until="commit")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=30000)
    page.wait_for_timeout(1200)
    shot = lambda name: page.screenshot(path=str(out / f"{name}.png"))
    if "settings" in views:
        page.click("#title-more button")
        page.wait_for_selector("#settings.shown")
        page.wait_for_timeout(500)
        shot("settings")
        page.keyboard.press("Escape")
        page.wait_for_selector("body[data-screen='title']", state="attached")
    page.click("#title button")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    page.wait_for_timeout(1000)
    page.keyboard.press("Digit2")
    page.wait_for_timeout(400)
    if "hud" in views:
        shot("hud")
    page.keyboard.press("Tab")
    page.wait_for_selector("#menu.shown")
    page.wait_for_timeout(700)
    page.keyboard.press("ArrowRight")
    page.wait_for_timeout(300)
    if "menu_select" in views:
        shot("menu_select")
    page.keyboard.press("KeyE")
    page.wait_for_timeout(200)
    if "menu_turn" in views:
        shot("menu_turn")
    page.wait_for_timeout(700)
    if "menu_map" in views:
        shot("menu_map")
    page.keyboard.press("KeyE")
    page.wait_for_timeout(800)
    if "menu_quest" in views:
        shot("menu_quest")
    page.keyboard.press("KeyE")
    page.wait_for_timeout(500)
    page.keyboard.press("ArrowRight")
    page.wait_for_timeout(500)
    if "menu_equipment" in views:
        shot("menu_equipment")
    page.keyboard.press("Tab")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    if "pause_menu" in views:
        page.keyboard.press("Escape")
        page.wait_for_selector("body[data-screen='paused']", state="attached")
        page.wait_for_timeout(600)
        shot("pause_menu")
        page.click("#paused button")
        page.wait_for_selector("body[data-screen='play']", state="attached")
    # the test acts open these on the first tick of play, as M5's trainer
    # and merchant will
    for name, acts in (("dialogue", "&act=test.hud&act=test.dialogue"), ("shop", "&act=test.shop&act=inventory.give:sword")):
        if name not in views:
            continue
        page.goto(base + "/index.html?pointer=free" + acts, wait_until="commit")
        page.wait_for_selector("body[data-ready='true']", state="attached", timeout=30000)
        page.wait_for_timeout(600)
        page.click("#title button")
        if name == "dialogue":
            page.wait_for_selector("#talk.shown")
            page.wait_for_timeout(1400)
        else:
            page.wait_for_selector("#shop.shown")
            page.click("#shop .shop-list .row-button")
            page.wait_for_timeout(600)
        shot(name)
    page.close()

def callback_stats(page):
    """The page's frame callbacks since the last call: median, 95th
    percentile and worst, ms."""
    return page.evaluate(
        """() => {
            const d = window.__cb.splice(0).sort((a, b) => a - b);
            const at = (q) => d[Math.min(d.length - 1, Math.floor(q * d.length))];
            return {callback_median_ms: at(0.5), callback_p95_ms: at(0.95), callback_worst_ms: d[d.length - 1], callbacks: d.length};
        }"""
    )


def m2_views(browser, base, out, views, logs, numbers, measure):
    """The world at its places (M2), and its measured walks."""
    for name, (go, flags) in M2_VIEWS.items():
        if name not in views:
            continue
        page = browser.new_page(viewport=SIZE, device_scale_factor=1)
        page.on("console", lambda m: logs.append(f"{m.type}: {m.text}") if m.type in ("error", "warning") else None)
        page.on("pageerror", lambda e: logs.append(f"pageerror: {e}"))
        page.goto(base + "/index.html?pointer=free&go=" + go + flags, wait_until="commit")
        page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
        wait_frames(page, 20)
        page.click("#title button")
        page.wait_for_selector("body[data-screen='play']", state="attached")
        # the ease from the title's view, and the meshes built ahead
        wait_frames(page, 150)
        if name == "map":
            page.keyboard.press("Tab")
            page.wait_for_selector("#menu.shown")
            page.wait_for_timeout(700)
            page.keyboard.press("ArrowRight")
            page.wait_for_timeout(300)
            page.keyboard.press("KeyE")
            page.wait_for_timeout(900)
        page.screenshot(path=str(out / f"m2_{name}.png"))
        page.close()
    if not measure:
        return
    for name, go, keys, px in (
        ("m2_village_standing", "2014,2862,12,-7", [], 0),
        ("m2_plains_running", "1500,2650,80,-3", ["ShiftLeft", "KeyW"], 2),
        ("m2_horizon_turning", "352,1772,282,-1", [], 1),
    ):
        page = browser.new_page(viewport=SIZE, device_scale_factor=1)
        page.add_init_script(CALLBACK_TIMES)
        page.goto(base + "/index.html?pointer=free&go=" + go, wait_until="commit")
        page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
        wait_frames(page, 20)
        page.click("#title button")
        page.wait_for_selector("body[data-screen='play']", state="attached")
        for k in keys:
            page.keyboard.down(k)
        if px:
            page.evaluate(
                """(px) => { window.__turning = true; const go = () => { window.dispatchEvent(new MouseEvent('mousemove', {movementX: px, movementY: 0})); if (window.__turning) requestAnimationFrame(go); }; requestAnimationFrame(go); }""",
                px,
            )
        wait_frames(page, 90)
        callback_stats(page)
        numbers[name] = frame_times(page, 5000)
        numbers[name].update(callback_stats(page))
        page.close()


HOOK = 'if (lookSpeed) act("look.speed", lookSpeed);'


def expose_act(route):
    """Serves app.js with the page's own `act` exposed to this tool."""
    response = route.fetch()
    text = response.text()
    if text.count(HOOK) != 1:
        raise SystemExit("shoot.py: app.js no longer has the line the model views hook onto")
    route.fulfill(response=response, body=text.replace(HOOK, "window.__act = act;\n  " + HOOK))


def pitch(page, px):
    """Looks down by `px` CSS pixels of mouse movement (up when negative)."""
    page.evaluate("(px) => window.dispatchEvent(new MouseEvent('mousemove', {movementX: 0, movementY: px}))", px)


def open_game(browser, base, logs, held=None):
    """The game at its reader-openable URL. With `held` (a list), the
    WebAssembly module's request is held in it, not answered, until the
    caller continues it: the page then shows its loading state."""
    page = browser.new_page(viewport=SIZE, device_scale_factor=1)
    page.on("console", lambda m: logs.append(f"{m.type}: {m.text}") if m.type in ("error", "warning") else None)
    page.on("pageerror", lambda e: logs.append(f"pageerror: {e}"))
    if held is not None:
        page.route("**/*_bg.wasm*", lambda route: held.append(route))
    else:
        page.route("**/app.js*", expose_act)
    t0 = time.perf_counter()
    page.goto(base + "/index.html?pointer=free", wait_until="commit")
    return page, t0


def frames(page, n):
    """Waits `n` display frames."""
    page.evaluate(
        "(n) => new Promise((done) => { let k = 0; const s = () => { k += 1; if (k < n) requestAnimationFrame(s); else done(); }; requestAnimationFrame(s); })",
        n,
    )


def move(page, dx, dy, steps):
    """`steps` mouse moves of (dx, dy) CSS pixels, one a display frame."""
    page.evaluate(
        """([dx, dy, steps]) => new Promise((done) => {
            let k = 0;
            const s = () => {
                window.dispatchEvent(new MouseEvent('mousemove', {movementX: dx, movementY: dy}));
                k += 1;
                if (k < steps) requestAnimationFrame(s); else done();
            };
            requestAnimationFrame(s);
        })""",
        [dx, dy, steps],
    )


def duel_page(browser, base, logs, flags):
    """The game opened with test flags, Play pressed, the camera's ease done."""
    page = browser.new_page(viewport=SIZE, device_scale_factor=1)
    page.on("console", lambda m: logs.append(f"{m.type}: {m.text}") if m.type in ("error", "warning") else None)
    page.on("pageerror", lambda e: logs.append(f"pageerror: {e}"))
    page.goto(base + "/index.html?pointer=free&" + flags, wait_until="commit")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=30000)
    frames(page, 20)
    page.click("#title button")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    frames(page, 60)
    return page


def draw_and_wind(page, up_px):
    """X, then the hand wound out to the right (and `up_px` up) for a sweep."""
    page.keyboard.press("KeyX")
    frames(page, 45)
    move(page, 45, -up_px / 12, 12)
    frames(page, 35)


def flick(page, ticks):
    """The left button held and the mouse flicked left, 150 px a frame."""
    page.mouse.move(960, 540)
    page.mouse.down()
    move(page, -150, 0, ticks)


def duel_views(browser, base, out, views, logs):
    """M1's views (DUEL_VIEWS), each from a fresh page."""
    if {"sheathed", "drawn"} & set(views):
        page = duel_page(browser, base, logs, "duel=orc&foe=still&gap=320")
        # drawn first, so the view turns to hold on the orc, then put away
        page.keyboard.press("KeyX")
        frames(page, 70)
        page.keyboard.press("KeyX")
        frames(page, 50)
        if "sheathed" in views:
            page.screenshot(path=str(out / "sheathed.png"))
        page.keyboard.press("KeyX")
        frames(page, 70)
        if "drawn" in views:
            page.screenshot(path=str(out / "drawn.png"))
        page.close()
    if "swing" in views:
        page = duel_page(browser, base, logs, "duel=orc&foe=still&gap=200")
        draw_and_wind(page, 0)
        flick(page, 4)
        page.screenshot(path=str(out / "swing.png"))
        page.mouse.up()
        page.close()
    if "block" in views:
        page = duel_page(browser, base, logs, "duel=orc&foe=still&gap=115")
        # the hand up 24° (160 px at 0.15° a pixel) to meet the orc's guard
        draw_and_wind(page, 160)
        flick(page, 9)
        move(page, -10, 0, 20)
        page.screenshot(path=str(out / "block.png"))
        page.mouse.up()
        page.close()
    if "hit" in views:
        page = duel_page(browser, base, logs, "duel=orc&foe=still&gap=85")
        # low: 26° down (173 px) passes under the orc's guard
        draw_and_wind(page, -173)
        flick(page, 9)
        move(page, -10, 0, 12)
        page.screenshot(path=str(out / "hit.png"))
        page.mouse.up()
        page.close()
    if "rock" in views:
        page = duel_page(browser, base, logs, "duel=bandit&foe=still&gap=900&act=inventory.give:rock:5")
        page.keyboard.press("KeyF")
        # 12 ticks of wind, then a sixth of a second in the air
        frames(page, 22)
        page.screenshot(path=str(out / "rock.png"))
        page.close()
    if "orc" in views:
        page = duel_page(browser, base, logs, "duel=orc&gap=400")
        page.keyboard.press("KeyX")
        frames(page, 150)
        for _ in range(3):
            move(page, 30, -12, 10)
            page.mouse.down()
            move(page, -120, 20, 6)
            page.mouse.up()
            frames(page, 30)
        page.screenshot(path=str(out / "orc.png"))
        page.close()
    if "slide" in views:
        page = duel_page(browser, base, logs, "speedster=5")
        page.keyboard.down("ShiftLeft")
        page.keyboard.down("KeyW")
        frames(page, 200)
        page.keyboard.up("KeyW")
        page.keyboard.up("ShiftLeft")
        # let go at 10 m/s: half a second on, still sliding on still feet
        frames(page, 30)
        page.screenshot(path=str(out / "slide.png"))
        page.close()


def main(argv):
    if not argv or argv[0].startswith("--"):
        sys.exit(__doc__)
    out = pathlib.Path(argv[0])
    out.mkdir(parents=True, exist_ok=True)
    views = [a for a in argv[1:] if not a.startswith("--")] or VIEWS + DUEL_VIEWS + M4_VIEWS + list(M2_VIEWS)
    unknown = [v for v in views if v not in VIEWS and v not in MODEL_VIEWS and v not in DUEL_VIEWS and v not in M4_VIEWS and v not in M2_VIEWS]
    if unknown:
        sys.exit(f"no view named {', '.join(unknown)}")
    soft = "--soft" in argv
    measure = "--measure" in argv
    if not (DIST / "index.html").exists():
        sys.exit("dist/ is missing: run scripts/build.sh first")
    server, base = serve()
    logs = []
    numbers = {}
    with sync_playwright() as p:
        browser = p.chromium.launch(args=SOFT if soft else GPU)
        if "loading" in views:
            held = []
            page, _ = open_game(browser, base, logs, held)
            page.wait_for_selector("#loading-text:not(:empty)", timeout=20000)
            page.wait_for_timeout(600)
            page.screenshot(path=str(out / "loading.png"))
            for route in held:
                route.continue_()
            page.close()
        page, t0 = open_game(browser, base, logs)
        page.wait_for_selector("body[data-ready='true']", state="attached", timeout=30000)
        numbers["load_s"] = round(time.perf_counter() - t0, 2)
        numbers["renderer"] = renderer_name(page)
        numbers["software"] = page.get_attribute("body", "data-software")
        page.wait_for_timeout(1500)
        if measure:
            numbers["title"] = frame_times(page, 3000)
        if "title" in views:
            page.screenshot(path=str(out / "title.png"))
        page.click("#title button")
        page.wait_for_selector("body[data-screen='play']", state="attached")
        page.wait_for_timeout(900)
        if "standing" in views:
            page.screenshot(path=str(out / "standing.png"))
        if measure:
            numbers["standing"] = frame_times(page, 3000)
        if "turn" in views:
            turn(page, -14, 20)
            page.evaluate("() => window.dispatchEvent(new MouseEvent('mousemove', {movementX: -14, movementY: 0}))")
            page.screenshot(path=str(out / "turn.png"))
            turn(page, -14, 10)
        if "run" in views or measure:
            page.keyboard.down("ShiftLeft")
            page.keyboard.down("KeyW")
            page.wait_for_timeout(1400)
            if measure:
                # run in a wide circle: the frame changes every frame
                page.evaluate(
                    """() => { window.__turning = true; const go = () => { window.dispatchEvent(new MouseEvent('mousemove', {movementX: 3, movementY: 0})); if (window.__turning) requestAnimationFrame(go); }; requestAnimationFrame(go); }"""
                )
                numbers["running"] = frame_times(page, 4000)
                page.evaluate("() => { window.__turning = false; }")
            turn(page, 6, 12)
            if "run" in views:
                page.screenshot(path=str(out / "run.png"))
            page.keyboard.up("KeyW")
            page.keyboard.up("ShiftLeft")
        if any(v in MODEL_VIEWS for v in views):
            # a quarter turn right, so the lineup stands clear of the test
            # field's lantern in front of the start (it anchors to this view)
            turn(page, 25, 30)
            page.wait_for_timeout(300)
        for name, (value, down) in MODEL_VIEWS.items():
            if name not in views:
                continue
            page.evaluate("([v]) => window.__act('models.lineup', v)", [value])
            result = page.text_content("#message") or None
            pitch(page, down)
            page.wait_for_timeout(500)
            page.screenshot(path=str(out / f"{name}.png"))
            pitch(page, -down)
            page.wait_for_timeout(200)
            if result is not None:
                logs.append(f"error: {result}")
        if any(v in MODEL_VIEWS for v in views):
            page.evaluate("() => window.__act('models.lineup', 'off')")
        if "paused" in views:
            page.wait_for_timeout(700)
            page.keyboard.press("Escape")
            page.wait_for_selector("body[data-screen='paused']", state="attached")
            page.wait_for_timeout(500)
            page.screenshot(path=str(out / "paused.png"))
        duel_views(browser, base, out, views, logs)
        if any(v in M4_VIEWS for v in views):
            m4_views(browser, base, out, views, logs)
        if any(v in M2_VIEWS for v in views) or measure:
            m2_views(browser, base, out, views, logs, numbers, measure)
        browser.close()
    server.shutdown()
    for name in views:
        for file in (f"{name}.png", f"m2_{name}.png"):
            if (out / file).exists():
                print(f"saved {out / file}")
    if measure:
        print(json.dumps(numbers, indent=2))
    for line in logs:
        print(line)
    sys.exit(1 if any(l.startswith(("error", "pageerror")) for l in logs) else 0)


if __name__ == "__main__":
    main(sys.argv[1:])

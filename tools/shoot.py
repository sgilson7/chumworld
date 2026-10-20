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


def open_game(browser, base, logs, held=None):
    """The game at its reader-openable URL. With `held` (a list), the
    WebAssembly module's request is held in it, not answered, until the
    caller continues it: the page then shows its loading state."""
    page = browser.new_page(viewport=SIZE, device_scale_factor=1)
    page.on("console", lambda m: logs.append(f"{m.type}: {m.text}") if m.type in ("error", "warning") else None)
    page.on("pageerror", lambda e: logs.append(f"pageerror: {e}"))
    if held is not None:
        page.route("**/*_bg.wasm*", lambda route: held.append(route))
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
        page = duel_page(browser, base, logs, "duel=orc&foe=still&gap=320&sticks=1")
        page.keyboard.down("KeyD")
        frames(page, 40)
        page.keyboard.up("KeyD")
        frames(page, 40)
        if "sheathed" in views:
            page.screenshot(path=str(out / "sheathed.png"))
        page.keyboard.press("KeyX")
        frames(page, 70)
        if "drawn" in views:
            page.screenshot(path=str(out / "drawn.png"))
        page.close()
    if "swing" in views:
        page = duel_page(browser, base, logs, "duel=orc&foe=still&gap=200&sticks=1")
        draw_and_wind(page, 0)
        flick(page, 4)
        page.screenshot(path=str(out / "swing.png"))
        page.mouse.up()
        page.close()
    if "block" in views:
        page = duel_page(browser, base, logs, "duel=orc&foe=still&gap=115&sticks=1")
        # the hand up 24° (160 px at 0.15° a pixel) to meet the orc's guard
        draw_and_wind(page, 160)
        flick(page, 9)
        move(page, -10, 0, 20)
        page.screenshot(path=str(out / "block.png"))
        page.mouse.up()
        page.close()
    if "hit" in views:
        page = duel_page(browser, base, logs, "duel=orc&foe=still&gap=85&sticks=1")
        # low: 26° down (173 px) passes under the orc's guard
        draw_and_wind(page, -173)
        flick(page, 9)
        move(page, -10, 0, 12)
        page.screenshot(path=str(out / "hit.png"))
        page.mouse.up()
        page.close()
    if "rock" in views:
        page = duel_page(browser, base, logs, "duel=bandit&foe=still&gap=900&sticks=1")
        page.keyboard.press("KeyF")
        # 12 ticks of wind, then a third of a second in the air
        frames(page, 32)
        page.screenshot(path=str(out / "rock.png"))
        page.close()
    if "orc" in views:
        page = duel_page(browser, base, logs, "duel=orc&gap=400&sticks=1")
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
        page = duel_page(browser, base, logs, "speedster=5&sticks=1")
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
    views = [a for a in argv[1:] if not a.startswith("--")] or VIEWS + DUEL_VIEWS
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
        if "paused" in views:
            page.wait_for_timeout(700)
            page.keyboard.press("Escape")
            page.wait_for_selector("body[data-screen='paused']", state="attached")
            page.wait_for_timeout(500)
            page.screenshot(path=str(out / "paused.png"))
        duel_views(browser, base, out, views, logs)
        browser.close()
    server.shutdown()
    for name in views:
        if (out / f"{name}.png").exists():
            print(f"saved {out / (name + '.png')}")
    if measure:
        print(json.dumps(numbers, indent=2))
    for line in logs:
        print(line)
    sys.exit(1 if any(l.startswith(("error", "pageerror")) for l in logs) else 0)


if __name__ == "__main__":
    main(sys.argv[1:])

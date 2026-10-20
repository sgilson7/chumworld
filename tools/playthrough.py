#!/usr/bin/env python3
"""An end-to-end playthrough of the published flow in a real browser, through
the keyboard and the mouse only (M7 QA, task P3). Not part of check.sh: it
takes most of an hour of game time.

    .venv/bin/python tools/playthrough.py [DIST_DIR] [--out DIR] [--only STEP ...]
                                          [--dev-start STEP] [--size WxH]

It serves DIST_DIR (default: dist/) on 127.0.0.1, opens it in headless
Chromium on the graphics card (Metal on a Mac) at the address a reader can
open (`?pointer=free`, the realtime kit's mode without pointer lock), and
plays: the title, Play, waking in bed; down once to an orc and the dream
line; out past the village boundary; the merchant's cart stopped with a
thrown rock and the shop; the trainer's practice; the bandit camp; the
chest and the boomerang; the five speedster pieces, all worn, and a slide
on ice; every menu.

Input is what a reader gives: `page.keyboard` and `page.mouse`, and mouse
movement dispatched as `MouseEvent`s with `movementX` / `movementY`, as
tools/shoot.py dispatches it (headless Chromium gives synthetic moves no
movement). To steer, the script reads what the page already has: the
frame's view values (the camera's eye and its matrix) and its moving
instances (`game.view()`, `game.instances()`: where the cart, the orcs and
the trainer are drawn, found by their colours in data/models.json), the
DOM's lines over the game (the prompt for E, the story's line), and the core's
view model (`game.ui()`). It writes nothing into the game. To reach them it
serves app.js with one line added after the game is made,
`window.__game = game; window.__act = act;`.

`window.__act` is used at most once, and only for the bandit camp's fight
outcome, if the fight cannot be won by input in its time; the log then says
"TEST ACT USED". Nothing else is a test act: no `?act=`, no `?go=`.

`--dev-start STEP` is for writing this script only: it opens the page at a
place with `?act=story.skip&go=...` and starts at STEP. Its log says
"DEV START" on its first line, and its result is not a playthrough.

Every step writes a line to OUT/playthrough-log.txt (PASS, ISSUE or INFO, the
time into the run and the step's own time) and a screenshot in OUT.
"""
import functools
import http.server
import json
import math
import os
import pathlib
import sys
import threading
import time
import traceback

from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[1]
GPU = ["--use-angle=metal", "--enable-gpu", "--ignore-gpu-blocklist"]
HOOK = 'if (lookSpeed) act("look.speed", lookSpeed);'

# Colours of the things the script steers by (data/models.json, 0 to 255).
MODELS = json.loads((ROOT / "data" / "models.json").read_text())["colours"]

# Where things are, world metres (x east, z south): data/places.json and
# data/story.json.
PLACES = {p["kind"]: p["centre"] for p in json.loads((ROOT / "data" / "places.json").read_text())["places"]}
LAYOUT = json.loads((ROOT / "data" / "places.json").read_text())
STORY = json.loads((ROOT / "data" / "story.json").read_text())
ROAD = next(r["line"] for r in LAYOUT["roads"] if r["kind"] == "road")
LANE = next(r["line"] for r in LAYOUT["roads"] if r["kind"] == "lane")
TRAILS = [r["line"] for r in LAYOUT["roads"] if r["kind"] == "trail"]
HUT_TRAIL = next(t for t in TRAILS if t[0][0] < 1000)
CAMP_TRAIL = next(t for t in TRAILS if t[0][0] > 1000)


def place(kind, off=(0, 0)):
    c = PLACES[kind]
    return [c[0] + off[0], c[1] + off[1]]


def pickup_at(item):
    for p in STORY["pickups"]:
        if p.get("item") == item:
            c = PLACES["village" if p["place"] == "bed" else p["place"]]
            return [c[0] + p["at_cm"][0] / 100, c[1] + p["at_cm"][1] / 100]
    raise KeyError(item)


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
        ".json": "application/json",
    }

    def log_message(self, *args):
        pass


def serve(dist):
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Handler, directory=str(dist)))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, f"http://127.0.0.1:{server.server_address[1]}"


def expose(route):
    response = route.fetch()
    text = response.text()
    if text.count(HOOK) != 1:
        raise SystemExit("playthrough.py: app.js no longer has the line this script hooks onto")
    route.fulfill(response=response, body=text.replace(HOOK, "window.__game = game; window.__act = act;\n  " + HOOK))


# The page-side helper: reads the frame, tracks the world origin, steers the
# view with mouse movement toward a target, and finds things by colour.
HELPER = r"""
(() => {
  const pt = window.__pt = {
    origin: null,       // world x, z of the frame's origin (a 256 m cell corner)
    last: null,         // last absolute eye
    target: null,       // [x, z] to turn toward, or null
    face: null,         // [x, z] to keep facing without walking, or null
    pitch: null,        // radians to hold the view at, or null
    sign: 1,            // which way +movementX turns (measured)
    gain: 0.8,          // px of movement per radian per frame, as a share of `full`
    full: 600,
    frames: 0,
    playFrames: 0,
    drops: [],
    lastY: null,
  };
  const wrap = (a) => { while (a > Math.PI) a -= 2 * Math.PI; while (a < -Math.PI) a += 2 * Math.PI; return a; };
  pt.read = () => {
    const g = window.__game;
    if (!g) return null;
    const v = g.view();
    const m = v.subarray(16, 32);
    let f = [m[3], m[7], m[11]];
    const n = Math.hypot(f[0], f[1], f[2]) || 1;
    f = f.map((c) => c / n);
    const rel = [v[32], v[33], v[34]];
    if (!pt.origin) pt.origin = pt.start || [0, 0];
    // the origin is a multiple of 256 m: the one that keeps the eye nearest
    // where it was
    let ex = rel[0] + pt.origin[0], ez = rel[2] + pt.origin[1];
    if (pt.last) {
      const ox = Math.round((pt.last[0] - rel[0]) / 256) * 256;
      const oz = Math.round((pt.last[2] - rel[2]) / 256) * 256;
      pt.origin = [ox, oz];
      ex = rel[0] + ox; ez = rel[2] + oz;
    }
    pt.last = [ex, rel[1], ez];
    const fh = Math.hypot(f[0], f[2]) || 1;
    const fx = f[0] / fh, fz = f[2] / fh;
    // the player stands ahead of the eye (2.6 m back) and left of it (0.6 m right)
    const px = ex + fx * 2.6 - (-fz) * 0.6, pz = ez + fz * 2.6 - fx * 0.6;
    return { eye: [ex, rel[1], ez], rel, fwd: f, heading: Math.atan2(fx, -fz), pitch: Math.asin(Math.max(-1, Math.min(1, f[1]))), pos: [px, pz] };
  };
  pt.bearing = (from, to) => Math.atan2(to[0] - from[0], -(to[1] - from[1]));
  // instances whose colour is within `tol` of rgb (0..255); their mean position, absolute
  pt.find = (rgb, tol) => {
    const g = window.__game;
    const a = g.instances();
    const want = rgb.map((c) => c / 255);
    let n = 0, sx = 0, sy = 0, sz = 0;
    const hits = [];
    for (let i = 0; i + 16 <= a.length; i += 16) {
      const r = a[i + 12], gg = a[i + 13], b = a[i + 14];
      if (Math.abs(r - want[0]) < tol && Math.abs(gg - want[1]) < tol && Math.abs(b - want[2]) < tol) {
        const x = a[i + 3] + pt.origin[0], y = a[i + 7], z = a[i + 11] + pt.origin[1];
        hits.push([x, y, z]);
      }
    }
    return hits;
  };
  // clusters of points (within `r` metres of each other)
  pt.clusters = (pts, r) => {
    const out = [];
    for (const p of pts) {
      const c = out.find((c) => Math.hypot(c.x - p[0], c.z - p[2]) < r);
      if (c) { c.n += 1; c.sx += p[0]; c.sy += p[1]; c.sz += p[2]; c.x = c.sx / c.n; c.y = c.sy / c.n; c.z = c.sz / c.n; }
      else out.push({ n: 1, sx: p[0], sy: p[1], sz: p[2], x: p[0], y: p[1], z: p[2] });
    }
    return out.map((c) => ({ x: c.x, y: c.y, z: c.z, n: c.n }));
  };
  const tick = () => {
    pt.frames += 1;
    if (!document.body || !window.__game) { requestAnimationFrame(tick); return; }
    if (document.body.dataset.screen === "play") pt.playFrames += 1;
    const s = pt.read();
    if (s) {
      const jumped = pt.lastXZ && Math.hypot(s.eye[0] - pt.lastXZ[0], s.eye[2] - pt.lastXZ[1]) > 3;
      if (pt.lastY !== null && pt.lastY - s.eye[1] > 3 && !jumped) pt.drops.push({ at: s.pos, from: pt.lastY, to: s.eye[1], frame: pt.frames });
      pt.lastY = s.eye[1];
      pt.lastXZ = [s.eye[0], s.eye[2]];
      const aimAt = pt.target || pt.face;
      let dx = 0, dy = 0;
      if (aimAt && document.body.dataset.screen === "play") {
        const err = wrap(pt.bearing(s.pos, aimAt) - s.heading);
        dx = Math.max(-pt.full, Math.min(pt.full, pt.sign * err * pt.gain * pt.full));
      }
      if (pt.pitch !== null && document.body.dataset.screen === "play") {
        const e = pt.pitch - s.pitch;
        dy = Math.max(-200, Math.min(200, -pt.signY * e * pt.gain * pt.full));
      }
      if (dx || dy) window.dispatchEvent(new MouseEvent("mousemove", { movementX: dx, movementY: dy }));
    }
    requestAnimationFrame(tick);
  };
  pt.signY = 1;
  requestAnimationFrame(tick);
})();
"""

# Text in the menus and over the game that overlaps other text or is clipped.
TEXT_CHECK = r"""
() => {
  const roots = ["#menus", "#overlay", "#hud"].map((s) => document.querySelector(s)).filter(Boolean);
  const leaves = [];
  const visible = (el) => {
    const st = getComputedStyle(el);
    if (st.visibility === "hidden" || st.display === "none" || Number(st.opacity) === 0) return false;
    for (let p = el; p; p = p.parentElement) {
      const s = getComputedStyle(p);
      if (s.display === "none" || Number(s.opacity) === 0 || s.visibility === "hidden") return false;
      if (p.hidden) return false;
      if (p.getAttribute && p.getAttribute("aria-hidden") === "true") return false;
    }
    return true;
  };
  for (const root of roots) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    for (let t = walker.nextNode(); t; t = walker.nextNode()) {
      if (!t.textContent.trim()) continue;
      const el = t.parentElement;
      if (!visible(el)) continue;
      const range = document.createRange();
      range.selectNodeContents(t);
      const rects = [...range.getClientRects()].filter((r) => r.width > 1 && r.height > 1);
      if (!rects.length) continue;
      leaves.push({ el, text: t.textContent.trim().slice(0, 40), rects });
    }
  }
  const issues = [];
  const W = innerWidth, H = innerHeight;
  for (const l of leaves) {
    for (const r of l.rects) {
      if (r.left < -1 || r.top < -1 || r.right > W + 1 || r.bottom > H + 1) { issues.push(`off screen: "${l.text}"`); break; }
    }
    const el = l.el;
    const st = getComputedStyle(el);
    if ((st.overflow !== "visible" || st.textOverflow === "ellipsis") && (el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 2)) issues.push(`clipped: "${l.text}" (${el.scrollWidth}x${el.scrollHeight} in ${el.clientWidth}x${el.clientHeight})`);
    // clipped by an ancestor that hides overflow
    for (let p = el.parentElement; p && p !== document.body; p = p.parentElement) {
      const s = getComputedStyle(p);
      if (s.overflow === "hidden" || s.overflowX === "hidden" || s.overflowY === "hidden") {
        const pr = p.getBoundingClientRect();
        // a face of the ring turned away is not shown text
        if (pr.width < 2) break;
        const out = l.rects.some((r) => r.left < pr.left - 1 || r.right > pr.right + 1 || r.top < pr.top - 1 || r.bottom > pr.bottom + 1);
        if (out) issues.push(`clipped by ${p.className || p.id}: "${l.text}"`);
        break;
      }
    }
  }
  for (let i = 0; i < leaves.length; i++) {
    for (let j = i + 1; j < leaves.length; j++) {
      const a = leaves[i], b = leaves[j];
      if (a.el === b.el || a.el.contains(b.el) || b.el.contains(a.el)) continue;
      const hit = a.rects.some((r) => b.rects.some((q) => Math.min(r.right, q.right) - Math.max(r.left, q.left) > 2 && Math.min(r.bottom, q.bottom) - Math.max(r.top, q.top) > 2));
      if (hit) issues.push(`overlap: "${a.text}" / "${b.text}"`);
    }
  }
  return [...new Set(issues)];
}
"""


class Run:
    def __init__(self, page, out, dev):
        self.page = page
        self.out = out
        self.t0 = time.perf_counter()
        self.log_path = out / "playthrough-log.txt"
        self.log_path.write_text("")
        self.issues = []
        self.shots = 0
        self.act_used = False
        self.step_name = ""
        self.step_t0 = self.t0
        self.unhealthy = []
        if dev:
            self.log("INFO", f"DEV START at {dev}: not a playthrough")

    # ---- logging ----
    def log(self, kind, text):
        line = f"[{time.perf_counter() - self.t0:8.1f}s] [{self.step_name or '-'}] {kind}: {text}"
        print(line, flush=True)
        with self.log_path.open("a") as f:
            f.write(line + "\n")
        if kind == "ISSUE":
            self.issues.append(f"{self.step_name}: {text}")

    def shot(self, name):
        self.shots += 1
        path = self.out / f"{self.shots:02d}-{name}.png"
        try:
            self.page.screenshot(path=str(path))
        except Exception as e:
            self.log("INFO", f"screenshot {name} failed: {e}")
            return None
        self.log("INFO", f"screenshot {path.name}")
        return path

    # ---- reading ----
    def state(self):
        return self.page.evaluate(
            """() => {
              const s = window.__pt && window.__pt.read();
              const hud = {};
              document.querySelectorAll('#hud .hud-item').forEach((n) => hud[n.id] = n.classList.contains('shown') ? n.textContent : null);
              return { s, screen: document.body.dataset.screen, hud, play: window.__pt ? window.__pt.playFrames : 0 };
            }"""
        )

    def pos(self):
        st = self.state()
        return st["s"]["pos"] if st["s"] else None

    def ui(self):
        return self.page.evaluate("() => JSON.parse(window.__game.ui()).ok")

    def screen(self):
        return self.page.evaluate("() => document.body.dataset.screen")

    def prompt(self):
        return self.page.evaluate("() => { const n = document.getElementById('hud-prompt'); return n && n.classList.contains('shown') ? n.textContent : ''; }")

    def story_line(self):
        return self.page.evaluate("() => { const n = document.getElementById('hud-story'); return n && n.classList.contains('shown') ? n.textContent : ''; }")

    def health(self):
        return self.page.evaluate("() => { const b = document.querySelector('#play-hud .bar-health .bar-text'); return b ? b.textContent : ''; }")

    def find(self, colour, tol=0.012, r=3.0):
        rgb = MODELS[colour] if isinstance(colour, str) else colour
        return self.page.evaluate("([rgb, tol, r]) => window.__pt.clusters(window.__pt.find(rgb, tol), r)", [rgb, tol, r])

    def nearest(self, colour, tol=0.012, r=3.0, most=None):
        p = self.pos()
        cs = self.find(colour, tol, r)
        if not cs or p is None:
            return None
        cs.sort(key=lambda c: math.hypot(c["x"] - p[0], c["z"] - p[1]))
        c = cs[0]
        d = math.hypot(c["x"] - p[0], c["z"] - p[1])
        if most is not None and d > most:
            return None
        return [c["x"], c["z"], d]

    def text_check(self, where):
        issues = self.page.evaluate(TEXT_CHECK)
        for i in issues:
            self.log("ISSUE", f"text at {where}: {i}")
        if not issues:
            self.log("PASS", f"text at {where}: no overlap or clipping")
        return issues

    def drops(self):
        d = self.page.evaluate("() => window.__pt.drops.splice(0)")
        for x in d:
            self.log("ISSUE", f"the view dropped {x['from'] - x['to']:.1f} m in one frame at ({x['at'][0]:.0f}, {x['at'][1]:.0f}): falling through the ground or a step")
        return d

    # ---- input ----
    def frames(self, n):
        self.page.evaluate(
            "(n) => new Promise((done) => { let k = 0; const f = () => (++k >= n ? done() : requestAnimationFrame(f)); requestAnimationFrame(f); })",
            n,
        )

    def wait(self, s):
        self.frames(max(1, int(s * 60)))

    def press(self, key, hold_frames=3):
        self.page.keyboard.down(key)
        self.frames(hold_frames)
        self.page.keyboard.up(key)
        self.frames(2)

    def hold(self, *keys):
        for k in keys:
            self.page.keyboard.down(k)

    def release(self, *keys):
        for k in keys or ("KeyW", "ShiftLeft", "KeyA", "KeyD", "KeyS"):
            self.page.keyboard.up(k)

    def move_mouse(self, dx, dy, steps=1):
        self.page.evaluate(
            """([dx, dy, steps]) => new Promise((done) => {
                let k = 0;
                const s = () => { window.dispatchEvent(new MouseEvent('mousemove', {movementX: dx, movementY: dy})); k += 1; if (k < steps) requestAnimationFrame(s); else done(); };
                requestAnimationFrame(s);
            })""",
            [dx, dy, steps],
        )

    def steer(self, target=None, face=None, pitch=None):
        self.page.evaluate(
            "([t, f, p]) => { window.__pt.target = t; window.__pt.face = f; window.__pt.pitch = p; }",
            [target, face, pitch],
        )

    def calibrate(self):
        """Which way +movementX turns the view, and +movementY tips it."""
        a = self.state()["s"]
        self.move_mouse(40, 0, 3)
        self.frames(4)
        b = self.state()["s"]
        d = b["heading"] - a["heading"]
        d = (d + math.pi) % (2 * math.pi) - math.pi
        sign = 1 if d > 0 else -1
        self.move_mouse(-40, 0, 3)
        self.move_mouse(0, 30, 3)
        self.frames(4)
        c = self.state()["s"]
        signy = 1 if c["pitch"] < b["pitch"] else -1
        self.move_mouse(0, -30, 3)
        self.frames(3)
        rad_per_px = abs(d) / 120 if d else 0
        # aim for about a fifth of the error a frame
        full = 0.2 / rad_per_px / math.pi if rad_per_px else 600
        self.page.evaluate("([s, sy, full]) => { window.__pt.sign = s; window.__pt.signY = sy; window.__pt.full = full; window.__pt.gain = 1; }", [sign, signy, full * 1.0])
        self.log("INFO", f"calibrated: +x turns {'right' if sign > 0 else 'left'}, {rad_per_px * 1000:.2f} mrad a px; +y looks {'down' if signy > 0 else 'up'}")

    def fix_origin(self):
        """The map's "you" mark gives the player's place in the square (to
        0.01% of 4 km): the frame's origin is the 256 m cell that agrees."""
        self.press("KeyM")
        self.frames(8)
        u = self.ui()
        menu = u.get("menu")
        if not menu:
            self.log("ISSUE", "M did not open the map")
            return
        face = next(f for f in menu["faces"] if f["id"] == "map")
        you = face["map"]["you"]
        x = float(you["left"].rstrip("%")) / 100 * 4000
        z = float(you["top"].rstrip("%")) / 100 * 4000
        self.press("KeyM")
        self.frames(8)
        if self.screen() != "play":
            self.log("ISSUE", f"M on the map did not close it (screen {self.screen()})")
            self.page.keyboard.press("Tab")
            self.frames(8)
        st = self.state()["s"]
        rel = st["rel"]
        # the player stands about 2.6 m ahead of the eye; the origin is a 256 m multiple
        ox = round((x - st["pos"][0] + st["eye"][0] - rel[0]) / 256) * 256
        oz = round((z - st["pos"][1] + st["eye"][2] - rel[2]) / 256) * 256
        ox = round((x - (st["pos"][0] - st["eye"][0]) - rel[0]) / 256) * 256
        oz = round((z - (st["pos"][1] - st["eye"][2]) - rel[2]) / 256) * 256
        self.page.evaluate("([ox, oz]) => { window.__pt.origin = [ox, oz]; window.__pt.last = null; }", [ox, oz])
        p = self.pos()
        self.log("INFO", f"the map puts the player at ({x:.0f}, {z:.0f}); the frame's origin ({ox}, {oz}); tracked ({p[0]:.0f}, {p[1]:.0f})")

    def goto(self, target, run=True, tol=2.5, timeout=240, label=None, stop_if=None):
        """Walks (or runs) to `target`, steering the view toward it. Returns
        True when within `tol`; logs being stuck."""
        label = label or f"({target[0]:.0f}, {target[1]:.0f})"
        self.steer(target=target, pitch=-0.12)
        self.frames(6)
        keys = ["KeyW"] + (["ShiftLeft"] if run else [])
        self.hold(*keys)
        t0 = time.perf_counter()
        last_p, last_t = None, t0
        stuck_turns = 0
        try:
            while time.perf_counter() - t0 < timeout:
                self.frames(6)
                st = self.state()
                if st["screen"] != "play":
                    self.release()
                    self.handle_screen(st["screen"])
                    self.hold(*keys)
                    continue
                p = st["s"]["pos"]
                d = math.hypot(target[0] - p[0], target[1] - p[1])
                if stop_if and stop_if(st):
                    return True
                if d < tol:
                    return True
                if d < 6 and run:
                    self.release("ShiftLeft")
                now = time.perf_counter()
                if last_p is None:
                    last_p, last_t = p, now
                elif now - last_t > 2.5:
                    moved = math.hypot(p[0] - last_p[0], p[1] - last_p[1])
                    if moved < 1.0:
                        stuck_turns += 1
                        self.log("ISSUE" if stuck_turns == 1 else "INFO", f"stuck walking to {label} at ({p[0]:.1f}, {p[1]:.1f}); stepping round it ({stuck_turns})")
                        self.release()
                        side = "KeyA" if stuck_turns % 2 else "KeyD"
                        self.hold("KeyS")
                        self.wait(0.5)
                        self.release("KeyS")
                        self.hold(side, "KeyW")
                        self.wait(1.2)
                        self.release(side)
                        self.hold(*keys)
                        if stuck_turns > 8:
                            self.log("ISSUE", f"could not reach {label}: stuck at ({p[0]:.1f}, {p[1]:.1f})")
                            return False
                    last_p, last_t = p, now
            p = self.pos()
            self.log("ISSUE", f"timed out walking to {label}; at ({p[0]:.0f}, {p[1]:.0f}), {math.hypot(target[0] - p[0], target[1] - p[1]):.0f} m off")
            return False
        finally:
            self.release()
            self.steer(None, None, None)
            self.drops()

    def path(self, points, run=True, tol=4, timeout_per=200, label="path"):
        """Walks the points in order, from the one nearest where the player
        stands (a later start skips those behind)."""
        p = self.pos()
        near = min(range(len(points)), key=lambda k: math.hypot(points[k][0] - p[0], points[k][1] - p[1]))
        if near + 1 < len(points) and math.hypot(points[near][0] - p[0], points[near][1] - p[1]) < 25:
            near += 1
        for k, q in list(enumerate(points))[near:]:
            last = k == len(points) - 1
            if not self.goto(q, run=run, tol=(2.0 if last else tol), timeout=timeout_per, label=f"{label} point {k + 1}/{len(points)}"):
                return False
        return True

    def handle_screen(self, screen):
        """A screen the walk did not expect (a dialogue, the dream line):
        read it on with Space, logged."""
        if screen == "dialogue":
            said = self.page.evaluate("() => document.querySelector('#talk .talk-text') ? document.querySelector('#talk .talk-text').textContent : ''")
            self.log("INFO", f"dialogue: {said!r}")
            self.read_dialogue()
        elif screen == "paused":
            self.log("ISSUE", "the game paused itself during a walk")
            self.page.click("#paused button")
            self.frames(10)
        elif screen == "shop":
            self.page.keyboard.press("Escape")
            self.frames(10)
        elif screen == "menu":
            self.page.keyboard.press("Tab")
            self.frames(10)

    def read_dialogue(self, timeout=30):
        """Space until the dialogue closes; the lines said."""
        lines = []
        t0 = time.perf_counter()
        while self.screen() == "dialogue" and time.perf_counter() - t0 < timeout:
            self.wait(0.6)
            said = self.page.evaluate("() => { const t = document.querySelector('#talk .talk-text'); return t ? t.textContent : ''; }")
            if not lines or lines[-1] != said:
                lines.append(said)
            self.press("Space")
        if self.screen() == "dialogue":
            self.log("ISSUE", "the dialogue did not close with Space")
        return lines

    def step(self, name):
        self.step_name = name
        self.step_t0 = time.perf_counter()
        self.log("INFO", "step begins")

    def done(self, ok, text):
        took = time.perf_counter() - self.step_t0
        self.log("PASS" if ok else "ISSUE", f"{text} (step took {took:.0f} s)")
        return ok


# ---------------------------------------------------------------- the steps


def road_points_between(x_from, x_to):
    """The merchant's road's points from x_from to x_to (either way)."""
    pts = [p for p in ROAD if min(x_from, x_to) <= p[0] <= max(x_from, x_to)]
    pts.sort(key=lambda p: p[0], reverse=x_from > x_to)
    return pts


def road_at(x):
    for a, b in zip(ROAD, ROAD[1:]):
        if a[0] <= x <= b[0]:
            t = (x - a[0]) / (b[0] - a[0])
            return [x, a[1] + t * (b[1] - a[1])]
    return [x, ROAD[-1][1]]


def s_title(r):
    r.step("title and play")
    p = r.page
    p.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    r.frames(30)
    r.shot("title")
    r.text_check("the title")
    # focus is visible on the first button
    focused = p.evaluate("() => document.activeElement && document.activeElement.textContent")
    r.log("INFO", f"focused on the title: {focused!r}")
    p.click("#title button")
    p.wait_for_selector("body[data-screen='play']", state="attached", timeout=10000)
    r.frames(10)
    r.calibrate()
    r.fix_origin()
    r.shot("in-bed")
    # wakes on its own after 1.5 s
    t0 = time.perf_counter()
    woke = ""
    while time.perf_counter() - t0 < 15:
        r.wait(0.25)
        woke = r.story_line()
        if woke:
            break
    r.shot("woke")
    return r.done(bool(woke), f"woke: {woke!r}")


def s_dream(r):
    r.step("down to an orc, the dream")
    # out of the door (the house's door faces west) and toward the green
    home = place("village", (-34, 6))
    door = [home[0] - 6, home[1]]
    r.goto(door, run=False, tol=1.5, timeout=30, label="the door")
    # round the house's south side to the green
    r.path([[home[0] - 8, home[1] + 8], [home[0] + 8, home[1] + 8], [1990, 2812]], run=False, label="round the house to the green")
    # stand, sword away, until an orc comes and puts the player down
    t0 = time.perf_counter()
    seen_down = False
    line = []
    while time.perf_counter() - t0 < 150:
        orc = r.nearest("orc_skin", most=40)
        if orc:
            r.steer(face=orc[:2])
        r.wait(0.4)
        scr = r.screen()
        veil = r.page.evaluate("() => { const n = document.getElementById('hud-veil'); return n && n.classList.contains('shown'); }")
        if veil and not seen_down:
            seen_down = True
            r.shot("down-veil")
        if scr == "dialogue":
            r.wait(1.5)
            r.shot("dream-line")
            line = r.read_dialogue()
            break
    r.steer(None)
    ok = any("funny dream" in l for l in line)
    if not seen_down:
        r.log("ISSUE", "no orc put the player down in 150 s of standing on the green")
    r.wait(2)
    r.shot("back-in-bed")
    return r.done(ok, f"the dream line: {line!r}")


def s_boundary(r):
    r.step("run to the boundary")
    home = place("village", (-34, 6))
    # wait to be up (or press W to get up)
    r.press("KeyW", 6)
    r.wait(2.0)
    r.page.evaluate("() => { window.__pt.reset = window.__pt.playFrames; }")
    door = [home[0] - 6, home[1]]
    route = [door, [home[0] - 9, home[1]], [1940, 2800], [1940, 2700], [1994, 2650], [1996, 2600], [1995, 2520]]
    ok = r.path(route, run=True, label="out of the village")
    line = ""
    for _ in range(20):
        line = r.story_line()
        if line:
            break
        r.wait(0.2)
    r.shot("boundary")
    p = r.pos()
    c = PLACES["village"]
    dist = math.hypot(p[0] - c[0], p[1] - c[1])
    return r.done(ok and dist > 250, f"{dist:.0f} m from the village centre; line: {line!r}")


def cart_estimate(r):
    """Where the cart is, by its canopy and its merchant's hat in the frame."""
    for colour in ("merchant_hat", "merchant_shirt"):
        c = r.nearest(colour, r=4.0)
        if c:
            return c
    return None


def s_cart(r):
    r.step("stop the cart and shop")
    # the rocks at the crossroads, on the way
    rocks = pickup_at("rock")
    rocks = [PLACES["crossroads"][0] + 7, PLACES["crossroads"][1] + 9]
    r.path([[1996, 2450], [1999, 2300], rocks], run=True, label="the crossroads' rocks")
    got = False
    for _ in range(6):
        pr = r.prompt()
        if "Rock" in pr or "rock" in pr:
            r.press("KeyE")
            r.wait(0.3)
            got = True
        else:
            break
    r.log("PASS" if got else "ISSUE", f"rocks at the crossroads: {'picked up' if got else 'no prompt to pick up'} (prompt {pr!r})")
    # west along the road, looking for the cart
    pts = road_points_between(2000, 470)
    seen = None
    x = 1990
    while x > 480 and not seen:
        x -= 40
        tgt = road_at(x)
        r.goto(tgt, run=True, tol=5, timeout=40, label=f"the road at x {x:.0f}")
        seen = cart_estimate(r)
        if seen and seen[2] > 80:
            seen = None
    if not seen:
        r.shot("no-cart")
        return r.done(False, "the cart was not seen on the road")
    r.log("INFO", f"the cart seen at ({seen[0]:.0f}, {seen[1]:.0f}), {seen[2]:.0f} m off")
    # get ahead of it (it heads west), turn and face it, throw
    stopped = False
    for attempt in range(8):
        c = cart_estimate(r)
        if not c:
            r.wait(0.5)
            continue
        ahead = road_at(c[0] - 9)
        r.goto(ahead, run=True, tol=2.0, timeout=60, label="ahead of the cart")
        c = cart_estimate(r) or c
        r.steer(face=c[:2], pitch=-0.02)
        r.wait(0.8)
        # the quick key holding rocks
        keys = r.page.evaluate("() => [...document.querySelectorAll('#play-hud .hud-quick .quick')].map((q) => q.dataset.item)")
        if "rock" in keys:
            r.press(f"Digit{keys.index('rock') + 1}")
        r.shot(f"throw-{attempt + 1}")
        r.press("KeyF", 4)
        r.wait(1.5)
        line = r.story_line()
        r.steer(None)
        if line and "stop" in line.lower():
            stopped = True
            r.log("PASS", f"the cart stopped: {line!r} (throw {attempt + 1})")
            break
        r.log("INFO", f"throw {attempt + 1} did not stop the cart (line {line!r})")
        # pick the rock up again if it lies near
        pr = r.prompt()
        if "ock" in pr:
            r.press("KeyE")
    if not stopped:
        r.shot("cart-not-stopped")
        return r.done(False, "the cart was not stopped by a thrown rock")
    r.shot("cart-stopped")
    c = cart_estimate(r)
    if c:
        r.goto([c[0], c[1] + 2.0], run=False, tol=1.5, timeout=20, label="beside the cart")
    for _ in range(10):
        pr = r.prompt()
        if "erchant" in pr:
            break
        c = cart_estimate(r)
        if c:
            r.goto(c[:2], run=False, tol=2.0, timeout=8, label="the cart")
    pr = r.prompt()
    r.log("INFO", f"prompt by the cart: {pr!r}")
    r.press("KeyE")
    r.wait(1)
    if r.screen() == "dialogue":
        r.shot("merchant-hello")
        r.read_dialogue()
    r.wait(0.5)
    if r.screen() != "shop":
        r.shot("no-shop")
        return r.done(False, f"the shop did not open (screen {r.screen()})")
    r.shot("shop")
    r.text_check("the shop")
    shop = r.ui()["shop"]
    coins = shop["coins"]["text"]
    hood = r.page.query_selector("#shop .shop-row[data-key='hood'] .row-button")
    if hood:
        r.log("INFO", f"coins {coins}; the hood's button {'enabled' if hood.is_enabled() else 'disabled'}")
        if hood.is_enabled():
            hood.click()
            r.wait(0.4)
            r.state_bought = True
            r.log("PASS", f"bought the hood: {r.page.text_content('#shop .shop-note')!r}")
        else:
            r.log("ISSUE", f"the hood costs more than the player has at the first meeting ({coins} coins): it cannot be bought on the first stop; the route needs a second meeting after the camp")
    else:
        r.log("ISSUE", "the hood is not in the shop")
    r.shot("shop-after")
    leave = r.page.query_selector("#shop .leave")
    leave.click()
    r.wait(0.5)
    return r.done(r.screen() == "play", "left the shop")


def s_trainer(r):
    r.step("the trainer's practice")
    br = PLACES["bridge"]
    road = road_points_between(r.pos()[0], 700)
    r.path(road + [[700, 2242], [br[0] + 20, br[1] - 2], [br[0] - 22, br[1] + 1]], run=True, label="to the bridge")
    trail = [list(map(float, p)) for p in HUT_TRAIL]
    r.path(trail[1:-1] + [[338, 1764]], run=True, label="up the trail to the hut")
    # the trainer stands by the signpost
    t = r.nearest("trainer_coat", r=2.5, most=40)
    if not t:
        r.shot("no-trainer")
        return r.done(False, "the trainer was not seen by the hut")
    r.goto(t[:2], run=False, tol=1.8, timeout=20, label="the trainer")
    pr = r.prompt()
    r.log("INFO", f"prompt by the trainer: {pr!r}")
    r.press("KeyE")
    r.wait(0.8)
    lines = r.read_dialogue()
    r.log("INFO", f"the trainer said {len(lines)} lines")
    r.shot("trainer-talked")
    # draw and keep the guard toward the trainer
    t = r.nearest("trainer_coat", r=2.5, most=40) or t
    r.steer(face=t[:2])
    r.wait(0.6)
    r.steer(None)
    r.press("KeyX")
    r.wait(1.0)
    r.shot("practice-drawn")
    t0 = time.perf_counter()
    learned = False
    k = 0
    while time.perf_counter() - t0 < 240:
        scr = r.screen()
        if scr == "dialogue":
            said = r.read_dialogue()
            r.log("INFO", f"trainer: {said!r}")
            if any("yours" in s.lower() or "q " in s.lower() for s in said):
                learned = True
        if learned or "Q" in (r.story_line() or "")[:3]:
            learned = True
            break
        # small sweeps of the hand across the guard
        k += 1
        r.move_mouse(8 if (k // 20) % 2 else -8, 0, 4)
        r.wait(0.1)
        if k == 60:
            r.shot("practice-volley")
    line = r.story_line()
    r.shot("practice-done")
    r.press("KeyX")
    r.wait(0.8)
    return r.done(learned, f"the practice: line {line!r}")


CAMP_COLOURS = ("bandit_hood", "lookout_hood", "leader_mantle")


def foes_near(r, most=30, standing=True):
    """Camp foes in the frame by their hoods and the leader's mantle, nearest
    first: (distance, colour, [x, z], height over the player's feet). A
    hood under a metre over the ground is a foe lying down."""
    st = r.state()["s"]
    p = st["pos"]
    ground = st["eye"][1] - 2.1
    out = []
    for c in CAMP_COLOURS:
        for k in r.find(c, tol=0.004, r=1.2):
            d = math.hypot(k["x"] - p[0], k["z"] - p[1])
            up = k["y"] - ground
            if d < most and (not standing or up > 0.9):
                out.append((d, c, [k["x"], k["z"]], up))
    out.sort()
    return out


def hp(r):
    t = r.health()
    try:
        return int(t.split("/")[0])
    except ValueError:
        return 100


def veiled(r):
    return r.page.evaluate("() => { const n = document.getElementById('hud-veil'); return n && n.classList.contains('shown'); }")


def drawn(r):
    """Whether the sword is out: the HUD's foe bar shows only when held on a
    foe, so this reads the controls' hint instead: the view no longer turns
    with the mouse."""
    a = r.state()["s"]["heading"]
    r.move_mouse(30, 0, 2)
    r.frames(3)
    b = r.state()["s"]["heading"]
    r.move_mouse(-30, 0, 2)
    r.frames(2)
    return abs(b - a) < 1e-3


def swing(r, k):
    """A side cut: the hand wound out, then the left button held and the
    mouse flicked across (tools/shoot.py's committed swing)."""
    side = 1 if k % 2 == 0 else -1
    r.move_mouse(45 * side, -4, 10)
    r.page.mouse.move(960, 540)
    r.page.mouse.down()
    r.move_mouse(-150 * side, 0, 8)
    r.page.mouse.up()
    r.frames(12)


def draw_sword(r):
    if not drawn(r):
        r.press("KeyX")
        r.wait(0.9)


def fight(r, timeout, until, low=0):
    """Sword out, close on the nearest standing foe, and swing side cuts left
    and right. Ends when `until()` is true (True), the player is down or
    under `low` health, or after `timeout` s (False)."""
    t0 = time.perf_counter()
    draw_sword(r)
    k = 0
    imbued_at = -99
    while time.perf_counter() - t0 < timeout:
        if until():
            r.release()
            return True
        scr = r.screen()
        if scr != "play":
            r.release()
            r.handle_screen(scr)
            continue
        if veiled(r) or hp(r) <= 0:
            r.release()
            r.log("INFO", "the player went down in the fight")
            return False
        if low and hp(r) < low:
            r.release()
            r.log("INFO", f"health {hp(r)}: breaking off")
            return False
        f = foes_near(r, 25)
        if time.perf_counter() - imbued_at > 21:
            r.press("KeyQ")
            imbued_at = time.perf_counter()
        if f and f[0][0] > 1.6:
            r.hold("KeyW")
        else:
            r.release("KeyW")
        swing(r, k)
        k += 1
    r.release()
    return False


def come_back_to_camp(r):
    """From wherever the player came to, back to the camp's trail."""
    p = r.pos()
    trail = [list(map(float, q)) for q in CAMP_TRAIL]
    if p[1] > 1950:
        r.path([[1996, 2450], [2000, 2260]] if p[0] < 2100 else [[p[0], p[1]]], run=True, label="up the lane")
        r.path(road_points_between(2000, 2860) + [[2870, 1888]], run=True, label="east on the road")
    r.path(trail[1:-1] + [[3175, 1075]], run=True, label="up the trail to the camp")


def s_camp(r):
    r.step("the bandit camp")
    p = r.pos()
    if p[0] < 2000:
        # from the west bank: over the bridge, east on the road
        r.path([[640, 2247], [700, 2242]], run=True, label="over the bridge")
        r.path(road_points_between(700, 2860) + [[2870, 1888]], run=True, label="east on the road")
    trail = [list(map(float, q)) for q in CAMP_TRAIL]
    r.path(trail[1:-1] + [[3175, 1075]], run=True, label="up the trail to the camp")
    r.shot("camp-approach")
    cleared = lambda: "camp is down" in (r.story_line() or "") or "camp is down" in r.page.evaluate("() => document.getElementById('hud-story') ? document.getElementById('hud-story').textContent : ''")
    t0 = time.perf_counter()
    # by input first: into the clearing, and fight
    r.goto([3200, 1012], run=False, tol=3, timeout=40, label="into the clearing", stop_if=lambda st: bool(foes_near(r, 9)))
    r.shot("camp-alarm")
    won = fight(r, 120, cleared, low=35)
    r.shot("camp-fight")
    down = veiled(r) or hp(r) <= 0
    if won:
        r.log("PASS", f"the camp cleared by input in {time.perf_counter() - t0:.0f} s")
    else:
        r.log("ISSUE", f"the camp not cleared by input in {time.perf_counter() - t0:.0f} s of fighting (health {hp(r)}, {'down' if down else 'breaking off'}); {len(foes_near(r, 40))} foes standing in sight")
        if down:
            r.wait(8)
            r.log("INFO", f"came to at ({r.pos()[0]:.0f}, {r.pos()[1]:.0f}): {r.story_line()!r}")
            r.shot("came-to")
            come_back_to_camp(r)
        else:
            # away down the trail past the foes' leash (70 m from home), until they go home
            r.path([[3180, 1060], [3130, 1180]], run=True, label="away down the trail")
            r.wait(8)
        # the one test act: the camp's foes held still for the fight's outcome
        r.act_used = True
        r.log("INFO", "TEST ACT USED: story.still on (every foe's input record held empty), for the camp fight's outcome only; the player still cuts each foe down by input")
        r.page.evaluate("() => window.__act('story.still', 'on')")
        r.goto([3200, 1012], run=True, tol=4, timeout=60, label="back into the clearing")
        t1 = time.perf_counter()
        while not cleared() and time.perf_counter() - t1 < 400:
            f = foes_near(r, 45)
            if not f:
                r.log("INFO", "no standing foe in sight; looking round")
                r.move_mouse(60, 0, 20)
                r.frames(10)
                if not foes_near(r, 45):
                    break
                continue
            d, colour, at, up = f[0]
            r.goto(at, run=False, tol=1.4, timeout=30, label=f"the {colour.split('_')[0]}")
            fight(r, 25, lambda: cleared() or not any(math.hypot(q[2][0] - at[0], q[2][1] - at[1]) < 1.5 for q in foes_near(r, 6)), low=0)
        won = cleared()
        r.page.evaluate("() => window.__act('story.still', 'off')")
        r.log("INFO", "TEST ACT ended: story.still off")
    r.wait(0.5)
    r.shot("camp-after-fight")
    if r.page.evaluate("() => true") and drawn(r):
        r.press("KeyX")
        r.wait(0.8)
    return r.done(won, f"the camp is {'down' if won else 'not down'}")


def pick_up_round(r, centre, radius, timeout=90, want=None):
    """Walks to each thing lying round `centre` whose prompt says "Pick up",
    and presses E; returns the prompts taken."""
    taken = []
    t0 = time.perf_counter()
    spots = [[centre[0] + radius * math.cos(a), centre[1] + radius * math.sin(a)] for a in [i * math.pi / 4 for i in range(8)]]
    spots = [centre] + spots
    for sp in spots:
        if time.perf_counter() - t0 > timeout:
            break
        r.goto(sp, run=False, tol=1.0, timeout=20, label="a spot to look for drops")
        for _ in range(8):
            pr = r.prompt()
            if "Pick up" in pr:
                r.press("KeyE")
                r.wait(0.4)
                taken.append(pr)
            else:
                break
    return taken


def collect_near(r, at, label, tries=12):
    """Goes to `at` and presses E while the prompt offers to pick something up."""
    r.goto(at, run=True, tol=1.2, timeout=240, label=label)
    got = []
    for k in range(tries):
        pr = r.prompt()
        if "Pick up" in pr or "Open" in pr:
            r.press("KeyE")
            r.wait(0.5)
            got.append(pr)
        else:
            # nudge round the spot
            ang = k * math.pi / 3
            r.goto([at[0] + 1.2 * math.cos(ang), at[1] + 1.2 * math.sin(ang)], run=False, tol=0.6, timeout=6, label=label)
    return got


def s_drops(r):
    r.step("the camp's drops and the chest")
    c = PLACES["camp"]
    lying = foes_near(r, 60, standing=False)
    got = []
    for d, colour, at, up in lying:
        got += collect_near(r, at, f"the fallen {colour.split('_')[0]}", tries=6)
    got += pick_up_round(r, [c[0] + 7, c[1] + 2], 2.0)
    r.log("INFO", f"picked up at the camp: {got}")
    inv = r.page.evaluate("() => JSON.parse(window.__game.ui()).ok")
    chest = [c[0] + 11, c[1] + 5]
    r.goto([chest[0] - 1.6, chest[1] - 0.6], run=False, tol=0.8, timeout=30, label="the chest")
    pr = ""
    for k in range(10):
        pr = r.prompt()
        if "chest" in pr:
            break
        ang = k * math.pi / 5
        r.goto([chest[0] + 1.7 * math.cos(ang), chest[1] + 1.7 * math.sin(ang)], run=False, tol=0.5, timeout=6, label="round the chest")
    r.log("INFO", f"prompt at the chest: {pr!r}")
    r.press("KeyE")
    r.wait(1.0)
    line = r.story_line()
    toast = r.page.evaluate("() => { const t = document.querySelector('#play-hud .hud-toast'); return t && t.classList.contains('shown') ? t.textContent : ''; }")
    r.shot("chest")
    ok = "oomerang" in toast
    if not ok:
        r.log("ISSUE", f"the chest did not give the boomerang (line {line!r}, toast {toast!r})")
    return r.done(ok, f"the chest: {toast or line!r}")


def menu_open(r, face=None):
    if r.screen() != "menu":
        r.press("Tab")
        r.wait(0.6)
    if face:
        for _ in range(4):
            u = r.ui().get("menu")
            if u and u["faces"][u["face"]]["id"] == face:
                break
            r.press("KeyE")
            r.wait(0.7)
    return r.ui().get("menu")


def menu_close(r):
    if r.screen() == "menu":
        r.press("Tab")
        r.wait(0.5)


def s_boomerang(r):
    r.step("throw the boomerang")
    m = menu_open(r, "select")
    slot = r.page.query_selector("#menu .face-select .slot[data-item='boomerang']")
    if not slot:
        r.shot("no-boomerang-slot")
        menu_close(r)
        return r.done(False, "the boomerang is not in Select Item")
    slot.hover()
    r.wait(0.3)
    r.press("Digit1")
    r.wait(0.3)
    r.shot("boomerang-on-key")
    note = r.page.evaluate("() => { const n = document.querySelector('#menu .face-select .info-notes'); return n ? n.textContent : ''; }")
    r.log("INFO", f"Select Item, the boomerang: {note!r}")
    menu_close(r)
    r.press("Digit1")
    r.wait(0.3)
    p = r.pos()
    r.press("KeyF", 4)
    far = 0.0
    seen_back = False
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < 4.5:
        r.frames(6)
        b = r.nearest("boomerang_paint", tol=0.01, r=1.0)
        if b:
            far = max(far, b[2])
            if far > 6 and b[2] < 1.5:
                seen_back = True
        if far > 4 and int((time.perf_counter() - t0) * 10) == 12:
            r.shot("boomerang-flight")
    r.shot("boomerang-after")
    toast = r.page.evaluate("() => [...document.querySelectorAll('#play-hud .hud-quick .quick')].map((q) => q.dataset.item)")
    ok = far > 8
    if not seen_back:
        r.log("INFO" if ok else "ISSUE", f"the boomerang's return to the hand was not seen in the frame (it flew {far:.1f} m out)")
    return r.done(ok, f"the boomerang flew {far:.1f} m out; quick keys {toast}")


def s_shrine(r):
    r.step("the boots at the shrine")
    p = r.pos()
    boots = pickup_at("boots")
    # from the hut along the river's west bank to the ford
    r.path([[420, 1600], [560, 1350], [650, 1150], [712, 1005], [730, 1003]], run=True, label="to the shrine's ford")
    got = collect_near(r, boots, "the boots")
    r.shot("shrine")
    ok = any("oots" in g for g in got)
    return r.done(ok, f"at the shrine: {got}")


def s_shrine_to_road(r):
    r.step("from the shrine to the road")
    ok = r.path([[760, 1005], [780, 1200], [760, 1500], [720, 1900], [700, 2242]], run=True, label="down the east bank")
    return r.done(ok, "at the road east of the bridge")


def s_trousers(r):
    r.step("the trousers in the hollow tree")
    trail = [list(map(float, q)) for q in CAMP_TRAIL][::-1]
    r.path(trail[1:] + [[2900, 1880]], run=True, label="down the trail to the road")
    r.path(road_points_between(2870, 3400) + [[3550, 1775], [3590, 1600], [3600, 1420]], run=True, label="to the hollow tree")
    got = collect_near(r, pickup_at("trousers"), "the trousers")
    r.shot("hollow-tree")
    ok = any("rousers" in g for g in got)
    return r.done(ok, f"at the hollow tree: {got}")


def s_hood(r):
    r.step("back to the merchant: the hood")
    r.path([[3590, 1600], [3550, 1775]] + road_points_between(3400, 2040), run=True, label="back to the road, west")
    seen = None
    x = 2040
    direction = -1
    passes = 0
    while not seen and passes < 3:
        x += direction * 40
        if x < 480 or x > 2040:
            direction = -direction
            passes += 1
            continue
        r.goto(road_at(x), run=True, tol=5, timeout=40, label=f"the road at x {x:.0f}")
        seen = cart_estimate(r)
        if seen and seen[2] > 80:
            seen = None
    if not seen:
        return r.done(False, "the cart was not found on its beat")
    stopped = stop_cart(r)
    if not stopped:
        return r.done(False, "the cart was not stopped the second time")
    if not open_shop(r):
        return r.done(False, "the shop did not open the second time")
    bought = buy(r, "hood")
    r.shot("shop-hood")
    leave = r.page.query_selector("#shop .leave")
    if leave:
        leave.click()
        r.wait(0.5)
    return r.done(bought, "bought the hood" if bought else "the hood was not bought")


def s_jacket(r):
    r.step("the jacket on the scarecrow")
    p = r.pos()
    r.path([[1400, road_at(1400)[1] + 20], [1400, 2700], [1400, 3200], [1400, 3290]], run=True, label="south to the scarecrow")
    got = collect_near(r, pickup_at("jacket"), "the jacket")
    r.shot("scarecrow")
    ok = any("acket" in g for g in got)
    return r.done(ok, f"at the scarecrow: {got}")


def s_wear(r):
    r.step("wear all five and slide")
    menu_open(r, "equipment")
    r.wait(0.5)
    r.shot("equipment-before")
    worn = 0
    for item in ("hood", "jacket", "gloves", "trousers", "boots"):
        slot = r.page.query_selector(f"#menu .face-equipment .slot[data-item='{item}']")
        if not slot:
            r.log("ISSUE", f"no {item} in Equipment")
            continue
        if "worn" not in (slot.get_attribute("class") or ""):
            slot.hover()
            r.wait(0.2)
            slot.click()
            r.wait(0.4)
        if "worn" in (r.page.query_selector(f"#menu .face-equipment .slot[data-item='{item}']").get_attribute("class") or ""):
            worn += 1
        else:
            r.log("ISSUE", f"the {item} did not show as worn after a click")
    r.shot("equipment-all-worn")
    r.text_check("Equipment, five worn")
    menu_close(r)
    bars = r.page.evaluate("() => [...document.querySelectorAll('#play-hud .footing .bar')].map((b) => b.textContent)")
    r.log("INFO", f"the HUD's footing bars: {bars}")
    # a run, let go, and how far the feet slide on
    r.steer(target=None, face=None, pitch=-0.12)
    r.hold("KeyW", "ShiftLeft")
    r.wait(3.0)
    a = r.pos()
    r.release()
    r.shot("slide")
    r.wait(4.0)
    b = r.pos()
    slide = math.hypot(b[0] - a[0], b[1] - a[1])
    r.steer(None)
    r.log("INFO", f"slid {slide:.1f} m after letting go of a run (CW-8: about 10 m with five pieces, 0.2 m bare)")
    return r.done(worn == 5 and slide > 4, f"{worn} of 5 worn, slid {slide:.1f} m")


def s_menus(r):
    r.step("every menu")
    ok = True
    for face in ("select", "map", "quest", "equipment"):
        menu_open(r, face)
        r.wait(0.8)
        u = r.ui().get("menu")
        front = u["faces"][u["face"]]["id"] if u else None
        if front != face:
            r.log("ISSUE", f"the ring did not turn to {face} (at {front})")
            ok = False
        r.shot(f"menu-{face}")
        r.text_check(f"the menu, {face}")
        # the keyboard's focus or the cursor is visible
        chosen = r.page.evaluate("() => document.querySelectorAll('#menu .face.front .slot.chosen').length")
        if face in ("select", "equipment") and chosen == 0:
            r.log("ISSUE", f"no chosen slot shows on {face}")
    menu_close(r)
    r.press("KeyM")
    r.wait(0.8)
    u = r.ui().get("menu")
    if not u or u["faces"][u["face"]]["id"] != "map":
        r.log("ISSUE", "M did not open the map")
        ok = False
    r.shot("menu-map-by-m")
    r.press("KeyM")
    r.wait(0.5)
    if r.screen() != "play":
        r.log("ISSUE", f"M did not close the map (screen {r.screen()})")
        menu_close(r)
    r.page.keyboard.press("Escape")
    r.wait(0.6)
    if r.screen() != "paused":
        r.log("ISSUE", f"Esc did not pause (screen {r.screen()})")
        ok = False
    r.shot("paused")
    r.text_check("the pause screen")
    settings = r.page.query_selector("#paused-more button")
    if settings:
        settings.click()
        r.wait(0.6)
        r.shot("settings")
        r.text_check("settings")
        focus = r.page.evaluate("() => document.activeElement ? document.activeElement.id || document.activeElement.className : ''")
        r.log("INFO", f"focused on settings: {focus!r}")
        r.page.keyboard.press("Escape")
        r.wait(0.5)
    else:
        r.log("ISSUE", "no settings button on the pause screen")
    if r.screen() == "settings":
        r.log("ISSUE", "Esc did not close settings")
        back = r.page.query_selector("#settings .back")
        if back:
            back.click()
            r.wait(0.4)
    if r.screen() == "paused":
        r.page.click("#paused button")
        r.wait(0.4)
    return r.done(ok and r.screen() == "play", f"every menu opened and closed; back to {r.screen()}")


def stop_cart(r):
    for attempt in range(8):
        c = cart_estimate(r)
        if not c:
            r.wait(0.5)
            continue
        ahead = road_at(c[0] - 9)
        r.goto(ahead, run=True, tol=2.0, timeout=60, label="ahead of the cart")
        c = cart_estimate(r) or c
        r.steer(face=c[:2], pitch=-0.02)
        r.wait(0.8)
        r.shot(f"throw-again-{attempt + 1}")
        r.press("KeyF", 4)
        r.wait(1.5)
        line = r.story_line()
        r.steer(None)
        if line and "stop" in line.lower():
            r.log("PASS", f"the cart stopped: {line!r} (throw {attempt + 1})")
            return True
        pr = r.prompt()
        if "ock" in pr:
            r.press("KeyE")
    return False


def open_shop(r):
    for _ in range(10):
        pr = r.prompt()
        if "erchant" in pr:
            break
        c = cart_estimate(r)
        if c:
            r.goto(c[:2], run=False, tol=2.0, timeout=8, label="the cart")
    r.press("KeyE")
    r.wait(1)
    if r.screen() == "dialogue":
        r.read_dialogue()
    r.wait(0.5)
    return r.screen() == "shop"


def buy(r, item):
    b = r.page.query_selector(f"#shop .shop-row[data-key='{item}'] .row-button")
    if not b or not b.is_enabled():
        coins = r.ui()["shop"]["coins"]["text"]
        r.log("ISSUE", f"the {item} cannot be bought ({coins} coins)")
        return False
    b.click()
    r.wait(0.4)
    r.log("INFO", f"the shop's note: {r.page.text_content('#shop .shop-note')!r}")
    return True


def main():
    args = sys.argv[1:]
    dist = ROOT / "dist"
    out = ROOT / "shots" / "m7qa" / "playthrough"
    only = None
    dev = None
    size = (1920, 1080)
    k = 0
    while k < len(args):
        a = args[k]
        if a == "--out":
            out = pathlib.Path(args[k + 1]); k += 2
        elif a == "--only":
            only = args[k + 1].split(","); k += 2
        elif a == "--dev-start":
            dev = args[k + 1]; k += 2
        elif a == "--size":
            w, h = args[k + 1].split("x"); size = (int(w), int(h)); k += 2
        else:
            dist = pathlib.Path(a); k += 1
    out.mkdir(parents=True, exist_ok=True)
    steps = [s_title, s_dream, s_boundary, s_cart, s_trainer, s_shrine, s_shrine_to_road, s_camp, s_drops, s_boomerang, s_trousers, s_hood, s_jacket, s_wear, s_menus]
    server, base = serve(dist)
    with sync_playwright() as pw:
        browser = pw.chromium.launch(args=GPU)
        page = browser.new_page(viewport={"width": size[0], "height": size[1]}, device_scale_factor=1)
        logs = []
        page.on("console", lambda m: logs.append(f"{m.type}: {m.text}") if m.type in ("error", "warning") else None)
        page.on("pageerror", lambda e: logs.append(f"pageerror: {e}"))
        page.route("**/app.js*", expose)
        start = [1792, 2560]
        url = base + "/index.html?pointer=free"
        if dev:
            url += f"&act=story.skip&go={dev}"
            # writing the script only: test acts to stage a later step
            for extra in os.environ.get("PLAYTHROUGH_DEV_ACTS", "").split():
                url += f"&act={extra}"
            gx, gz = (float(v) for v in dev.split(",")[:2])
            start = [math.floor(gx / 256) * 256, math.floor(gz / 256) * 256]
        page.add_init_script(f"window.__pt_start = {json.dumps(start)};")
        page.add_init_script("(" + HELPER.strip().rstrip(";") + ")")
        page.add_init_script("window.__pt.start = window.__pt_start;")
        page.goto(url, wait_until="commit")
        run = Run(page, out, dev)
        ok = True
        for s in steps:
            name = s.__name__[2:]
            if only and name not in only and not (name == "title"):
                continue
            try:
                s(run)
            except Exception as e:
                run.log("ISSUE", f"the step failed with {type(e).__name__}: {e}")
                traceback.print_exc()
                run.release()
                run.shot("step-failed")
        for line in logs:
            run.log("INFO", f"console {line}")
        run.step_name = "summary"
        run.log("INFO", f"{len(run.issues)} issues; run took {time.perf_counter() - run.t0:.0f} s")
        browser.close()
    server.shutdown()


if __name__ == "__main__":
    main()

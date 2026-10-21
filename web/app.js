// The page draws what the core returns and decides nothing.
//
// It loads the data files, starts the core through the shim, and each
// display frame passes the clock, the held actions and the mouse and wheel
// movement in, then draws the frame that comes out (gl.js). The screen's
// panels and the lines over the game are built from the core's view model
// (`ui`) without knowing what any panel means; every button sends its action
// back through `act`, the core's one door. Esc, a lost pointer and a hidden
// tab are reported to the core as they happen; the core decides what each
// means.
import init, { Game, numbers } from "./pkg/tool_shim.js";
import { makeRenderer } from "./gl.js";
import { makeInput } from "./input.js";
import { makeMenus } from "./menus/menus.js";
import { makeSound } from "./sound.js";

const el = (id) => document.getElementById(id);
const make = (tag, text, className) => {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
};
const body = document.body;

let hints = [];

// A label whose text is a glossary term shows the definition on hover.
function mark(node) {
  const hint = hints.find((h) => h.term === node.textContent);
  if (hint) {
    node.dataset.term = hint.term;
    node.title = hint.definition;
  }
}

function ready() {
  el("loading").hidden = true;
  body.dataset.ready = "true";
}

async function main() {
  const strings = await (await fetch("data/strings.json")).text();
  el("loading-text").textContent = JSON.parse(strings).loading;
  const [glossary, look, controls, display, vertex, fragment, keys, waterVertex, waterFragment] = await Promise.all([
    fetch("data/glossary.md").then((r) => r.text()),
    fetch("data/look.json").then((r) => r.text()),
    fetch("data/controls.json").then((r) => r.json()),
    fetch("data/display.json").then((r) => r.json()),
    fetch("data/shaders/scene.vert").then((r) => r.text()),
    fetch("data/shaders/scene.frag").then((r) => r.text()),
    fetch("data/keys.json").then((r) => r.json()),
    fetch("data/shaders/water.vert").then((r) => r.text()),
    fetch("data/shaders/water.frag").then((r) => r.text()),
  ]);
  await init();
  const N = JSON.parse(numbers());

  // chance for this session, drawn once and handed to the core (rng.rs)
  const seed = new Uint32Array(1);
  crypto.getRandomValues(seed);
  let game;
  try {
    game = new Game(strings, glossary, look, seed[0]);
  } catch (e) {
    el("message").textContent = String(e);
    ready();
    return;
  }

  const canvas = el("view");
  const meshes = JSON.parse(game.meshes());
  const renderer = makeRenderer(canvas, N, { vertex, fragment, waterVertex, waterFragment, software: display.software }, meshes, (id) => game.mesh(id));
  if (!renderer) {
    body.dataset.webgl = "none";
    el("message").textContent = JSON.parse(game.ui()).ok.no_webgl;
    ready();
    return;
  }
  body.dataset.webgl = "2";
  body.dataset.software = String(renderer.software);
  if (renderer.software) game.act("display.software", "true");
  renderer.setUniforms(game.uniforms(), game.clear_color(), game.water_uniforms());

  let ui = null;
  // M7: sound, set up on the reader's first press (web/sound.js)
  const sound = makeSound(game);
  window.addEventListener("pointerdown", () => sound.wake(), true);
  window.addEventListener("keydown", () => sound.wake(), true);
  const input = makeInput(canvas, controls, N.actions, () => {});
  // a reader's look speed from the address (?look=<percent>), as any setting,
  // and a place to start at (?go=<place> [yaw] [pitch], for views of the world)
  const params = new URLSearchParams(location.search);
  const lookSpeed = params.get("look");
  const goTo = params.get("go");

  // Sends one action through the core's door and draws what comes back.
  // `gesture`: the act came from a key or click, so if the core now wants
  // the pointer (a menu closed back to play) the page may take it again.
  function act(name, value, gesture) {
    const result = JSON.parse(game.act(name, value));
    if (result.error) {
      el("message").textContent = result.error;
      return;
    }
    el("message").textContent = "";
    show(result.ok);
    if (gesture && result.ok.capture && !input.engaged()) input.engage();
  }

  // M4: the HUD, the menu ring, dialogue, the shop and settings (web/menus/)
  const menus = makeMenus(el("menus"), { act, pixels: () => game.map_pixels(), keys });

  // M7: the Equipment screen's figure, the player's own model, drawn on its
  // own canvas from the core's figure frame when the core sends one
  const shaders = { vertex, fragment, waterVertex, waterFragment, software: display.software };
  const figureCanvas = menus.figureCanvas;
  let figure = null;
  let figureSize = "";
  function drawFigure() {
    const draws = game.figure_draws();
    if (draws.length === 0) return;
    if (!figure) {
      figure = makeRenderer(figureCanvas, N, shaders, meshes, (id) => game.mesh(id));
      if (!figure) return;
      figure.setUniforms(game.uniforms(), game.figure_clear_color(), game.water_uniforms());
    }
    const size = `${figureCanvas.clientWidth} ${figureCanvas.clientHeight}`;
    if (size !== figureSize) {
      figureSize = size;
      const [w, h] = game.figure_resize(figureCanvas.clientWidth, figureCanvas.clientHeight, window.devicePixelRatio || 1);
      figure.resize(w, h);
    }
    figure.frame(game.figure_view(), game.figure_instances(), game.figure_draws(), game.water_frame());
  }

  function panel(p) {
    const box = make("section", undefined, "panel");
    box.id = p.id;
    if (p.heading) box.append(make("h1", p.heading));
    if (p.text) box.append(make("p", p.text));
    (p.actions || []).forEach((a) => {
      const button = make("button", a.label);
      button.disabled = !a.enabled;
      button.dataset.action = a.name;
      button.dataset.value = a.value;
      button.addEventListener("click", () => {
        // the browser grants the pointer only inside a click
        if (a.engage) input.engage();
        button.blur();
        act(a.name, a.value);
      });
      box.append(button);
    });
    if (p.rows) {
      if (p.rows_heading) box.append(make("h2", p.rows_heading));
      const table = make("table", undefined, "controls");
      for (const r of p.rows) {
        const row = table.insertRow();
        row.insertCell().append(make("kbd", r.key));
        row.insertCell().append(make("span", r.does));
      }
      box.append(table);
    }
    return box;
  }

  // Builds the screen from the core's view model.
  function show(u) {
    const changed = !ui || ui.screen !== u.screen;
    ui = u;
    hints = u.hints;
    document.title = u.title;
    body.dataset.screen = u.screen;
    el("overlay").replaceChildren(...u.panels.map(panel));
    el("overlay").hidden = u.panels.length === 0;
    for (const item of u.hud) {
      let node = el(`hud-${item.id}`);
      if (!node) {
        node = make("p", item.text, "hud-item");
        node.id = `hud-${item.id}`;
        el("hud").append(node);
      }
      node.textContent = item.text;
      node.classList.toggle("shown", item.shown);
    }
    document.querySelectorAll("#overlay h1, #overlay h2, #overlay td span").forEach(mark);
    if (!u.capture) input.release();
    menus.show(u);
    // a screen of buttons opens with its first one focused, for the keyboard
    const first = el("overlay").querySelector("button");
    if (changed && first) first.focus({ preventScroll: true });
  }

  // Esc, a lost pointer and a hidden tab: the core decides what they mean.
  window.addEventListener("keydown", (e) => {
    if (e.code === "Escape") act("key.escape", "");
    else menus.key(e);
  });
  let locked = false;
  document.addEventListener("pointerlockchange", () => {
    const now = Boolean(document.pointerLockElement);
    // only a lock that was held and is now lost: a browser that grants the
    // lock and drops it at once plays on without it
    if (locked && !now) act("pointer.lost", "");
    locked = now;
  });
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) act("page.hidden", "");
  });
  // in play, a click on the game takes the pointer again if it was let go
  canvas.addEventListener("mousedown", () => {
    if (ui && ui.capture && !input.engaged()) input.engage();
  });

  if (lookSpeed) act("look.speed", lookSpeed);
  if (goTo) act("places.go", goTo);
  // a test duel and test settings from the address, each through the core's
  // door like any setting: ?duel=orc, ?foe=still, ?gap=115, ?speedster=5,
  // ?sticks=1
  const flags = new URLSearchParams(location.search);
  for (const [flag, name] of [["duel", "duel.spawn"], ["foe", "duel.pilot"], ["gap", "duel.gap"], ["speedster", "motion.speedster"], ["sticks", "duel.sticks"]]) {
    const value = flags.get(flag);
    if (value) act(name, value);
  }
  // acts from the address, as `?act=name` or `?act=name:value` (the test
  // inventory and HUD the screenshots and checks use: test.give_all, …)
  for (const asked of new URLSearchParams(location.search).getAll("act")) {
    const [name, value] = asked.split(":");
    act(name, value || "");
  }

  function resize() {
    const [w, h] = game.resize(canvas.clientWidth, canvas.clientHeight, window.devicePixelRatio || 1);
    renderer.resize(w, h);
  }
  window.addEventListener("resize", resize);
  resize();

  let staticVersion = 0;
  let uiVersion = 0;
  function frame(now) {
    const { bits, dx, dy, wheel } = input.read();
    game.advance(now, bits, dx, dy, wheel);
    // the meshes the core built this frame, each uploaded once
    const ups = game.uploads();
    for (let i = 0; i < ups.length; i += N.upload_values) renderer.setMesh(ups[i], game.mesh(ups[i]), ups[i + 1]);
    if (game.static_version() !== staticVersion) {
      staticVersion = game.static_version();
      renderer.setStatic(game.static_values());
    }
    renderer.frame(game.view(), game.instances(), game.draws(), game.water_frame());
    sound.frame();
    if (sound.started() && body.dataset.sound !== "on") body.dataset.sound = "on";
    drawFigure();
    if (game.ui_version() !== uiVersion) {
      uiVersion = game.ui_version();
      show(JSON.parse(game.ui()).ok);
    }
    if (body.dataset.ready !== "true") ready();
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
}

main();

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
import init, { Game, numbers, mesh } from "./pkg/tool_shim.js";
import { makeRenderer } from "./gl.js";
import { makeInput } from "./input.js";

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
  const [glossary, look, controls, display, vertex, fragment] = await Promise.all([
    fetch("data/glossary.md").then((r) => r.text()),
    fetch("data/look.json").then((r) => r.text()),
    fetch("data/controls.json").then((r) => r.json()),
    fetch("data/display.json").then((r) => r.json()),
    fetch("data/shaders/scene.vert").then((r) => r.text()),
    fetch("data/shaders/scene.frag").then((r) => r.text()),
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
  const renderer = makeRenderer(canvas, N, { vertex, fragment, software: display.software }, mesh);
  if (!renderer) {
    body.dataset.webgl = "none";
    el("message").textContent = JSON.parse(game.ui()).ok.no_webgl;
    ready();
    return;
  }
  body.dataset.webgl = "2";
  body.dataset.software = String(renderer.software);
  if (renderer.software) game.act("display.software", "true");
  renderer.setUniforms(game.uniforms(), game.clear_color());

  let ui = null;
  const input = makeInput(canvas, controls, N.actions, () => {});
  // a reader's look speed from the address (?look=<percent>), as any setting
  const lookSpeed = new URLSearchParams(location.search).get("look");

  // Sends one action through the core's door and draws what comes back.
  function act(name, value) {
    const result = JSON.parse(game.act(name, value));
    if (result.error) {
      el("message").textContent = result.error;
      return;
    }
    el("message").textContent = "";
    show(result.ok);
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
  }

  // Esc, a lost pointer and a hidden tab: the core decides what they mean.
  window.addEventListener("keydown", (e) => {
    if (e.code === "Escape") act("key.escape", "");
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
    if (game.static_version() !== staticVersion) {
      staticVersion = game.static_version();
      renderer.setStatic(game.static_values());
    }
    renderer.frame(game.view(), game.instances(), game.draws());
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

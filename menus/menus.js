// The menus: the HUD over the game, the ring of four screens (Select Item,
// Map, Quest, Equipment), dialogue, the shop and settings. Each is built
// from the core's view model (`ui()`, crates/core/src/ui.rs) and decides
// nothing: every key and click goes back through `act`, and the core
// answers with the next view model.
//
// Nodes are made once and then updated in place, so a CSS transition (the
// ring turning, a bar filling, a toast fading) runs across updates and a
// node under the pointer stays the same node. The ring's turn reaches CSS
// as a custom property; the angles are in menus.css.
//
// M7 travel: the flute's song screen and the traveller's list are in
// song.js; the song's notes also show in the dialogue that teaches it and on
// the Quest screen.

import { makeSong, noteRow } from "./song.js?v=87cb5a6d3e2e";

const make = (tag, className, text) => {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
};

const text = (node, value) => {
  const s = value === undefined || value === null ? "" : String(value);
  if (node.textContent !== s) node.textContent = s;
};

const prop = (node, name, value) => {
  const s = String(value);
  if (node.style.getPropertyValue(name) !== s) node.style.setProperty(name, s);
};

const flag = (node, name, on) => node.classList.toggle(name, Boolean(on));

function icon(img, src, alt) {
  if (src) {
    if (img.getAttribute("src") !== src) img.setAttribute("src", src);
    img.alt = alt || "";
    img.hidden = false;
  } else {
    img.removeAttribute("src");
    img.alt = "";
    img.hidden = true;
  }
}

export function makeMenus(root, { act, pixels, keys }) {
  let hints = [];
  let current = null;
  const figureCanvas = make("canvas", "figure");
  figureCanvas.setAttribute("aria-hidden", "true");

  // A label whose text is a glossary term shows the definition on hover.
  function term(node) {
    const hint = hints.find((h) => h.term === node.textContent);
    if (hint) {
      node.dataset.term = hint.term;
      node.title = hint.definition;
    } else {
      delete node.dataset.term;
      node.removeAttribute("title");
    }
  }

  function button(className, onClick) {
    const b = make("button", className);
    b.type = "button";
    b.addEventListener("click", onClick);
    return b;
  }

  // ---- shared pieces ----

  function bar(id) {
    const box = make("div", `bar bar-${id}`);
    box.label = make("span", "bar-label");
    box.value = make("span", "bar-text");
    const track = make("span", "bar-track");
    box.fill = make("span", "bar-fill");
    track.append(box.fill);
    const top = make("span", "bar-top");
    top.append(box.label, box.value);
    box.append(top, track);
    return box;
  }

  function setBar(box, b) {
    box.hidden = !b;
    if (!b) return;
    text(box.label, b.label);
    text(box.value, b.text);
    prop(box, "--pct", b.pct);
    term(box.label);
  }

  function quick() {
    const q = make("div", "quick");
    q.key = make("kbd", "quick-key");
    q.well = make("span", "well");
    q.img = make("img");
    q.count = make("span", "count");
    q.well.append(q.img);
    q.append(q.well, q.count, q.key);
    return q;
  }

  function setQuick(q, v) {
    text(q.key, v.key);
    icon(q.img, v.item && v.item.icon, v.item && v.item.name);
    text(q.count, v.item ? v.item.count || "" : "");
    flag(q, "ready", v.ready);
    flag(q, "empty", !v.item);
    q.dataset.key = v.key;
    q.dataset.item = v.item ? v.item.id : "";
    q.title = v.item ? v.item.name : "";
  }

  function count() {
    const c = make("div", "count-box");
    c.img = make("img");
    c.value = make("span", "count-value");
    c.label = make("span", "count-label");
    c.append(c.img, c.value, c.label);
    return c;
  }

  function setCount(c, v) {
    icon(c.img, v.icon, "");
    text(c.value, v.text);
    text(c.label, v.label);
    term(c.label);
  }

  function info() {
    const box = make("footer", "info");
    box.name = make("h3", "info-name");
    box.about = make("p", "info-about");
    box.notes = make("p", "info-notes");
    box.append(box.name, box.about, box.notes);
    return box;
  }

  function setInfo(box, v) {
    flag(box, "blank", !v);
    text(box.name, v ? v.name : "");
    text(box.about, v ? v.about : "");
    text(box.notes, v ? v.notes.join("  ·  ") : "");
    term(box.name);
  }

  // A round slot on a ring screen. Pointing at it moves the core's cursor;
  // a click chooses it.
  function slot(face) {
    const s = button("slot", () => {
      if (face.classList.contains("front")) act("menu.choose", s.dataset.index);
    });
    s.addEventListener("mouseenter", () => {
      if (face.classList.contains("front") && !s.classList.contains("chosen")) act("menu.point", s.dataset.index);
    });
    s.well = make("span", "well");
    s.img = make("img");
    s.empty = make("span", "slot-label");
    s.well.append(s.img, s.empty);
    s.count = make("span", "count");
    s.badge = make("kbd", "badge");
    s.append(s.well, s.count, s.badge);
    return s;
  }

  function setSlot(s, v) {
    s.dataset.index = v.index;
    s.dataset.place = v.place;
    s.dataset.item = v.item ? v.item.id : "";
    icon(s.img, v.item && v.item.icon, v.item && v.item.name);
    text(s.empty, v.item ? "" : v.label || "");
    text(s.count, v.item ? v.item.count || "" : "");
    text(s.badge, v.quick || "");
    flag(s, "chosen", v.chosen);
    flag(s, "worn", v.worn);
    flag(s, "filled", v.item);
    flag(s, "on-key", v.quick);
    s.setAttribute("aria-label", v.item ? v.item.name : v.label || "");
    s.setAttribute("aria-pressed", String(Boolean(v.chosen)));
  }

  // Keeps one node per key in `parent`, in the list's order.
  function keyed(parent, list, keyOf, create, update) {
    const old = new Map();
    for (const child of parent.children) old.set(child.dataset.key, child);
    const nodes = list.map((v) => {
      const k = keyOf(v);
      const node = old.get(k) || create(v);
      node.dataset.key = k;
      update(node, v);
      return node;
    });
    const same = nodes.length === parent.children.length && nodes.every((n, i) => parent.children[i] === n);
    if (!same) parent.replaceChildren(...nodes);
  }

  // ---- the HUD ----

  const hud = make("div");
  hud.id = "play-hud";
  const health = bar("health");
  const foe = bar("foe");
  const imbue = make("div", "imbue-ring");
  imbue.dial = make("span", "dial");
  imbue.img = make("img");
  imbue.dial.append(imbue.img);
  imbue.label = make("span", "ring-label");
  imbue.value = make("span", "ring-text");
  const ringWords = make("span", "ring-words");
  ringWords.append(imbue.label, imbue.value);
  imbue.append(imbue.dial, ringWords);
  const footing = make("div", "footing");
  footing.bars = [bar("speed"), bar("footing")];
  footing.append(...footing.bars);
  const vitals = make("div", "hud-vitals");
  // M7 (T14): the sprint's build, while sprinting
  const sprint = bar("sprint");
  // T31: hung on one of the Ant King's plates, its hold
  const plate = bar("plate");
  const vitalBars = make("div", "hud-bars");
  vitalBars.append(health, footing, sprint, plate);
  vitals.append(imbue, vitalBars);
  const foeBox = make("div", "hud-foe");
  foeBox.append(foe);
  const quicks = make("div", "hud-quick");
  const hudQuick = [quick(), quick(), quick()];
  quicks.append(...hudQuick);
  const coins = count();
  coins.classList.add("hud-coins");
  const toast = make("div", "hud-toast");
  toast.img = make("img");
  toast.value = make("span");
  toast.append(toast.img, toast.value);
  hud.append(vitals, foeBox, quicks, coins, toast);

  function showHud(u) {
    const h = u.play;
    flag(hud, "shown", u.hud_shown);
    setBar(health, h.health);
    setBar(foe, h.foe);
    flag(foeBox, "shown", h.foe);
    imbue.hidden = !h.imbue;
    if (h.imbue) {
      icon(imbue.img, h.imbue.icon, "");
      text(imbue.label, h.imbue.label);
      text(imbue.value, h.imbue.text);
      prop(imbue, "--pct", h.imbue.pct);
      imbue.dataset.state = h.imbue.state;
      term(imbue.label);
    }
    footing.hidden = h.footing.length === 0;
    footing.bars.forEach((b, i) => setBar(b, h.footing[i]));
    setBar(sprint, h.sprint);
    setBar(plate, h.plate);
    h.quick.forEach((q, i) => setQuick(hudQuick[i], q));
    setCount(coins, h.coins);
    flag(toast, "shown", h.toast.shown);
    if (h.toast.shown) {
      icon(toast.img, h.toast.icon, "");
      text(toast.value, h.toast.text);
    }
  }

  // ---- the ring ----

  const menu = make("div");
  menu.id = "menu";
  menu.setAttribute("role", "dialog");
  const backdrop = make("div", "menu-backdrop");
  const stage = make("div", "ring-stage");
  const ring = make("div", "ring");
  stage.append(ring);
  const turnLeft = button("turn turn-left", () => act(turnLeft.action.name, turnLeft.action.value));
  turnLeft.key = make("kbd");
  turnLeft.label = make("span");
  turnLeft.append(turnLeft.key, turnLeft.label);
  const turnRight = button("turn turn-right", () => act(turnRight.action.name, turnRight.action.value));
  turnRight.key = make("kbd");
  turnRight.label = make("span");
  turnRight.append(turnRight.label, turnRight.key);
  const foot = make("div", "menu-foot");
  const assign = make("div", "menu-assign");
  assign.label = make("span", "assign-label");
  const menuQuick = [quick(), quick(), quick()];
  const quickRow = make("div", "quick-row");
  quickRow.append(...menuQuick);
  assign.append(assign.label, quickRow);
  const close = button("menu-close", () => act(close.action.name, close.action.value));
  close.key = make("kbd");
  close.label = make("span");
  close.append(close.key, close.label);
  // M7: a quick key pointed at or focused is described over the keys
  const quickInfo = info();
  quickInfo.classList.add("quick-info");
  menuQuick.forEach((q, i) => {
    q.tabIndex = 0;
    q.setAttribute("role", "button");
    const on = () => act("menu.hover_quick", String(i));
    const off = () => act("menu.hover_quick", "");
    q.addEventListener("mouseenter", on);
    q.addEventListener("focus", on);
    q.addEventListener("mouseleave", off);
    q.addEventListener("blur", off);
  });
  foot.append(assign, close, quickInfo);
  menu.append(backdrop, stage, turnLeft, turnRight, foot);
  const faces = new Map();

  // M7: the names on the turn buttons change when the ring has finished
  // turning, not as it starts (they fade out while it turns), so they name
  // the screens beside the one in front.
  let shownTurn = null;
  let pendingNames = null;
  function setNames(m) {
    text(turnLeft.key, m.left.key);
    text(turnLeft.label, m.left.text);
    text(turnRight.key, m.right.key);
    text(turnRight.label, m.right.text);
  }
  function settle() {
    if (pendingNames) setNames(pendingNames);
    pendingNames = null;
    flag(menu, "turning", false);
  }
  ring.addEventListener("transitionend", (e) => {
    if (e.target === ring && e.propertyName === "transform") settle();
  });

  function faceShell(f) {
    const face = make("section", `face face-${f.id}`);
    face.dataset.face = f.id;
    prop(face, "--i", f.index);
    face.heading = make("h2", "face-heading");
    const head = make("header", "face-head");
    head.append(face.heading);
    face.body = make("div", "face-body");
    face.append(head, face.body);
    return face;
  }

  const builders = {
    select(face, f) {
      face.grid = make("div", "grid");
      face.slots = f.slots.map(() => slot(face));
      face.grid.append(...face.slots);
      face.info = info();
      face.body.append(face.grid);
      face.append(face.info);
    },
    equipment(face, f) {
      const wrap = make("div", "equip");
      const hand = make("div", "equip-hand");
      face.handHeading = make("h3");
      const handSlots = make("div", "hand-slots");
      const figure = make("div", "equip-figure");
      // M7: the player's own model, drawn by app.js on this canvas from the
      // core's figure frame (crates/core/src/stage.rs)
      face.figure = figureCanvas;
      figure.append(face.figure);
      face.slots = f.slots.map((s) => {
        const node = slot(face);
        (s.place === "hand" ? handSlots : figure).append(node);
        return node;
      });
      hand.append(face.handHeading, handSlots);
      const set = make("div", "equip-set");
      face.setHead = make("h3");
      face.setIcon = make("img", "set-icon");
      const setTitle = make("div", "set-title");
      setTitle.append(face.setIcon, face.setHead);
      face.setCount = make("p", "set-count");
      face.setBars = [bar("speed"), bar("footing")];
      set.append(setTitle, face.setCount, ...face.setBars);
      wrap.append(hand, figure, set);
      face.info = info();
      face.body.append(wrap);
      face.append(face.info);
    },
    map(face) {
      const wrap = make("div", "map-wrap");
      const paper = make("div", "parchment");
      face.canvas = make("canvas");
      face.marks = make("div", "marks");
      face.you = make("div", "you");
      face.you.arrow = make("span", "you-arrow");
      face.you.label = make("span", "you-label");
      face.you.append(face.you.arrow, face.you.label);
      // T24: the map on a sheet the core's zoom and window place in the
      // parchment's frame; the wheel over it goes to the core as it turns
      face.sheet = make("div", "sheet");
      face.sheet.append(face.canvas, face.marks, face.you);
      paper.append(face.sheet);
      // T34: with the pointer and the frame's box, so the core zooms in at
      // the pointer; a drag over it goes to the core as it moves
      paper.addEventListener(
        "wheel",
        (e) => {
          e.preventDefault();
          const box = paper.getBoundingClientRect();
          act("menu.wheel", `${e.deltaY} ${e.clientX} ${e.clientY} ${box.left} ${box.top} ${box.width} ${box.height}`, true);
        },
        { passive: false },
      );
      paper.addEventListener("pointerdown", (e) => {
        if (e.button !== 0) return;
        e.preventDefault();
        paper.setPointerCapture(e.pointerId);
        paper.classList.add("dragging");
      });
      paper.addEventListener("pointermove", (e) => {
        if (!paper.hasPointerCapture(e.pointerId)) return;
        act("menu.drag", `${e.movementX} ${e.movementY} ${paper.clientWidth} ${paper.clientHeight}`, true);
      });
      const dropped = () => paper.classList.remove("dragging");
      paper.addEventListener("pointerup", dropped);
      paper.addEventListener("pointercancel", dropped);
      paper.addEventListener("lostpointercapture", dropped);
      const side = make("aside", "map-side");
      face.legend = make("p", "map-legend");
      face.walked = make("p", "map-walked");
      face.zoom = make("p", "map-zoom");
      face.list = make("ul", "map-list");
      face.youRow = make("p", "map-list");
      const youRow = make("li");
      youRow.append(make("span", "you-arrow"), face.youRow);
      const youList = make("ul", "map-list");
      youList.append(youRow);
      side.append(face.legend, face.walked, face.zoom, youList, face.list);
      wrap.append(paper, side);
      face.body.append(wrap);
      face.version = null;
    },
    quest(face) {
      const wrap = make("div", "quest");
      const ability = make("div", "quest-card quest-imbue");
      face.imbueWell = make("span", "well");
      face.imbueImg = make("img");
      face.imbueWell.append(face.imbueImg);
      const words = make("div");
      face.imbueName = make("h3");
      face.imbueAbout = make("p");
      words.append(face.imbueName, face.imbueAbout);
      ability.append(face.imbueWell, words);
      face.coins = count();
      face.coins.classList.add("quest-card", "quest-coins");
      const task = make("div", "quest-card quest-task");
      face.taskHeading = make("h3");
      face.task = make("p");
      task.append(face.taskHeading, face.task);
      // M7 travel: the songs learned, with their notes
      face.songs = make("div", "quest-card quest-songs");
      face.songsHeading = make("h3");
      face.songName = make("p", "song-name");
      face.songRow = make("div", "note-row");
      face.songAbout = make("p", "song-about");
      face.songs.append(face.songsHeading, face.songName, face.songRow, face.songAbout);
      // M7 rumours (T17): the quest lines, each with its latest rumour
      face.lines = make("div", "quest-card quest-lines");
      face.linesHeading = make("h3");
      face.linesNone = make("p", "lines-none");
      face.lineList = make("ul", "line-list");
      face.lines.append(face.linesHeading, face.linesNone, face.lineList);
      wrap.append(ability, face.coins, task, face.songs, face.lines);
      face.body.append(wrap);
    },
  };

  const updaters = {
    select(face, f) {
      f.slots.forEach((s, i) => setSlot(face.slots[i], s));
      setInfo(face.info, f.info);
    },
    equipment(face, f) {
      text(face.handHeading, f.hand_heading);
      face.figure.hidden = !f.stage;
      f.slots.forEach((s, i) => setSlot(face.slots[i], s));
      text(face.setHead, f.set.heading);
      icon(face.setIcon, f.set.icon, "");
      text(face.setCount, f.set.count);
      face.setBars.forEach((b, i) => setBar(b, f.set.bars[i]));
      setInfo(face.info, f.info);
      term(face.handHeading);
      term(face.setHead);
    },
    map(face, f) {
      const m = f.map;
      if (face.version !== m.version) {
        face.version = m.version;
        face.canvas.width = m.size;
        face.canvas.height = m.size;
        const data = new Uint8ClampedArray(pixels());
        face.canvas.getContext("2d").putImageData(new ImageData(data, m.size), 0, 0);
      }
      keyed(
        face.marks,
        m.marks,
        (k) => `${k.kind} ${k.left} ${k.top}`,
        () => {
          const node = make("span", "mark");
          node.img = make("img");
          node.label = make("span", "mark-label");
          node.append(node.img, node.label);
          return node;
        },
        (node, k) => {
          icon(node.img, k.icon, "");
          text(node.label, k.label);
          node.classList.toggle("quiet", k.quiet);
          prop(node, "left", k.left);
          prop(node, "top", k.top);
        },
      );
      prop(face.you, "left", m.you.left);
      prop(face.you, "top", m.you.top);
      prop(face.you, "--heading", `${m.heading}deg`);
      text(face.you.label, m.you.label);
      text(face.youRow, m.you.label);
      text(face.legend, m.legend);
      text(face.walked, m.walked);
      text(face.zoom, m.zoom_line);
      prop(face.sheet, "--zoom", String(m.zoom));
      prop(face.sheet, "--x", m.left);
      prop(face.sheet, "--y", m.top);
      keyed(
        face.list,
        m.marks,
        (k) => k.kind,
        () => {
          const li = make("li");
          li.img = make("img");
          li.label = make("span");
          li.append(li.img, li.label);
          return li;
        },
        (li, k) => {
          icon(li.img, k.icon, "");
          text(li.label, k.label);
        },
      );
    },
    quest(face, f) {
      const q = f.quest;
      icon(face.imbueImg, q.imbue_icon, "");
      flag(face.imbueWell, "dim", !q.knows_imbue);
      text(face.imbueName, q.imbue.name);
      text(face.imbueAbout, q.imbue.about);
      setCount(face.coins, q.coins);
      text(face.taskHeading, q.task_heading);
      text(face.task, q.task);
      term(face.imbueName);
      face.songs.hidden = !q.song;
      if (q.song) {
        text(face.songsHeading, q.songs_heading);
        text(face.songName, q.song.name);
        noteRow(face.songRow, q.song.notes);
        text(face.songAbout, q.song.about);
        term(face.songName);
      }
      text(face.linesHeading, q.lines_heading);
      text(face.linesNone, q.lines_none);
      face.linesNone.hidden = q.lines.length > 0;
      keyed(
        face.lineList,
        q.lines,
        (l) => l.id,
        () => {
          const li = make("li");
          li.head = make("div", "line-head");
          li.lineName = make("span", "line-title");
          li.progress = make("span", "line-progress");
          li.head.append(li.lineName, li.progress);
          li.lineText = make("p", "line-text");
          li.append(li.head, li.lineText);
          return li;
        },
        (li, l) => {
          text(li.lineName, l.title);
          text(li.progress, l.done ? l.done_label : l.progress);
          text(li.lineText, l.text);
          flag(li, "done", l.done);
        },
      );
    },
  };

  function showMenu(m) {
    const opening = !menu.classList.contains("shown");
    if (opening) ring.classList.add("still");
    for (const f of m.faces) {
      let face = faces.get(f.id);
      if (!face) {
        face = faceShell(f);
        builders[f.id](face, f);
        faces.set(f.id, face);
        ring.append(face);
      }
      text(face.heading, f.heading);
      flag(face, "front", f.front);
      face.setAttribute("aria-hidden", String(!f.front));
      updaters[f.id](face, f);
    }
    prop(ring, "--turn", m.turn);
    menu.dataset.face = m.faces[m.face].id;
    turnLeft.action = m.left.action;
    turnRight.action = m.right.action;
    close.action = m.close.action;
    const turning = !opening && shownTurn !== null && shownTurn !== m.turn;
    shownTurn = m.turn;
    if (turning && getComputedStyle(ring).transitionDuration !== "0s") {
      pendingNames = m;
      flag(menu, "turning", true);
    } else if (!menu.classList.contains("turning")) {
      setNames(m);
    } else {
      pendingNames = m;
    }
    text(close.key, m.close.key);
    text(close.label, m.close.text);
    text(assign.label, m.assign);
    m.quick.forEach((q, i) => setQuick(menuQuick[i], q));
    quickInfo.hidden = !m.quick_info;
    if (m.quick_info) setInfo(quickInfo, m.quick_info);
    menuQuick.forEach((q) => q.setAttribute("aria-label", q.title || q.dataset.key));
    flag(menu, "shown", true);
    if (opening) {
      settle();
      setNames(m);
      // the ring appears already turned; only later turns are eased
      ring.getBoundingClientRect();
      requestAnimationFrame(() => ring.classList.remove("still"));
    }
  }

  // ---- dialogue ----

  const talk = make("div");
  talk.id = "talk";
  const talkBox = make("div", "talk-box");
  talkBox.addEventListener("click", () => {
    if (talk.next) act(talk.next.name, talk.next.value, talk.next.engage);
  });
  const portrait = make("span", "portrait");
  const portraitImg = make("img");
  portrait.append(portraitImg);
  const talkName = make("div", "talk-name");
  const talkText = make("p", "talk-text");
  const said = make("span", "said");
  const unsaid = make("span", "unsaid");
  unsaid.setAttribute("aria-hidden", "true");
  talkText.append(said, unsaid);
  const talkNext = make("div", "talk-next");
  const talkKey = make("kbd");
  const talkLabel = make("span");
  const caret = make("span", "caret");
  talkNext.append(talkKey, talkLabel, caret);
  // M7 travel: the song the line teaches, its notes under the words
  const talkSong = make("div", "note-row talk-song");
  const talkMain = make("div", "talk-main");
  talkMain.append(talkName, talkText, talkSong, talkNext);
  talkBox.append(portrait, talkMain);
  talk.append(talkBox);

  function showTalk(d) {
    flag(talk, "shown", d);
    if (!d) return;
    talk.next = d.next;
    talk.dataset.who = d.who;
    icon(portraitImg, d.portrait, "");
    text(talkName, d.name);
    text(said, d.said);
    text(unsaid, d.unsaid);
    flag(talkNext, "done", d.done);
    text(talkKey, d.key);
    text(talkLabel, d.next.label);
    talkSong.hidden = !d.song;
    if (d.song) noteRow(talkSong, d.song.notes);
  }

  // ---- the shop ----

  const shop = make("div");
  shop.id = "shop";
  shop.setAttribute("role", "dialog");
  const shopCard = make("div", "card shop-card");
  const shopHead = make("header", "shop-head");
  const shopTitle = make("h1");
  const shopCoins = count();
  shopHead.append(shopTitle, shopCoins);
  const cols = make("div", "shop-cols");
  const buyCol = make("section", "shop-col");
  const buyHead = make("h2");
  const buyList = make("ul", "shop-list");
  buyCol.append(buyHead, buyList);
  const sellCol = make("section", "shop-col");
  const sellHead = make("h2");
  const sellList = make("ul", "shop-list");
  const sellEmpty = make("p", "shop-empty");
  sellCol.append(sellHead, sellList, sellEmpty);
  cols.append(buyCol, sellCol);
  const shopNote = make("p", "shop-note");
  shopNote.setAttribute("role", "status");
  const leave = button("leave", () => act(leave.action.name, leave.action.value, leave.action.engage));
  shopCard.append(shopHead, cols, shopNote, leave);
  shop.append(shopCard);

  function row() {
    const li = make("li", "shop-row");
    li.well = make("span", "well");
    li.img = make("img");
    li.well.append(li.img);
    li.name = make("span", "row-name");
    li.count = make("span", "count");
    li.price = make("span", "row-price");
    li.button = button("row-button", () => act(li.action.name, li.action.value));
    li.append(li.well, li.name, li.count, li.price, li.button);
    return li;
  }

  function setRow(li, r) {
    li.action = r.action;
    icon(li.img, r.item.icon, "");
    text(li.name, r.item.name);
    text(li.count, r.item.count || "");
    text(li.price, r.price);
    text(li.button, r.action.label);
    li.button.disabled = !r.action.enabled;
    flag(li, "kept", r.kept);
    li.button.setAttribute("aria-label", `${r.action.label}: ${r.item.name}, ${r.price}`);
    term(li.name);
  }

  function showShop(s) {
    flag(shop, "shown", s);
    if (!s) return;
    text(shopTitle, s.heading);
    setCount(shopCoins, s.coins);
    text(buyHead, s.buy_heading);
    text(sellHead, s.sell_heading);
    keyed(buyList, s.buy, (r) => r.item.id, row, setRow);
    keyed(sellList, s.sell, (r) => r.item.id, row, setRow);
    text(sellEmpty, s.empty || "");
    text(shopNote, s.note || "");
    leave.action = s.leave;
    text(leave, s.leave.label);
    term(shopTitle);
  }

  // ---- settings ----

  const settings = make("div");
  settings.id = "settings";
  settings.setAttribute("role", "dialog");
  const setCard = make("div", "card settings-card");
  const setTitle = make("h1");
  const controls = make("div", "controls-list");
  const setNote = make("p", "settings-note");
  const back = button("back", () => act(back.action.name, back.action.value));
  setCard.append(setTitle, controls, setNote, back);
  settings.append(setCard);

  function control(c) {
    const box = make("div", `control control-${c.kind}`);
    box.label = make("label", "control-label");
    box.value = make("span", "control-value");
    const id = `setting-${c.id}`;
    box.label.htmlFor = id;
    if (c.kind === "range") {
      box.input = make("input");
      box.input.type = "range";
      box.input.addEventListener("input", () => act(box.control.name, box.input.value));
    } else {
      box.input = button("switch", () => act(box.control.name, box.control.next));
      box.input.setAttribute("role", "switch");
      box.input.append(make("span", "knob"));
    }
    box.input.id = id;
    box.append(box.label, box.input, box.value);
    return box;
  }

  function setControl(box, c) {
    box.control = c;
    text(box.label, c.label);
    text(box.value, c.text);
    if (c.kind === "range") {
      box.input.min = c.min;
      box.input.max = c.max;
      box.input.step = c.step;
      if (box.input.value !== String(c.value)) box.input.value = c.value;
      box.input.setAttribute("aria-valuetext", c.text);
      prop(box.input, "--min", c.min);
      prop(box.input, "--max", c.max);
      prop(box.input, "--value", c.value);
    } else {
      box.input.setAttribute("aria-checked", String(c.on));
      flag(box.input, "on", c.on);
    }
    term(box.label);
  }

  function showSettings(s) {
    const opening = !settings.classList.contains("shown");
    flag(settings, "shown", s);
    if (!s) return;
    text(setTitle, s.heading);
    keyed(controls, s.controls, (c) => c.id, control, setControl);
    text(setNote, s.note);
    back.action = s.back;
    text(back, s.back.label);
    if (opening) controls.querySelector("input, button").focus();
  }

  root.append(hud, menu, talk, shop, settings);
  const song = makeSong(root, { act, pixels });

  return {
    figureCanvas,
    show(u) {
      hints = u.hints;
      current = u;
      showHud(u);
      if (u.menu) showMenu(u.menu);
      else flag(menu, "shown", false);
      showTalk(u.dialogue);
      showShop(u.shop);
      showSettings(u.settings);
      song.show(u.song);
      // M7 travel: the last sound travel asked for, for the sound table
      if (u.travel_sound) {
        root.dataset.travelSound = u.travel_sound.id;
        root.dataset.travelSoundSeq = u.travel_sound.seq;
      }
    },
    // A key on a screen whose keys the core reads (ui().keymap): sent as
    // `<keymap>.key` with the word data/keys.json gives it. Whether it was.
    key(e) {
      const map = current && keys[current.keymap];
      const word = map && map[e.code];
      if (!word) return false;
      e.preventDefault();
      if (e.repeat && !keys.repeat.includes(word)) return true;
      act(`${current.keymap}.key`, word, true);
      return true;
    },
  };
}

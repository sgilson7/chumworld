// M7 travel (T11): the flute's song screen and the traveller's list, built
// from the core's view model (`ui().song`, crates/core/src/travel.rs). It
// decides nothing: notes, judging, the list and prices are the core's; each
// key reaches the core through the `travel` keymap (data/keys.json), each
// click through `act`. Every place and size comes from the core as a CSS
// percentage, so this file holds no number.

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
  if (img.getAttribute("src") !== src) img.setAttribute("src", src);
  img.alt = alt || "";
}

// A row of note icons (the song as the dialogue, the quest screen and the
// song screen show it), one node per note, made again only when it changes.
export function noteRow(row, notes) {
  const key = notes.map((n) => n.id).join(" ");
  if (row.dataset.notes === key) return;
  row.dataset.notes = key;
  row.replaceChildren(
    ...notes.map((n) => {
      const img = make("img", "note-icon");
      icon(img, n.icon, n.label);
      img.title = n.label;
      return img;
    }),
  );
}

export function makeSong(root, { act, pixels }) {
  const box = make("div");
  box.id = "song";
  box.setAttribute("role", "dialog");
  const card = make("div", "card song-card");
  const heading = make("h1", "song-heading");

  // the staff: five lines, the notes played on them
  const notesPane = make("div", "song-notes");
  const staff = make("div", "staff");
  const lines = [];
  const played = make("div", "staff-notes");
  staff.append(played);
  const said = make("p", "song-said");
  said.setAttribute("role", "status");
  const keys = make("div", "song-keys");
  // the songs known, one row each (T35, G2: the song of strafing too)
  const known = make("div", "song-known-list");
  function knownRow() {
    const r = make("div", "song-known");
    r.name = make("span", "song-known-name");
    r.row = make("span", "note-row");
    r.append(r.name, r.row);
    return r;
  }
  notesPane.append(staff, said, keys, known);

  // the list: the map with the waystones on it, and the rows beside it
  const pickPane = make("div", "song-pick");
  const pickHead = make("h2", "pick-heading");
  const pickBody = make("div", "pick-body");
  const paper = make("div", "parchment pick-map");
  const canvas = make("canvas");
  const marks = make("div", "marks");
  const you = make("div", "you");
  you.arrow = make("span", "you-arrow");
  you.label = make("span", "you-label");
  you.append(you.arrow, you.label);
  paper.append(canvas, marks, you);
  const side = make("div", "pick-side");
  const coins = make("p", "pick-coins");
  const list = make("ul", "pick-list");
  const empty = make("p", "pick-empty");
  const note = make("p", "pick-note");
  note.setAttribute("role", "status");
  side.append(coins, list, empty, note);
  pickBody.append(paper, side);
  pickPane.append(pickHead, pickBody);
  let mapVersion = null;

  const hint = make("p", "song-hint");
  const leave = make("button", "leave");
  leave.type = "button";
  leave.addEventListener("click", () => act(leave.action.name, leave.action.value, leave.action.engage));
  card.append(heading, notesPane, pickPane, hint, leave);
  box.append(card);
  root.append(box);

  function showStaff(s) {
    while (lines.length < s.lines.length) {
      const line = make("span", "staff-line");
      lines.push(line);
      staff.insertBefore(line, played);
    }
    s.lines.forEach((top, i) => prop(lines[i], "top", top));
    const key = s.played.map((n) => n.id).join(" ");
    if (played.dataset.notes !== key) {
      played.dataset.notes = key;
      played.replaceChildren(
        ...s.played.map((n) => {
          const img = make("img", "staff-note");
          icon(img, n.icon, n.label);
          prop(img, "left", n.left);
          prop(img, "top", n.top);
          return img;
        }),
      );
    }
    staff.dataset.state = s.state;
    text(said, s.said || "");
    if (keys.childElementCount !== s.keys.length) {
      keys.replaceChildren(
        ...s.keys.map(() => {
          const k = make("span", "song-key");
          k.kbd = make("kbd");
          k.img = make("img", "note-icon");
          k.append(k.kbd, k.img);
          return k;
        }),
      );
    }
    s.keys.forEach((k, i) => {
      const node = keys.children[i];
      text(node.kbd, k.key);
      icon(node.img, k.note.icon, k.note.label);
      node.title = k.note.label;
    });
    known.hidden = s.songs.length === 0;
    while (known.children.length < s.songs.length) known.append(knownRow());
    while (known.children.length > s.songs.length) known.lastChild.remove();
    s.songs.forEach((c, i) => {
      const r = known.children[i];
      text(r.name, c.name);
      noteRow(r.row, c.notes);
      r.title = c.about;
    });
  }

  function row() {
    const li = make("li", "pick-row");
    li.button = make("button", "pick-button");
    li.button.type = "button";
    li.button.addEventListener("click", () => act(li.action.name, li.action.value, li.action.engage));
    li.label = make("span", "pick-label");
    li.price = make("span", "pick-price");
    li.button.append(li.label, li.price);
    li.append(li.button);
    return li;
  }

  function showPick(p) {
    text(pickHead, p.heading);
    if (mapVersion !== p.version) {
      mapVersion = p.version;
      canvas.width = p.size;
      canvas.height = p.size;
      const data = new Uint8ClampedArray(pixels());
      canvas.getContext("2d").putImageData(new ImageData(data, p.size), 0, 0);
    }
    prop(you, "left", p.you.left);
    prop(you, "top", p.you.top);
    text(you.label, p.you.label);
    const ids = p.rows.map((r) => r.id).join(" ");
    if (marks.dataset.ids !== ids) {
      marks.dataset.ids = ids;
      marks.replaceChildren(
        ...p.rows.map((r) => {
          const m = make("button", "mark pick-mark");
          m.type = "button";
          m.dot = make("span", "pick-dot");
          m.label = make("span", "mark-label");
          m.append(m.dot, m.label);
          m.addEventListener("click", () => act(m.action.name, m.action.value, m.action.engage));
          return m;
        }),
      );
      list.replaceChildren(...p.rows.map(row));
    }
    p.rows.forEach((r, i) => {
      const m = marks.children[i];
      m.action = r.action;
      prop(m, "left", r.left);
      prop(m, "top", r.top);
      text(m.label, r.label);
      flag(m, "chosen", r.chosen);
      m.dataset.id = r.id;
      const li = list.children[i];
      li.action = r.action;
      li.dataset.id = r.id;
      text(li.label, r.label);
      text(li.price, r.price || "");
      flag(li, "chosen", r.chosen);
      flag(li, "short", !r.enabled);
      li.button.setAttribute("aria-label", r.price ? `${r.label}, ${r.price}` : r.label);
    });
    coins.hidden = !p.coins;
    if (p.coins) text(coins, `${p.coins.label}: ${p.coins.text}`);
    text(empty, p.empty || "");
    text(note, p.note || "");
  }

  return {
    show(s) {
      flag(box, "shown", s);
      if (!s) return;
      box.dataset.phase = s.phase;
      text(heading, s.heading);
      text(hint, s.pick ? s.pick.hint : s.hint);
      notesPane.hidden = s.phase !== "notes";
      pickPane.hidden = !s.pick;
      if (s.phase === "notes") showStaff(s);
      if (s.pick) showPick(s.pick);
      leave.action = s.leave;
      text(leave, s.leave.label);
    },
  };
}

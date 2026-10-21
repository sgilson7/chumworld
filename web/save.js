// M7 save (P10, T16): the one module that touches browser storage (CW-12).
//
// The core writes the save's text and reads it back (crates/core/src/save.rs);
// this module only keeps that text under one key, hands it back at the start,
// offers it as a download, and reads a file the reader chooses. Every access
// to storage is wrapped: a private window or blocked storage means there is
// no save, never an error. crates/core/tests/one_save.rs checks that no other
// file under web/ names a storage API and that this one uses one key.

const KEY = "chumworld.save";

function store() {
  try {
    return window.localStorage;
  } catch (e) {
    return null;
  }
}

export function makeSave() {
  // the counts last acted on (the core's `ui().save`), so each acts once
  let seen = null;
  let picked = null;
  const input = document.createElement("input");
  input.type = "file";
  input.accept = "application/json,.json";
  input.hidden = true;
  input.id = "save-file";
  document.body.append(input);
  input.addEventListener("change", async () => {
    const file = input.files && input.files[0];
    input.value = "";
    if (!file || !picked) return;
    let text = "";
    try {
      text = await file.text();
    } catch (e) {
      text = "";
    }
    picked(text);
  });

  function write(text) {
    const s = store();
    if (!s || !text) return;
    try {
      s.setItem(KEY, text);
    } catch (e) {
      // full or blocked: the game goes on without a browser save
    }
  }

  function forget() {
    const s = store();
    if (!s) return;
    try {
      s.removeItem(KEY);
    } catch (e) {
      // blocked: nothing was kept
    }
  }

  function download(text, name) {
    if (!text) return;
    const url = URL.createObjectURL(new Blob([text], { type: "application/json" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = name;
    a.hidden = true;
    document.body.append(a);
    a.click();
    a.remove();
    requestAnimationFrame(() => URL.revokeObjectURL(url));
  }

  return {
    // the text kept in this browser, or "" when there is none or it is blocked
    stored() {
      const s = store();
      if (!s) return "";
      try {
        return s.getItem(KEY) || "";
      } catch (e) {
        return "";
      }
    },
    // the core's view: a new save to keep, a download asked for, a save to
    // forget; `text` reads the core's save text
    show(view, text) {
      if (!view) return;
      if (seen) {
        if (view.seq !== seen.seq) write(text());
        if (view.export !== seen.export) download(text(), view.file_name);
        if (view.wipe !== seen.wipe) forget();
      }
      seen = { seq: view.seq, export: view.export, wipe: view.wipe };
    },
    // asks the reader for a file; its text goes to `then`
    pick(then) {
      picked = then;
      input.click();
    },
  };
}

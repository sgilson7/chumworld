//! The brief allows one save in the browser (T16, CW-12, BUILDER-SPEC.md's
//! table of changed defaults for D17), so this replaces the reference
//! default's `no_storage.rs`. It reads every file under web/ (vendor/
//! aside) and fails if any file but `web/save.js` names a browser storage
//! API, if `save.js` names one other than local storage, or if `save.js`
//! keeps more than one key: one `KEY` constant, and every storage call
//! made with it. The core writes and reads what is stored
//! (`crates/core/src/save.rs`), and its tests show an unreadable save is
//! treated as a new game.

use std::fs;
use std::path::{Path, PathBuf};

const STORAGE_NAMES: &[&str] = &[
    "localStorage",
    "sessionStorage",
    "indexedDB",
    "IDBFactory",
    "document.cookie",
    "caches",
    "CacheStorage",
    "serviceWorker",
    "navigator.storage",
    "openDatabase",
];

const SAVE_MODULE: &str = "save.js";

fn storage_names_in(text: &str) -> Vec<&'static str> {
    STORAGE_NAMES
        .iter()
        .copied()
        .filter(|name| text.contains(name))
        .collect()
}

fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(read) = fs::read_dir(&d) else {
            continue;
        };
        for entry in read {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                if path.file_name().map(|n| n != "vendor").unwrap_or(true) {
                    stack.push(path);
                }
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn web() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web")
}

/// The keys a module stores under: each `const KEY = "…"` it declares, and
/// whether every `getItem`, `setItem` and `removeItem` call passes `KEY`.
fn keys_in(text: &str) -> (Vec<String>, bool) {
    let mut keys = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("const KEY = \"") {
            if let Some(end) = rest.find('"') {
                keys.push(rest[..end].to_string());
            }
        }
    }
    let mut all_keyed = true;
    for call in ["getItem(", "setItem(", "removeItem(", "key(", "clear("] {
        let mut from = 0;
        while let Some(at) = text[from..].find(call) {
            let after = &text[from + at + call.len()..];
            if !after.starts_with("KEY") {
                all_keyed = false;
            }
            from += at + call.len();
        }
    }
    (keys, all_keyed)
}

#[test]
fn only_the_save_module_names_browser_storage() {
    let mut found = Vec::new();
    let mut saw_module = false;
    for path in files_under(&web()) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let names = storage_names_in(&text);
        if path.file_name().is_some_and(|n| n == SAVE_MODULE) && path.parent() == Some(web().as_path()) {
            saw_module = true;
            assert_eq!(
                names,
                vec!["localStorage"],
                "web/save.js names a storage API other than local storage"
            );
            continue;
        }
        for name in names {
            found.push(format!("{}: {name}", path.display()));
        }
    }
    assert!(saw_module, "web/save.js is missing");
    assert!(
        found.is_empty(),
        "browser storage outside web/save.js (CW-12):\n{}",
        found.join("\n")
    );
}

#[test]
fn the_save_module_keeps_exactly_one_key() {
    let text = fs::read_to_string(web().join(SAVE_MODULE)).expect("web/save.js");
    let (keys, all_keyed) = keys_in(&text);
    assert_eq!(keys.len(), 1, "one KEY constant: {keys:?}");
    assert!(all_keyed, "a storage call made with something other than KEY");
}

#[test]
fn the_scanners_see_storage_and_a_second_key() {
    assert_eq!(
        storage_names_in("window.localStorage.setItem('n', name)"),
        vec!["localStorage"]
    );
    assert_eq!(storage_names_in("indexedDB.open('x')"), vec!["indexedDB"]);
    assert!(storage_names_in("el(\"note\").value").is_empty());
    let (keys, keyed) = keys_in("const KEY = \"a\";\ns.setItem(KEY, t);\ns.getItem(KEY);");
    assert_eq!(keys, vec!["a"]);
    assert!(keyed);
    let (_, keyed) = keys_in("const KEY = \"a\";\ns.setItem(\"b\", t);");
    assert!(!keyed, "a call with another key is seen");
    let (keys, _) = keys_in("const KEY = \"a\";\nconst KEY = \"b\";");
    assert_eq!(keys.len(), 2);
}

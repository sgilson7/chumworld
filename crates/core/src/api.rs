//! The door into the core. The shim holds one `game::Game` and forwards the
//! page's calls; everything the page draws in HTML comes from `ui`, a view
//! model, and everything it sends back goes through `act(name, value)`,
//! answered `{"ok": <ui>}` or `{"error": "..."}` with a message the page
//! shows as it is. (After Peter's Mandible.)
//!
//! The view model names the screen, its panels (heading, text, rows of key
//! and meaning, buttons with the action each sends), the lines over the
//! game, and the glossary's definitions with their numbers filled in by the
//! core. The page builds the DOM from it without knowing what a panel means.
//! A button marked `engage` also asks the browser for the pointer, which only
//! a click may do.
//!
//! Extension (M4's `ui.rs`): a menu or a screen adds its panels in `ui`
//! below, by screen; a line over the game comes from `World::hud`.

use serde::Serialize;

use crate::game::{Game, Screen};
use crate::input;
use crate::look;
use crate::prim::{self, MeshId};
use crate::render;
use crate::world::HudItem;

pub fn ok<T: Serialize>(value: &T) -> String {
    serde_json::json!({ "ok": value }).to_string()
}

pub fn error(message: String) -> String {
    serde_json::json!({ "error": message }).to_string()
}

/// One button: the action name and value the page sends back through `act`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Action {
    pub name: String,
    pub value: String,
    pub label: String,
    pub enabled: bool,
    /// also take the pointer (pointer lock) on this click
    pub engage: bool,
}

/// One row of a panel's table: a key and what it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    pub key: String,
    pub does: String,
}

/// One block of the screen. Empty fields are left out of the JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Panel {
    pub id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub heading: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub rows_heading: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<Row>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<Action>,
}

/// A term and its definition, for the page's hover hints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hint {
    pub term: String,
    pub definition: String,
}

/// Everything the page draws in HTML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ui {
    pub screen: String,
    pub title: String,
    pub panels: Vec<Panel>,
    pub hud: Vec<HudItem>,
    pub hints: Vec<Hint>,
    /// the text the page shows if it cannot draw (no WebGL 2)
    pub no_webgl: String,
    /// whether the game wants the pointer held (pointer lock): the page lets
    /// go of it when this is false, and takes it again on a click when true
    pub capture: bool,
    pub version: u32,
    /// M4: the HUD, the menu ring, dialogue, the shop and settings (`ui.rs`)
    #[serde(flatten)]
    pub menus: crate::ui::Views,
    /// M7 save: what the page stores, downloads or forgets (`save::View`)
    pub save: crate::save::View,
}

fn controls(g: &Game) -> (String, Vec<Row>) {
    let s = &g.strings;
    let row = |k: &str, d: &str| Row {
        key: s.get(k),
        does: s.get(d),
    };
    (
        s.get("controls"),
        vec![
            row("key_move", "control_move"),
            row("key_run", "control_run"),
            row("key_look", "control_look"),
            row("key_pause", "control_pause"),
            // M1: the duel's controls
            row("key_draw", "control_draw"),
            row("key_hand", "control_hand"),
            row("key_swing", "control_swing"),
            row("key_hold_look", "control_hold_look"),
            row("key_lock", "control_lock"),
            row("key_reach", "control_reach"),
            row("key_throw", "control_throw"),
            row("key_dodge", "control_dodge"),
            row("key_imbue", "control_imbue"), // M6
        ],
    )
}

fn button(g: &Game, name: &str, value: &str, label: &str, engage: bool) -> Action {
    Action {
        name: name.into(),
        value: value.into(),
        label: g.strings.get(label),
        enabled: true,
        engage,
    }
}

/// A definition with each `{placeholder}` replaced by the core's number.
pub fn fill(definition: &str, values: &[(&str, String)]) -> String {
    let mut out = definition.to_string();
    for (k, v) in values {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

/// The view model for the game as it stands.
pub fn ui(g: &Game) -> Ui {
    let s = &g.strings;
    let (rows_heading, rows) = controls(g);
    let mut panels = Vec::new();
    match g.screen {
        Screen::Title => panels.push(Panel {
            id: "title".into(),
            heading: s.get("title"),
            // M7 (T10): how to choose a cut
            text: s.get("swing_how"),
            rows_heading,
            rows,
            // M7 save: Continue when the browser holds a save
            actions: vec![crate::save::title_button(g)
                .unwrap_or_else(|| button(g, "screen.show", "play", "play", true))],
        }),
        Screen::Paused => panels.push(Panel {
            id: "paused".into(),
            heading: s.get("paused"),
            text: s.get("swing_how"), // M7 (T10)
            rows_heading,
            rows,
            actions: vec![button(g, "screen.show", "play", "resume", true)],
        }),
        Screen::Play => {}
        // M4: drawn from `ui::views` below
        Screen::Menu | Screen::Settings | Screen::Dialogue | Screen::Shop => {}
        Screen::Song => {} // M7 travel: drawn from `travel::view`
    }
    // M4: the settings button and the menu's keys on the title and pause
    crate::ui::panels(g, &mut panels);
    // M7 save: New game, Load save file, Download save, and the save's line
    crate::save::panels(g, &mut panels);
    let mut hud = vec![HudItem {
        id: "hint".into(),
        text: s.get("hint_pause"),
        shown: g.hint_shown(),
    }];
    g.world.hud(s, &mut hud);
    let values = g.glossary_values();
    Ui {
        screen: g.screen.name().into(),
        title: s.get("title"),
        panels,
        hud,
        hints: g
            .glossary
            .iter()
            .map(|e| Hint {
                term: e.term.clone(),
                definition: fill(&e.definition, &values),
            })
            .collect(),
        no_webgl: s.get("no_webgl"),
        capture: g.screen == Screen::Play,
        version: g.ui_version,
        menus: crate::ui::views(g),
        save: crate::save::view(g),
    }
}

/// `act` as the page calls it: the new view model, or the error.
pub fn act(g: &mut Game, name: &str, value: &str) -> String {
    match g.act(name, value) {
        Ok(()) => ok(&ui(g)),
        Err(e) => error(e),
    }
}

/// Sizes and layout the page needs to upload and draw: every stride, offset,
/// bit, pass state and uniform name, so the page holds no layout of its own.
pub fn numbers() -> String {
    let b = render::BYTES_PER_VALUE;
    let vec4 = 4 * b;
    serde_json::json!({
        "actions": input::ACTIONS.iter().map(|(n, bit)| (n.to_string(), serde_json::json!(bit))).collect::<serde_json::Map<_, _>>(),
        "vertex": {
            "stride": prim::VERTEX_VALUES * b,
            "attributes": [["aPos", 3, 0], ["aNormal", 3, 3 * b], ["aColor", 3, 6 * b]],
        },
        "instance": {
            "stride": render::INSTANCE_VALUES * b,
            "attributes": [["iA", 4, 0], ["iB", 4, vec4], ["iC", 4, 2 * vec4], ["iD", 4, 3 * vec4]],
        },
        "draw_values": render::DRAW_VALUES,
        "buffers": { "static": render::STATIC_BUFFER, "dynamic": render::DYNAMIC_BUFFER },
        "passes": render::pass_states(),
        // the look's uniforms, then M2's sky's (`sky.rs`)
        "uniforms": look::UNIFORMS.iter().chain(crate::sky::UNIFORMS).map(|(n, call, len)| serde_json::json!([n, call, len, call.contains("Matrix")])).collect::<Vec<_>>(),
        // the view values: each layer's matrix by layer number, then the eye
        "view": { "layers": [[0, 16], [16, 32]], "eye": [32, 35] },
        // M2: `uploads` lists each new mesh as (id, vertex count); the water
        // program's own uniforms (once) and its frame's (each frame), and the
        // nearest distance each layer draws water at, by layer number
        "upload_values": 2,
        "water": {
            "uniforms": crate::water::UNIFORMS.iter().map(|(n, call, len)| serde_json::json!([n, call, len, call.contains("Matrix")])).collect::<Vec<_>>(),
            "frame": crate::water::FRAME.iter().map(|(n, call, len)| serde_json::json!([n, call, len, call.contains("Matrix")])).collect::<Vec<_>>(),
            "cut": [render::NEAR_LAYER_FAR / crate::fx::ONE, 0],
        },
    })
    .to_string()
}

/// Every mesh the page uploads, by number, with its vertex count: the
/// primitives, then the world's.
pub fn meshes(g: &Game) -> String {
    let list: Vec<serde_json::Value> = (0..g.meshes.count() as u32)
        .map(|id| {
            let n = g.meshes.vertices(MeshId(id)).len() / prim::VERTEX_VALUES;
            serde_json::json!({ "id": id, "vertices": n })
        })
        .collect();
    serde_json::Value::Array(list).to_string()
}

/// M4: the map menu's image, RGBA, `ui::MAP_PX` a side.
pub fn map_pixels(g: &Game) -> Vec<u8> {
    crate::ui::map_pixels(g)
}

/// One mesh's vertex values by number; none for a number past the end. A
/// number past the static table is a mesh the detail levels built (M2,
/// `lod.rs`), there while the page holds it after `uploads`.
pub fn mesh(g: &Game, id: u32) -> Vec<i32> {
    if (id as usize) < g.meshes.count() {
        return g.meshes.vertices(MeshId(id));
    }
    g.detail.lent(id).cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tests::game;
    use crate::prim::Mesh;

    fn json(text: &str) -> serde_json::Value {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn the_title_screen_shows_the_title_the_controls_and_a_play_button_that_takes_the_pointer() {
        let g = game();
        let u = json(&ok(&ui(&g)));
        let p = &u["ok"]["panels"][0];
        assert_eq!(u["ok"]["screen"], "title");
        assert_eq!(p["heading"], "CHUMWORLD");
        assert_eq!(p["rows"][1]["key"], "Shift");
        assert_eq!(p["rows"][1]["does"], "Run");
        assert_eq!(p["actions"][0]["label"], "Play");
        assert_eq!(p["actions"][0]["engage"], true);
        assert_eq!(p["actions"][0]["name"], "screen.show");
    }

    #[test]
    fn act_answers_with_the_new_screen_or_the_error() {
        let mut g = game();
        let v = json(&act(&mut g, "screen.show", "play"));
        assert_eq!(v["ok"]["screen"], "play");
        assert!(
            v["ok"]["panels"].as_array().unwrap().is_empty(),
            "nothing covers play"
        );
        assert_eq!(v["ok"]["hud"][0]["shown"], true);
        assert_eq!(v["ok"]["capture"], true, "play holds the pointer");
        let v = json(&act(&mut g, "key.escape", ""));
        assert_eq!(v["ok"]["panels"][0]["heading"], "Paused");
        assert_eq!(v["ok"]["panels"][0]["actions"][0]["label"], "Resume");
        assert_eq!(v["ok"]["capture"], false, "a pause lets go of the pointer");
        let v = json(&act(&mut g, "screen.jump", "x"));
        assert_eq!(v["error"], "Unknown action \"screen.jump\".");
    }

    #[test]
    fn glossary_numbers_are_filled_by_the_core() {
        let g = game();
        let u = ui(&g);
        let run = u
            .hints
            .iter()
            .find(|h| h.term == "Run")
            .expect("Run is in the glossary");
        assert!(run.definition.contains("5 metres a second"), "{}", run.definition);
        let walk = u.hints.iter().find(|h| h.term == "Walk").unwrap();
        assert!(
            walk.definition.contains("2.2 metres a second"),
            "{}",
            walk.definition
        );
        for h in &u.hints {
            assert!(
                !h.definition.contains('{'),
                "{}: a placeholder the core does not fill",
                h.term
            );
        }
    }

    #[test]
    fn numbers_name_every_mesh_pass_and_uniform_the_page_needs() {
        let n = json(&numbers());
        let g = game();
        let m: serde_json::Value = serde_json::from_str(&meshes(&g)).unwrap();
        let list = m.as_array().unwrap();
        assert_eq!(
            list.len(),
            Mesh::ALL.len() + g.meshes.world.len(),
            "the primitives and the world's static meshes (the water's)"
        );
        assert_eq!(list[0]["vertices"], 36, "the cube's 12 triangles");
        assert_eq!(n["vertex"]["stride"], 36);
        assert_eq!(n["passes"].as_array().unwrap().len(), render::Pass::ORDER.len());
        assert_eq!(n["instance"]["stride"], 64);
        assert_eq!(n["actions"]["forward"], 1);
        assert_eq!(n["actions"]["swing"], 1 << 24);
        assert_eq!(n["view"]["eye"][1], render::VIEW_VALUES);
    }
}

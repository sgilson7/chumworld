//! The reader-visible strings, from `data/strings.json`. The core checks the
//! file before anything is drawn: every key the game uses must be there and
//! hold text.
//!
//! The file is one object of key to text. A milestone that needs a string
//! adds its key to the file and to `REQUIRED` (adding entries only), and
//! reads it with `Strings::get`. Keys starting with `_` hold notes.

use std::collections::BTreeMap;

use serde::Serialize;

/// Every key the game reads, so a missing one is found at start, not mid-game.
pub const REQUIRED: &[&str] = &[
    "title",
    "loading",
    "no_webgl",
    "play",
    "paused",
    "resume",
    "controls",
    "key_move",
    "key_run",
    "key_look",
    "key_pause",
    "control_move",
    "control_run",
    "control_look",
    "control_pause",
    "hint_pause",
    // M7 (T14): the sprint
    "key_sprint",
    "control_sprint",
    "sprint",
    "sprint_builds",
    "sprint_times",
    // M1: the duel's controls
    "key_draw",
    "control_draw",
    "key_hand",
    "control_hand",
    "key_swing",
    "control_swing",
    "key_hold_look",
    "control_hold_look",
    "key_lock",
    "control_lock",
    "key_reach",
    "control_reach",
    "key_throw",
    "control_throw",
    "key_dodge",
    "control_dodge",
    // M6
    "key_imbue",
    "control_imbue",
    // M4: the menus, the HUD, dialogue, the shop and settings
    "key_menu",
    "control_menu",
    "key_map",
    "control_map",
    "key_quick",
    "control_quick",
    "select_item",
    "equipment",
    "map",
    "quest",
    "key_q",
    "key_e",
    "key_tab",
    "key_space",
    "key_1",
    "key_2",
    "key_3",
    "close",
    "next",
    "assign_keys",
    "item_short_sword",
    "item_short_sword_about",
    "item_sword",
    "item_sword_about",
    "item_boomerang",
    "item_boomerang_about",
    "item_rock",
    "item_rock_about",
    "item_knife",
    "item_knife_about",
    "item_bandage",
    "item_bandage_about",
    "item_chest_key",
    "item_chest_key_about",
    "item_hood",
    "item_hood_about",
    "item_jacket",
    "item_jacket_about",
    "item_gloves",
    "item_gloves_about",
    "item_trousers",
    "item_trousers_about",
    "item_boots",
    "item_boots_about",
    "carry_of",
    "on_key",
    "assign_hint",
    "no_key",
    "refuse_key",
    "refuse_wear",
    "refuse_missing",
    "refuse_key_number",
    "slot_head",
    "slot_body",
    "slot_hands",
    "slot_legs",
    "slot_feet",
    "sword_hand",
    "in_hand",
    "hold_hint",
    "worn_hint",
    "wear_hint",
    "not_found",
    "speedster_armour",
    "pieces_worn",
    "speed",
    "footing",
    "percent",
    "you",
    "map_legend",
    "walked",
    "place_village",
    "place_camp",
    "place_hut",
    "place_shrine",
    "place_bridge",
    "place_tree",
    "place_scarecrow",
    "place_chest",
    "imbue",
    "imbue_known",
    "imbue_unknown",
    "coins",
    "task_heading",
    "quest_opening",
    "health",
    "health_of",
    "imbue_ready",
    "seconds",
    "received",
    "item_times",
    "who_trainer",
    "who_merchant",
    "who_villager",
    "who_orc",
    "who_bandit",
    "who_leader",
    "who_lookout",
    "shop",
    "for_sale",
    "to_sell",
    "nothing_to_sell",
    "price",
    "buy",
    "sell",
    "leave",
    "refuse_not_sold",
    "refuse_coins",
    "refuse_full",
    "bought",
    "sold",
    "price_one",
    "refuse_coins_one",
    "sold_one",
    "refuse_keep",
    "settings",
    "look_speed",
    "mouse_sensitivity",
    "invert_y",
    "volume",
    // M7 (T10): the swing cue, its setting and how to cut
    "swing_cue",
    "swing_how",
    "swing_level_left",
    "swing_level_right",
    "swing_chop",
    "swing_rising",
    "swing_thrust",
    "swing_diagonal_up_left",
    "swing_diagonal_up_right",
    "swing_diagonal_down_left",
    "swing_diagonal_down_right",
    "on",
    "off",
    "settings_note",
    "back",
    "sample_line_1",
    "sample_line_2",
    // M5: the opening, the story's lines, E's prompt, the merchant, the trainer
    "who_you",
    "dream_line",
    "dream_line_again",
    "story_wake",
    "story_free",
    "story_came_to",
    "story_full",
    "story_coins",
    "story_cart_stopped",
    "story_alarm",
    "story_camp_cleared",
    "story_chest_locked",
    "story_chest_empty",
    "story_learned",
    "quest_trainer",
    "quest_camp",
    "quest_speedster",
    "quest_done",
    "prompt",
    "does_pick_up",
    "does_talk_merchant",
    "does_talk_trainer",
    "does_open_chest",
    "merchant_hello",
    "trainer_1",
    "trainer_2",
    "trainer_3",
    "trainer_draw",
    "trainer_again",
    "trainer_pass_1",
    "trainer_pass_2",
    "trainer_done",
    // M7 roadside (T12): E at the stops, the villagers' lines, the map's marks
    "does_search_cart",
    "does_drink",
    "does_talk_villager",
    "roadside_searched",
    "roadside_drink",
    "roadside_north_1",
    "roadside_north_2",
    "roadside_north_west_1",
    "roadside_north_west_2",
    "roadside_north_east_1",
    "roadside_north_east_2",
    "roadside_west_1",
    "roadside_west_2",
    "place_wreck_burnt",
    "place_watchtower",
    "place_tor",
    "place_roadside_well",
    "place_lone_tree",
    // M7 rumours (T17): the rumours, the people who tell them, the quest lines
    "rumour_hood",
    "rumour_jacket",
    "rumour_gloves",
    "rumour_trousers",
    "rumour_boots",
    "rumour_sword",
    "rumour_chest_key",
    "rumour_boomerang",
    "rumour_imbue",
    "rumour_flute",
    "rumour_traveller",
    "rumour_waystone",
    "rumour_marked",
    "rumour_noted",
    "rumour_villager_done",
    "rumour_woodcutter_done",
    "rumour_fisher_done",
    "rumour_herder_done",
    "woodcutter_hello",
    "fisher_hello",
    "herder_hello",
    "who_woodcutter",
    "who_fisher",
    "who_herder",
    "does_talk_woodcutter",
    "does_talk_fisher",
    "does_talk_herder",
    "lines_heading",
    "lines_none",
    "line_progress",
    "line_done",
    "line_speedster",
    "line_sword",
    "line_chest",
    "line_trainer",
    "line_waystones",
    "line_speedster_done",
    "line_sword_done",
    "line_chest_done",
    "line_trainer_done",
    "line_waystones_done",
    "place_crossroads",
];

/// The reader-visible strings the page needs. They live in data, not source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Strings(pub BTreeMap<String, String>);

/// Why a strings file cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringsError {
    NotJson(String),
    Missing(String),
    Empty(String),
}

impl StringsError {
    /// The message the page shows. Wording that depends on the rule stays
    /// beside the rule.
    pub fn message(&self) -> String {
        match self {
            StringsError::NotJson(detail) => format!("The strings file could not be read: {detail}"),
            StringsError::Missing(key) => format!("The strings file has no \"{key}\"."),
            StringsError::Empty(key) => format!("The strings file has an empty \"{key}\"."),
        }
    }
}

impl Strings {
    /// The text for a key; a key outside `REQUIRED` that is missing reads as
    /// itself, so the gap shows on screen rather than as a blank.
    pub fn get(&self, key: &str) -> String {
        self.0.get(key).cloned().unwrap_or_else(|| key.to_string())
    }
}

/// Parses and checks the strings file. A value that is only whitespace counts
/// as empty, because the page would draw nothing.
pub fn parse_strings(json: &str) -> Result<Strings, StringsError> {
    let all: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(json).map_err(|e| StringsError::NotJson(e.to_string()))?;
    let mut out = BTreeMap::new();
    for (k, v) in all {
        if k.starts_with('_') {
            continue;
        }
        let Some(text) = v.as_str() else {
            return Err(StringsError::NotJson(format!("\"{k}\" is not text")));
        };
        if text.trim().is_empty() {
            return Err(StringsError::Empty(k));
        }
        out.insert(k, text.to_string());
    }
    if let Some(missing) = REQUIRED.iter().find(|k| !out.contains_key(**k)) {
        return Err(StringsError::Missing(missing.to_string()));
    }
    Ok(Strings(out))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn strings() -> Strings {
        parse_strings(include_str!("../../../data/strings.json")).expect("data/strings.json")
    }

    fn with(key: &str, value: serde_json::Value) -> String {
        let mut v: serde_json::Value =
            serde_json::from_str(include_str!("../../../data/strings.json")).unwrap();
        v[key] = value;
        v.to_string()
    }

    #[test]
    fn the_strings_file_holds_every_required_key() {
        let s = strings();
        for k in REQUIRED {
            assert!(!s.get(k).trim().is_empty(), "{k}");
        }
    }

    #[test]
    fn an_empty_title_is_rejected_by_name() {
        let e = parse_strings(&with("title", "  ".into())).unwrap_err();
        assert_eq!(e, StringsError::Empty("title".into()));
        assert_eq!(e.message(), "The strings file has an empty \"title\".");
    }

    #[test]
    fn a_missing_key_is_rejected_by_name() {
        let mut v: serde_json::Value =
            serde_json::from_str(include_str!("../../../data/strings.json")).unwrap();
        v.as_object_mut().unwrap().remove("resume");
        let e = parse_strings(&v.to_string()).unwrap_err();
        assert_eq!(e.message(), "The strings file has no \"resume\".");
    }

    #[test]
    fn a_value_that_is_not_text_and_text_that_is_not_json_are_rejected() {
        assert!(matches!(
            parse_strings(&with("play", 3.into())),
            Err(StringsError::NotJson(_))
        ));
        assert!(matches!(parse_strings("title: T"), Err(StringsError::NotJson(_))));
    }

    #[test]
    fn notes_are_skipped_and_an_unknown_key_reads_as_itself() {
        let s = parse_strings(&with("_note", "for people".into())).unwrap();
        assert!(!s.0.contains_key("_note"));
        assert_eq!(s.get("no.such.key"), "no.such.key");
    }
}

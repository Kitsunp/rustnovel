use super::*;

#[test]
fn parse_condition_supports_var_cmp() {
    let cond = parse_cond_expr("score >= 10").expect("cond");
    match cond {
        CondRaw::VarCmp { key, op, value } => {
            assert_eq!(key, "score");
            assert_eq!(op, CmpOp::Ge);
            assert_eq!(value, 10);
        }
        _ => panic!("expected var cmp"),
    }
}

#[test]
fn parse_menu_option_without_cond() {
    let (text, cond) = parse_menu_option_decl("\"Go\":").expect("menu option");
    assert_eq!(text, "Go");
    assert!(cond.is_none());
}

#[test]
fn parse_dialogue_alias_resolution() {
    let mut aliases = std::collections::HashMap::new();
    aliases.insert("e".to_string(), "Eileen".to_string());
    let dialogue = parse_dialogue_line("e \"Hello\"", &aliases).expect("dialogue");
    assert_eq!(dialogue.speaker, "Eileen");
    assert_eq!(dialogue.text, "Hello");
}

#[test]
fn parse_dialogue_single_quotes() {
    let aliases = std::collections::HashMap::new();
    let dialogue = parse_dialogue_line("e 'Hola'", &aliases).expect("dialogue");
    assert_eq!(dialogue.speaker, "e");
    assert_eq!(dialogue.text, "Hola");
}

#[test]
fn parse_show_bg_alias_maps_to_background_patch() {
    let mut aliases = std::collections::HashMap::new();
    aliases.insert("bg street".to_string(), "images/bg/street.png".to_string());
    let parsed = parse_show_decl("show bg street", &aliases).expect("show parse");
    assert_eq!(
        parsed.patch.background.as_deref(),
        Some("images/bg/street.png")
    );
    assert!(parsed.patch.add.is_empty());
}

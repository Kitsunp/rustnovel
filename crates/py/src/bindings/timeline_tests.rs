use super::*;

#[test]
fn parse_easing_rejects_unknown_tokens() {
    let err = parse_easing("ease_sideways")
        .expect_err("unknown easing tokens must be rejected instead of becoming linear");

    assert!(err.to_string().contains("unknown easing"));
}

#[test]
fn parse_easing_accepts_documented_aliases() {
    assert_eq!(parse_easing("easein").unwrap(), Easing::EaseIn);
    assert_eq!(parse_easing("ease_in_out").unwrap(), Easing::EaseInOut);
}

#[test]
fn parse_property_rejects_unknown_tokens() {
    let err = parse_property("blur_radius")
        .expect_err("unknown property tokens must be rejected instead of becoming position_x");

    assert!(err.to_string().contains("unknown property"));
}

#[test]
fn parse_property_accepts_documented_aliases() {
    assert_eq!(parse_property("x").unwrap(), PropertyType::PositionX);
    assert_eq!(parse_property("alpha").unwrap(), PropertyType::Opacity);
}

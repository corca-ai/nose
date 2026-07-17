use super::*;

#[test]
fn colors_canonicalize_to_one_form() {
    for eq in [
        vec![
            "#fff",
            "#ffffff",
            "#FFFFFF",
            "#FFFF",
            "white",
            "WHITE",
            "rgb(255,255,255)",
            "rgb(255 255 255)",
            "#ffffffff",
        ],
        vec!["#000", "#000000", "black", "rgb(0,0,0)", "rgba(0,0,0,1)"],
        vec!["#ff0000", "red", "RED", "#f00", "rgb(255, 0, 0)"],
    ] {
        let first = normalize_color(eq[0]).unwrap();
        for c in &eq {
            assert_eq!(normalize_color(c).as_deref(), Some(first.as_str()), "{c}");
        }
    }
}

#[test]
fn colors_that_differ_stay_distinct() {
    assert_ne!(normalize_color("#f00"), normalize_color("#00f"));
    assert_ne!(normalize_color("red"), normalize_color("blue"));
    // Fractional alpha is left untouched (not guessed) -> not equal to opaque.
    assert_eq!(normalize_color("rgba(0,0,0,0.5)"), None);
    // currentColor is not a fixed color.
    assert_eq!(normalize_color("currentColor"), None);
}

#[test]
fn zero_length_units_collapse_but_percent_time_angle_do_not() {
    for z in ["0", "0px", "0em", "0rem", "0vh", "0.0px", "0pt"] {
        assert_eq!(normalize_number(z).as_deref(), Some("0"), "{z}");
    }
    assert_eq!(normalize_number("0%").as_deref(), Some("0%"));
    assert_eq!(normalize_number("0s").as_deref(), Some("0s"));
    assert_eq!(normalize_number("0deg").as_deref(), Some("0deg"));
}

#[test]
fn number_spelling_canonicalizes() {
    assert_eq!(normalize_number("1.0px").as_deref(), Some("1px"));
    assert_eq!(normalize_number("1.50").as_deref(), Some("1.5"));
    assert_eq!(normalize_number(".5em").as_deref(), Some("0.5em"));
    assert_eq!(normalize_number("+2px").as_deref(), Some("2px"));
    assert_eq!(normalize_number("-0").as_deref(), Some("0"));
    assert_eq!(normalize_number("auto"), None); // not a number
}

#[test]
fn custom_property_tokens_preserve_spelling_and_casing() {
    assert_eq!(canonicalize_value("--color", &["white"]), vec!["white"]);
    assert_eq!(canonicalize_value("--color", &["#FFF"]), vec!["#FFF"]);
    assert_eq!(canonicalize_value("--length", &["0px"]), vec!["0px"]);
    assert_eq!(canonicalize_value("--keyword", &["1PX"]), vec!["1PX"]);
}

#[test]
fn box_shorthand_collapses_soundly() {
    fn v(s: &str) -> Vec<&str> {
        s.split_whitespace().collect()
    }
    assert_eq!(canonicalize_value("margin", &v("0 0 0 0")), vec!["0"]);
    assert_eq!(canonicalize_value("margin", &v("0px 0 0em 0")), vec!["0"]);
    assert_eq!(
        canonicalize_value("padding", &v("1px 2px 1px 2px")),
        vec!["1px", "2px"]
    );
    assert_eq!(
        canonicalize_value("margin", &v("1px 2px 3px 2px")),
        vec!["1px", "2px", "3px"]
    );
    // Hard negatives: these must NOT collapse to the same thing.
    assert_ne!(
        canonicalize_value("margin", &v("0 1px")),
        canonicalize_value("margin", &v("1px")),
    );
    assert_ne!(
        canonicalize_value("margin", &v("1px 2px 3px 4px")),
        canonicalize_value("margin", &v("1px 2px 3px")),
    );
    // A non-box property is never collapsed.
    assert_eq!(
        canonicalize_value("transition", &v("0 0 0 0")),
        v("0 0 0 0")
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
    );
}

//! Sound, conservative CSS value canonicalization toward **computed equivalence**.
//!
//! These are the low-level primitives the CSS/HTML declarative fingerprint
//! ([`crate::css`], [`crate::html`]) uses to canonicalize value tokens: parse a color
//! to a canonical hex, parse a number/length to a canonical spelling, and collapse box
//! shorthands. They are deliberately conservative — a token that is not *provably*
//! equivalent to its canonical form is returned UNCHANGED, so the worst case is a missed
//! merge (recall), never a false merge (soundness).
//!
//! This is the soundness-bearing step of the declarative fingerprint: the fingerprint
//! IS the canonical computed style (no IL rewrite), so *equal fingerprint ⟹ equal
//! computed style* holds by construction PROVIDED every canonicalization here is
//! meaning-preserving. That precondition is `empirical-only` — defended by the
//! adversarial per-rule batteries (the tests below plus the CSS/HTML convergence and
//! hard-negative tests in `crates/nose-cli/tests/equivalence.rs`), not by Lean. See the
//! obligation registered in `formal/obligations/normalize/css/computed_style/`.
//!
//! proof-obligation: normalize.css.computed_style

/// Canonicalize a declaration's value (a list of raw tokens) toward computed
/// equivalence: normalize each token, then collapse a box-model shorthand if the
/// property is one. Returns the canonical token list (length may shrink on collapse).
pub(crate) fn canonicalize_value(property: &str, tokens: &[&str]) -> Vec<String> {
    let norm: Vec<String> = tokens.iter().map(|t| normalize_token(t)).collect();
    if is_box_shorthand(property) {
        collapse_box(&norm)
    } else if is_two_axis_shorthand(property) {
        collapse_two_axis(&norm)
    } else {
        norm
    }
}

/// Normalize a single value token: a color → canonical hex; a number/length →
/// canonical spelling; otherwise unchanged.
pub(crate) fn normalize_token(tok: &str) -> String {
    if let Some(c) = normalize_color(tok) {
        return c;
    }
    if let Some(n) = normalize_number(tok) {
        return n;
    }
    tok.to_string()
}

// ----- colors -----

/// A color token → canonical `#rrggbb` / `#rrggbbaa` (lowercase). `None` if `tok` is
/// not a color we can canonicalize EXACTLY (left untouched, so never a false merge).
pub(crate) fn normalize_color(tok: &str) -> Option<String> {
    let t = tok.trim();
    if let Some(hex) = t.strip_prefix('#') {
        return canonical_hex(hex);
    }
    if let Some(rgba) = parse_rgb_func(t) {
        return Some(rgba);
    }
    named_color(&t.to_ascii_lowercase())
}

/// `#rgb` / `#rgba` / `#rrggbb` / `#rrggbbaa` → lowercase 6- or 8-digit form, dropping
/// a fully-opaque alpha (`…ff` → 6-digit).
fn canonical_hex(hex: &str) -> Option<String> {
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let h = hex.to_ascii_lowercase();
    let expand = |s: &str| -> String { s.chars().flat_map(|c| [c, c]).collect() };
    let full = match h.len() {
        3 => expand(&h),
        4 => expand(&h),
        6 | 8 => h,
        _ => return None,
    };
    // Drop a fully-opaque trailing alpha so `#rrggbbff` ≡ `#rrggbb`.
    if full.len() == 8 && full.ends_with("ff") {
        return Some(format!("#{}", &full[..6]));
    }
    Some(format!("#{full}"))
}

/// `rgb(r,g,b)` / `rgb(r g b)` / `rgba(r,g,b,a)` with INTEGER channels (and `a` either
/// 1 or 0) → canonical hex. Anything with percentages, non-integers, or fractional
/// alpha returns `None` (left untouched — fail closed, no guess).
fn parse_rgb_func(t: &str) -> Option<String> {
    let lower = t.to_ascii_lowercase();
    let inner = lower
        .strip_prefix("rgba(")
        .or_else(|| lower.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let parts: Vec<&str> = inner
        .split([',', '/', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() != 3 && parts.len() != 4 {
        return None;
    }
    let mut bytes = [0u8; 3];
    for (i, p) in parts.iter().take(3).enumerate() {
        let v: u32 = p.parse().ok()?; // integer only; percentages/floats → None
        if v > 255 {
            return None;
        }
        bytes[i] = v as u8;
    }
    let hex6 = format!("#{:02x}{:02x}{:02x}", bytes[0], bytes[1], bytes[2]);
    if parts.len() == 4 {
        match parts[3] {
            "1" | "1.0" | "100%" => Some(hex6),
            "0" | "0.0" | "0%" => Some(format!("{hex6}00")),
            _ => None, // fractional alpha: leave the rgba() spelling untouched
        }
    } else {
        Some(hex6)
    }
}

// ----- numbers / lengths -----

/// A number or length → canonical spelling: drop a `+` sign and trailing-zero noise
/// (`1.0px`→`1px`, `.50`→`0.5`), and collapse a ZERO length (`0px`/`0em`/…) to `0`.
/// `None` if `tok` is not a number/length (so non-numeric tokens are left untouched).
/// Percentages, times, and angles keep their unit (`0%`, `0s`, `0deg` are NOT `0`).
pub(crate) fn normalize_number(tok: &str) -> Option<String> {
    let t = tok.trim();
    let body = t.strip_prefix('+').unwrap_or(t);
    let split = body
        .find(|c: char| c.is_ascii_alphabetic() || c == '%')
        .unwrap_or(body.len());
    let (num, unit) = body.split_at(split);
    if num.is_empty() || !is_numeric(num) {
        return None;
    }
    let canon_num = canonical_number_text(num)?;
    let unit_lc = unit.to_ascii_lowercase();
    if canon_num == "0" && is_length_unit(&unit_lc) {
        return Some("0".to_string()); // 0px ≡ 0em ≡ 0 (length zero is unit-free)
    }
    Some(format!("{canon_num}{unit_lc}"))
}

fn is_numeric(s: &str) -> bool {
    let mut seen_dot = false;
    let mut seen_digit = false;
    for c in s.chars() {
        match c {
            '0'..='9' => seen_digit = true,
            '.' if !seen_dot => seen_dot = true,
            '-' => {}
            _ => return false,
        }
    }
    seen_digit
}

/// Canonical numeric text WITHOUT unit: sign preserved, leading zero ensured
/// (`.5`→`0.5`), trailing zeros trimmed (`1.50`→`1.5`, `1.0`→`1`), `-0`→`0`. Uses
/// string surgery (no float parse) so canonicalization is exact.
fn canonical_number_text(num: &str) -> Option<String> {
    let (sign, rest) = match num.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", num),
    };
    let (int_part, frac_part) = match rest.split_once('.') {
        Some((i, f)) => (i, f),
        None => (rest, ""),
    };
    let int_trimmed = int_part.trim_start_matches('0');
    let frac_trimmed = frac_part.trim_end_matches('0');
    let int_canon = if int_trimmed.is_empty() {
        "0"
    } else {
        int_trimmed
    };
    let mut out = String::new();
    if !frac_trimmed.is_empty() {
        out.push_str(int_canon);
        out.push('.');
        out.push_str(frac_trimmed);
    } else {
        out.push_str(int_canon);
    }
    if out == "0" {
        return Some("0".to_string()); // -0 ≡ 0
    }
    Some(format!("{sign}{out}"))
}

fn is_length_unit(u: &str) -> bool {
    matches!(
        u,
        "px" | "em"
            | "rem"
            | "ex"
            | "ch"
            | "cap"
            | "ic"
            | "lh"
            | "rlh"
            | "vw"
            | "vh"
            | "vi"
            | "vb"
            | "vmin"
            | "vmax"
            | "cm"
            | "mm"
            | "q"
            | "in"
            | "pt"
            | "pc"
    )
}

// ----- box shorthand collapse -----

fn is_box_shorthand(p: &str) -> bool {
    matches!(
        p,
        "margin"
            | "padding"
            | "inset"
            | "border-width"
            | "border-style"
            | "border-color"
            | "scroll-margin"
            | "scroll-padding"
    )
}

/// Two-axis shorthands (`gap`, `overflow`, …): `a a` ≡ `a`.
fn is_two_axis_shorthand(p: &str) -> bool {
    matches!(
        p,
        "gap" | "overflow" | "overscroll-behavior" | "place-items" | "place-content"
    )
}

/// Collapse a 1–4 value box shorthand to its canonical shortest form (CSS rules):
/// `a a a a`→`a`, `a b a b`→`a b`, `a b c b`→`a b c`.
fn collapse_box(v: &[String]) -> Vec<String> {
    if v.len() != 4 {
        return v.to_vec();
    }
    let (t, r, b, l) = (&v[0], &v[1], &v[2], &v[3]);
    if t == r && r == b && b == l {
        vec![t.clone()]
    } else if t == b && r == l {
        vec![t.clone(), r.clone()]
    } else if r == l {
        vec![t.clone(), r.clone(), b.clone()]
    } else {
        v.to_vec()
    }
}

fn collapse_two_axis(v: &[String]) -> Vec<String> {
    if v.len() == 2 && v[0] == v[1] {
        vec![v[0].clone()]
    } else {
        v.to_vec()
    }
}

/// A small but real CSS named-color → hex table (lowercase keys). Extending it only
/// adds recall; it never affects soundness (an unknown name is left untouched).
fn named_color(name: &str) -> Option<String> {
    let hex = match name {
        "transparent" => "#00000000",
        "black" => "#000000",
        "silver" => "#c0c0c0",
        "gray" | "grey" => "#808080",
        "white" => "#ffffff",
        "maroon" => "#800000",
        "red" => "#ff0000",
        "purple" => "#800080",
        "fuchsia" | "magenta" => "#ff00ff",
        "green" => "#008000",
        "lime" => "#00ff00",
        "olive" => "#808000",
        "yellow" => "#ffff00",
        "navy" => "#000080",
        "blue" => "#0000ff",
        "teal" => "#008080",
        "aqua" | "cyan" => "#00ffff",
        "orange" => "#ffa500",
        "pink" => "#ffc0cb",
        "gold" => "#ffd700",
        "indigo" => "#4b0082",
        "violet" => "#ee82ee",
        "tomato" => "#ff6347",
        "crimson" => "#dc143c",
        "salmon" => "#fa8072",
        "khaki" => "#f0e68c",
        "coral" => "#ff7f50",
        "turquoise" => "#40e0d0",
        "lavender" => "#e6e6fa",
        "beige" => "#f5f5dc",
        "ivory" => "#fffff0",
        "wheat" => "#f5deb3",
        "chocolate" => "#d2691e",
        "tan" => "#d2b48c",
        "whitesmoke" => "#f5f5f5",
        "rebeccapurple" => "#663399",
        _ => return None,
    };
    Some(hex.to_string())
}

#[cfg(test)]
mod tests {
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
        // Fractional alpha is left untouched (not guessed) → not equal to opaque.
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
}

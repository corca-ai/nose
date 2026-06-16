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
    if let Some(u) = normalize_url(tok) {
        return u;
    }
    if let Some(h) = normalize_color_function_spelling(tok) {
        return h;
    }
    if let Some(n) = normalize_number(tok) {
        return n;
    }
    tok.to_string()
}

/// `url("x")` / `url('x')` / `url(x)` → `url(x)` (the quotes are insignificant). `None`
/// if not a `url(...)` token.
fn normalize_url(tok: &str) -> Option<String> {
    let lower_prefix = tok.get(..4)?.to_ascii_lowercase();
    if lower_prefix != "url(" || !tok.ends_with(')') {
        return None;
    }
    let inner = tok[4..tok.len() - 1].trim();
    let unquoted = inner.trim_matches(|c| c == '"' || c == '\'');
    Some(format!("url({unquoted})"))
}

/// Canonicalize the SPELLING (separators) of an `hsl()/hsla()/hwb()` color — `hsl(0,
/// 100%, 50%)` ≡ `hsl(0 100% 50%)` — without converting to hex (that needs rounding the
/// browser may do differently, a false-merge risk). `None` if not such a function. The
/// rgb/rgba → hex conversion is EXACT (channels are the 8-bit values) and stays in
/// [`normalize_color`]; only the rounding-bearing models are spelling-only here.
fn normalize_color_function_spelling(tok: &str) -> Option<String> {
    let lower = tok.to_ascii_lowercase();
    let name = ["hsla(", "hsl(", "hwb("]
        .into_iter()
        .find(|p| lower.starts_with(p))?;
    let inner = lower.strip_prefix(name)?.strip_suffix(')')?;
    let parts: Vec<&str> = inner
        .split([',', '/', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    Some(format!("{name}{})", parts.join(" ")))
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

/// The full CSS named-color → spec hex table (lowercase keys). These mappings are
/// EXACT (the spec defines the hex), so converting a name to hex is sound — it never
/// merges colors that render differently; an unknown name is left untouched.
// A flat data table, not branching logic — the line-count cap doesn't apply.
#[allow(clippy::too_many_lines)]
fn named_color(name: &str) -> Option<String> {
    let hex = match name {
        "transparent" => "#00000000",
        "aliceblue" => "#f0f8ff",
        "antiquewhite" => "#faebd7",
        "aqua" | "cyan" => "#00ffff",
        "aquamarine" => "#7fffd4",
        "azure" => "#f0ffff",
        "beige" => "#f5f5dc",
        "bisque" => "#ffe4c4",
        "black" => "#000000",
        "blanchedalmond" => "#ffebcd",
        "blue" => "#0000ff",
        "blueviolet" => "#8a2be2",
        "brown" => "#a52a2a",
        "burlywood" => "#deb887",
        "cadetblue" => "#5f9ea0",
        "chartreuse" => "#7fff00",
        "chocolate" => "#d2691e",
        "coral" => "#ff7f50",
        "cornflowerblue" => "#6495ed",
        "cornsilk" => "#fff8dc",
        "crimson" => "#dc143c",
        "darkblue" => "#00008b",
        "darkcyan" => "#008b8b",
        "darkgoldenrod" => "#b8860b",
        "darkgray" | "darkgrey" => "#a9a9a9",
        "darkgreen" => "#006400",
        "darkkhaki" => "#bdb76b",
        "darkmagenta" => "#8b008b",
        "darkolivegreen" => "#556b2f",
        "darkorange" => "#ff8c00",
        "darkorchid" => "#9932cc",
        "darkred" => "#8b0000",
        "darksalmon" => "#e9967a",
        "darkseagreen" => "#8fbc8f",
        "darkslateblue" => "#483d8b",
        "darkslategray" | "darkslategrey" => "#2f4f4f",
        "darkturquoise" => "#00ced1",
        "darkviolet" => "#9400d3",
        "deeppink" => "#ff1493",
        "deepskyblue" => "#00bfff",
        "dimgray" | "dimgrey" => "#696969",
        "dodgerblue" => "#1e90ff",
        "firebrick" => "#b22222",
        "floralwhite" => "#fffaf0",
        "forestgreen" => "#228b22",
        "fuchsia" | "magenta" => "#ff00ff",
        "gainsboro" => "#dcdcdc",
        "ghostwhite" => "#f8f8ff",
        "gold" => "#ffd700",
        "goldenrod" => "#daa520",
        "gray" | "grey" => "#808080",
        "green" => "#008000",
        "greenyellow" => "#adff2f",
        "honeydew" => "#f0fff0",
        "hotpink" => "#ff69b4",
        "indianred" => "#cd5c5c",
        "indigo" => "#4b0082",
        "ivory" => "#fffff0",
        "khaki" => "#f0e68c",
        "lavender" => "#e6e6fa",
        "lavenderblush" => "#fff0f5",
        "lawngreen" => "#7cfc00",
        "lemonchiffon" => "#fffacd",
        "lightblue" => "#add8e6",
        "lightcoral" => "#f08080",
        "lightcyan" => "#e0ffff",
        "lightgoldenrodyellow" => "#fafad2",
        "lightgray" | "lightgrey" => "#d3d3d3",
        "lightgreen" => "#90ee90",
        "lightpink" => "#ffb6c1",
        "lightsalmon" => "#ffa07a",
        "lightseagreen" => "#20b2aa",
        "lightskyblue" => "#87cefa",
        "lightslategray" | "lightslategrey" => "#778899",
        "lightsteelblue" => "#b0c4de",
        "lightyellow" => "#ffffe0",
        "lime" => "#00ff00",
        "limegreen" => "#32cd32",
        "linen" => "#faf0e6",
        "maroon" => "#800000",
        "mediumaquamarine" => "#66cdaa",
        "mediumblue" => "#0000cd",
        "mediumorchid" => "#ba55d3",
        "mediumpurple" => "#9370db",
        "mediumseagreen" => "#3cb371",
        "mediumslateblue" => "#7b68ee",
        "mediumspringgreen" => "#00fa9a",
        "mediumturquoise" => "#48d1cc",
        "mediumvioletred" => "#c71585",
        "midnightblue" => "#191970",
        "mintcream" => "#f5fffa",
        "mistyrose" => "#ffe4e1",
        "moccasin" => "#ffe4b5",
        "navajowhite" => "#ffdead",
        "navy" => "#000080",
        "oldlace" => "#fdf5e6",
        "olive" => "#808000",
        "olivedrab" => "#6b8e23",
        "orange" => "#ffa500",
        "orangered" => "#ff4500",
        "orchid" => "#da70d6",
        "palegoldenrod" => "#eee8aa",
        "palegreen" => "#98fb98",
        "paleturquoise" => "#afeeee",
        "palevioletred" => "#db7093",
        "papayawhip" => "#ffefd5",
        "peachpuff" => "#ffdab9",
        "peru" => "#cd853f",
        "pink" => "#ffc0cb",
        "plum" => "#dda0dd",
        "powderblue" => "#b0e0e6",
        "purple" => "#800080",
        "rebeccapurple" => "#663399",
        "red" => "#ff0000",
        "rosybrown" => "#bc8f8f",
        "royalblue" => "#4169e1",
        "saddlebrown" => "#8b4513",
        "salmon" => "#fa8072",
        "sandybrown" => "#f4a460",
        "seagreen" => "#2e8b57",
        "seashell" => "#fff5ee",
        "sienna" => "#a0522d",
        "silver" => "#c0c0c0",
        "skyblue" => "#87ceeb",
        "slateblue" => "#6a5acd",
        "slategray" | "slategrey" => "#708090",
        "snow" => "#fffafa",
        "springgreen" => "#00ff7f",
        "steelblue" => "#4682b4",
        "tan" => "#d2b48c",
        "teal" => "#008080",
        "thistle" => "#d8bfd8",
        "tomato" => "#ff6347",
        "turquoise" => "#40e0d0",
        "violet" => "#ee82ee",
        "wheat" => "#f5deb3",
        "white" => "#ffffff",
        "whitesmoke" => "#f5f5f5",
        "yellow" => "#ffff00",
        "yellowgreen" => "#9acd32",
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

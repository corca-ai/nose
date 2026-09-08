use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};

mod incremental;
pub(crate) use incremental::apply_cached_family_lines;

/// Anti-unify N line-blocks at line granularity. Anchored on the first (largest) copy,
/// a line *survives* into the shared body only if it is matched in *every* other copy
/// (each copy votes via a pairwise `line_diff` against the anchor); any maximal run of
/// non-surviving anchor lines collapses to one `⟨region N⟩` placeholder. Returns the
/// skeleton, the count of lines shared across all copies, and the parameter count. With
/// two copies this is exactly the old pairwise anti-unification; with more, it is the
/// honest intersection — what the `--show proposal` view renders.
pub(crate) fn anti_unify_all(members: &[Vec<String>]) -> (Vec<String>, u32, u32) {
    let anchor: Vec<&str> = members[0].iter().map(String::as_str).collect();
    let n = anchor.len();
    // survive[i]: anchor line i is matched in every other copy.
    let mut survive = vec![true; n];
    for other in &members[1..] {
        let b: Vec<&str> = other.iter().map(String::as_str).collect();
        let mut matched = vec![false; n];
        let mut ai = 0usize;
        for (tag, _line) in line_diff(&anchor, &b) {
            match tag {
                // matched line — advances the anchor cursor and votes the line in.
                ' ' => {
                    if ai < n {
                        matched[ai] = true;
                    }
                    ai += 1;
                }
                // anchor-only line — advances the cursor, not voted in.
                '-' => ai += 1,
                // other-only line ('+') — does not advance the anchor cursor.
                _ => {}
            }
        }
        for (s, m) in survive.iter_mut().zip(matched) {
            *s &= m;
        }
    }
    let mut skeleton: Vec<String> = Vec::new();
    let mut shared = 0u32;
    let mut params = 0u32;
    // The open hole, if any: (the skeleton slot to fill once it closes, the placeholder
    // indent, and the anchor lines that vary across it — kept so the placeholder can carry a
    // value-class hint for the helper signature, #374 item 6).
    let mut hole: Option<(usize, &str, Vec<&str>)> = None;
    for (line, &kept) in anchor.iter().zip(&survive) {
        if kept {
            if let Some((slot, indent, lines)) = hole.take() {
                skeleton[slot] = format!("{indent}⟨region {params}: {}⟩", classify_param(&lines));
            }
            shared += 1;
            skeleton.push((*line).to_string());
        } else {
            match &mut hole {
                Some((_, _, lines)) => lines.push(line),
                None => {
                    params += 1;
                    let indent = &line[..line.len() - line.trim_start().len()];
                    let slot = skeleton.len();
                    skeleton.push(String::new());
                    hole = Some((slot, indent, vec![line]));
                }
            }
        }
    }
    if let Some((slot, indent, lines)) = hole.take() {
        skeleton[slot] = format!("{indent}⟨region {params}: {}⟩", classify_param(&lines));
    }
    (skeleton, shared, params)
}

/// A coarse value-class for one skeleton hole, from its (line-granularity) varying text — a
/// signature hint for the extracted helper, not a proof: `literal` (a constant → a value
/// parameter), `name` (a bare identifier), `call` (a call expression → maybe a closure/fn
/// parameter), `block` (a multi-line region → a large or divergent parameter), or `expr`
/// (anything else single-line).
pub(crate) fn classify_param(lines: &[&str]) -> &'static str {
    if lines.len() > 1 {
        return "block";
    }
    let t = lines.first().map_or("", |s| s.trim());
    let Some(first) = t.chars().next() else {
        return "expr";
    };
    if first.is_ascii_digit() || matches!(first, '"' | '\'' | '`') {
        "literal"
    } else if t.ends_with(')') && t.contains('(') {
        "call"
    } else if t
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.'))
    {
        "name"
    } else {
        "expr"
    }
}

/// The invariant (shared) source lines across a family, plus the parameter count — the
/// honest counterpart to structural similarity. Returns *all* shared lines, including
/// boilerplate (`if err != nil {`, `}`): when a family genuinely shares a block, that
/// boilerplate is part of the helper you'd extract. The caller separates signal from
/// noise by *gating* on the substantive (non-trivial, rare) shared lines — a family
/// that shares only boilerplate scores ~0, while one with real shared content is
/// credited for its whole block (this is what stops idioms from ranking yet still
/// credits a `resolve*()` trio that shares a 13-line skeleton around a few varying args).
///
/// The shared set is intersected over a *majority* of members (up to `MEMBER_CAP`), not
/// just the closest pair — so a diverging copy shrinks the count honestly rather than
/// the flattering pair count overstating `N of M shared`. Parameters come from the first
/// pair that reads (a lower bound on the varying spots). `None` if no pair reads.
/// What the difference analysis yields for a family: the lines that drive the
/// *ranking* weight, the *displayed* invariant-line count, and the parameter
/// count — kept as three values because the display count and the ranking set
/// answer different questions (coevo S4-C2).
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub(crate) struct SharedLines {
    /// Majority-voted invariant lines (deduped, sorted) — the robust signal the
    /// ranking weights by IDF. Robustness is the point: a 6-copy family isn't
    /// tanked because its 6th copy diverges.
    pub(crate) rank_lines: Vec<String>,
    /// Lines invariant across **all** copies (#366) — the all-copies anti-unification
    /// count, the same number `nose query` shows, so analysis and query report one
    /// shared/removable headline per family. Bounded by the representative pair's
    /// invariant count (`display ≤ rep-pair-invariant`), so with `params` (rep-pair
    /// holes ≤ `M − rep-pair-invariant`) it still holds `display + params ≤ M`: the
    /// `N of M shared, K spots differ` summary can never read `5 of 6 + 2 spots`
    /// (the §S4-C2 self-contradiction).
    pub(crate) display: u32,
    /// Hole count over the same all-copies intersection as `display`. Query
    /// output uses this count, so computing it here avoids repeating every
    /// source read and N-way diff during JSON rendering.
    pub(crate) display_params: u32,
    /// The representative pair's hole count — `K` in `N of M shared, K spots differ`,
    /// kept tied to `varying_spots` and the `param_penalty`/`shallow-extraction`
    /// ranking. Deliberately representative-pair, not all-copies: the all-copies
    /// hole count was gold-set-measured into the shallow ratio and regressed held-out
    /// (experiments §CL), so only `display` moved to the all-copies basis.
    pub(crate) params: u32,
}

pub(crate) fn shared_lines_of(
    locs: &[nose_detect::Loc],
    cache: &mut FileLineCache,
) -> Option<SharedLines> {
    const MEMBER_CAP: usize = 8;
    // Read the anchor (largest copy) and up to MEMBER_CAP-1 others once.
    let anchor = cache.slice(&locs[0].file, locs[0].start_line, locs[0].end_line)?;
    let ar: Vec<&str> = anchor.iter().map(String::as_str).collect();
    let mut display_survive = vec![true; ar.len()];
    // The pairwise pass against the anchor feeds the majority-vote `rank_lines`
    // (→ `shared_weight`) and `params` (the representative-pair hole count, which stays
    // tied to `varying_spots` and drives `param_penalty`/`shallow-extraction`). These are
    // the ranking inputs. The same diff also votes the all-copies display count, avoiding a
    // second anti-unification pass over the same member slices.
    let mut counts: FxHashMap<String, usize> = FxHashMap::default();
    let mut n_others = 0usize;
    let mut params = 0u32;
    for b in locs.iter().skip(1).take(MEMBER_CAP - 1) {
        let Some(lb) = cache.slice(&b.file, b.start_line, b.end_line) else {
            continue;
        };
        let br: Vec<&str> = lb.iter().map(String::as_str).collect();
        let mut shared = Vec::new();
        let mut display_matched = vec![false; ar.len()];
        let mut ai = 0usize;
        let mut p = 0u32;
        let mut in_hole = false;
        for (tag, line) in &line_diff(&ar, &br) {
            if *tag == ' ' {
                if ai < display_matched.len() {
                    display_matched[ai] = true;
                }
                ai += 1;
                in_hole = false;
                let t = line.trim();
                if !t.is_empty() {
                    shared.push(t.to_string());
                }
            } else {
                if *tag == '-' {
                    ai += 1;
                }
                if !in_hole {
                    in_hole = true;
                    p += 1;
                }
            }
        }
        for (survive, matched) in display_survive.iter_mut().zip(display_matched) {
            *survive &= matched;
        }
        // Params come from the first pair that actually reads (the rep pair).
        if n_others == 0 {
            params = p;
        }
        n_others += 1;
        shared.sort_unstable();
        shared.dedup();
        for l in shared {
            *counts.entry(l).or_insert(0) += 1;
        }
    }
    if n_others == 0 {
        return None;
    }
    // Display: lines invariant across **all** copies (#366) — the same all-copies
    // anti-unification `nose query` renders, so analysis and query report one shared/removable
    // headline per family (the old pairwise count over-stated families whose 3rd+ copies
    // diverge). Display-only and gold-set-measured ranking-neutral: the order reads
    // `shared_weight`/`params`, never this. (All-copies *params* was measured too and
    // regressed held-out — experiments §CL — so `params` stays representative-pair.)
    let mut display = 0u32;
    let mut display_params = 0u32;
    let mut in_hole = false;
    for survives in display_survive {
        if survives {
            display += 1;
            in_hole = false;
        } else if !in_hole {
            display_params += 1;
            in_hole = true;
        }
    }
    let need = ((n_others as f64) * 0.6).ceil().max(1.0) as usize;
    let mut rank_lines: Vec<String> = counts
        .into_iter()
        .filter(|(_, c)| *c >= need)
        .map(|(l, _)| l)
        .collect();
    // Sort to a deterministic order: the caller sums `idf.weight()` over these lines,
    // and float addition isn't associative, so a `HashMap`-iteration order would make
    // `shared_weight` (and, via sort ties, the family order) vary run-to-run and across
    // thread counts — violating the byte-identical-output guarantee.
    rank_lines.sort_unstable();
    Some(SharedLines {
        rank_lines,
        display,
        display_params,
        params,
    })
}

/// The varying spots between two location line-blocks (#223): each maximal
/// differing run in the line diff becomes one spot carrying both sides' ABSOLUTE
/// source-line ranges and trimmed, length-capped text — so an agent can see WHAT
/// an extracted helper would parameterize (e.g. "every spot is a data literal")
/// without opening files. Same diff the `params` count walks.
pub(crate) fn varying_spots_of(
    a: &nose_detect::Loc,
    b: &nose_detect::Loc,
    cache: &mut FileLineCache,
) -> Option<Vec<nose_detect::VaryingSpot>> {
    const SPOT_CAP: usize = 16;
    const TEXT_CAP: usize = 160;
    let la = cache.slice(&a.file, a.start_line, a.end_line)?;
    let lb = cache.slice(&b.file, b.start_line, b.end_line)?;
    let ar: Vec<&str> = la.iter().map(String::as_str).collect();
    let br: Vec<&str> = lb.iter().map(String::as_str).collect();
    let cap_text = |t: &str| {
        let t = t.trim();
        if t.len() > TEXT_CAP {
            let mut end = TEXT_CAP;
            while !t.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}…", &t[..end])
        } else {
            t.to_string()
        }
    };
    let mut spots: Vec<nose_detect::VaryingSpot> = Vec::new();
    let (mut ai, mut bi) = (0u32, 0u32);
    let mut open = false;
    for (tag, line) in line_diff(&ar, &br) {
        match tag {
            ' ' => {
                open = false;
                ai += 1;
                bi += 1;
            }
            _ => {
                if !open {
                    open = true;
                    if spots.len() >= SPOT_CAP {
                        return Some(spots);
                    }
                    spots.push(nose_detect::VaryingSpot {
                        param: spots.len() as u32 + 1,
                        a_lines: None,
                        b_lines: None,
                        a_text: String::new(),
                        b_text: String::new(),
                    });
                }
                let spot = spots.last_mut().expect("opened above");
                if tag == '-' {
                    let abs = a.start_line + ai;
                    spot.a_lines = Some(match spot.a_lines {
                        None => (abs, abs),
                        Some((s, _)) => (s, abs),
                    });
                    if !spot.a_text.is_empty() {
                        spot.a_text.push(' ');
                    }
                    if spot.a_text.len() <= TEXT_CAP {
                        spot.a_text.push_str(&cap_text(&line));
                    }
                    ai += 1;
                } else {
                    let abs = b.start_line + bi;
                    spot.b_lines = Some(match spot.b_lines {
                        None => (abs, abs),
                        Some((s, _)) => (s, abs),
                    });
                    if !spot.b_text.is_empty() {
                        spot.b_text.push(' ');
                    }
                    if spot.b_text.len() <= TEXT_CAP {
                        spot.b_text.push_str(&cap_text(&line));
                    }
                    bi += 1;
                }
            }
        }
    }
    for s in &mut spots {
        s.a_text = cap_text(&s.a_text);
        s.b_text = cap_text(&s.b_text);
    }
    Some(spots)
}

/// A line with no extractable content on its own: blank, pure delimiters (`}`, `});`,
/// `)`), or a bare control keyword. Sharing one of these between two blocks says
/// nothing about whether they're the same code.
pub(crate) fn is_trivial_line(t: &str) -> bool {
    t.is_empty()
        || t.chars().all(|c| {
            matches!(
                c,
                '{' | '}' | '(' | ')' | '[' | ']' | ';' | ',' | ' ' | '\t'
            )
        })
        || matches!(
            t,
            "return" | "break" | "continue" | "else" | "else {" | "};" | "})" | "});"
        )
}

/// How *idiomatic* (pervasive) each source line is across the analyzed corpus, by the
/// fraction of files it appears in. A line in a large fraction of files is a language
/// idiom (`if err != nil {`, a ubiquitous logging call) and earns ~0 weight; a line in
/// few files is specific and earns full weight — so a language idiom, however often it's
/// literally duplicated, can't rank as an extractable refactor, with no hardcoded
/// idiom list. The floor is generous (`LO`): ordinary cross-file duplication — the very
/// thing we want to surface — keeps full weight; only genuinely pervasive lines are
/// docked. This matters on small repos, where naive IDF would penalize everything.
pub(crate) struct LineIdf {
    df: FxHashMap<String, u32>,
    n_files: f64,
}

impl LineIdf {
    pub(crate) fn weight(&self, line: &str) -> f64 {
        if self.n_files <= 1.0 {
            return 1.0; // single-file corpus: no frequency signal
        }
        let frac = self.df.get(line).copied().unwrap_or(1) as f64 / self.n_files;
        const LO: f64 = 0.25; // ≤25% of files: specific → full weight
        const HI: f64 = 0.60; // ≥60% of files: pervasive idiom → no weight
        ((HI - frac) / (HI - LO)).clamp(0.0, 1.0)
    }
}

/// Build the [`LineIdf`] by reading every analyzed file once (through `cache`, which the
/// per-family diffs then reuse) and counting, per trimmed non-trivial line, how many
/// distinct files contain it.
pub(crate) fn corpus_line_idf(
    refs: &[&std::path::Path],
    exclude: &[String],
    cache: &mut FileLineCache,
) -> LineIdf {
    let paths = nose_frontend::discover_unique_paths(refs, exclude)
        .into_iter()
        .map(|(path, _lang)| path)
        .collect::<Vec<_>>();
    let loaded = paths
        .into_par_iter()
        .map(|path| {
            let data = std::fs::read_to_string(&path).ok().map(|text| {
                let lines = text.lines().map(str::to_string).collect::<Vec<_>>();
                let mut seen = FxHashSet::default();
                for line in &lines {
                    let t = line.trim();
                    if !is_trivial_line(t) {
                        seen.insert(t.to_string());
                    }
                }
                (lines, seen)
            });
            (path, data)
        })
        .collect::<Vec<_>>();
    let mut df: FxHashMap<String, u32> = FxHashMap::default();
    let mut n_files = 0u32;
    for (path, data) in loaded {
        match data {
            Some((lines, seen)) => {
                n_files += 1;
                for line in seen {
                    *df.entry(line).or_insert(0) += 1;
                }
                cache.0.insert(path, Some(lines));
            }
            None => {
                cache.0.insert(path, None);
            }
        }
    }
    LineIdf {
        df,
        n_files: n_files.max(1) as f64,
    }
}

pub(crate) fn cached_line_idf(
    context: &crate::cache::CachedLineContext,
    cache: &mut FileLineCache,
    force_full: bool,
) -> (
    LineIdf,
    crate::cache::LineIndexStats,
    FxHashSet<String>,
    usize,
    bool,
) {
    let (index, stats) =
        crate::cache::build_line_index(&context.run, &context.source_files, force_full);
    let mut files = index.files;
    let aliases = files
        .iter()
        .flat_map(|(path, lines)| {
            let mut aliases = Vec::new();
            if let Ok(canonical) = std::fs::canonicalize(path) {
                aliases.push((canonical.to_string_lossy().into_owned(), lines.clone()));
            }
            if let Ok(cwd) = std::env::current_dir() {
                aliases.push((crate::path_utils::relativize(path, &cwd), lines.clone()));
            }
            aliases
        })
        .collect::<Vec<_>>();
    files.extend(aliases);
    cache.0 = files;
    let file_count = index.file_count;
    let complete = index.complete;
    let changed_lines = index.changed_lines;
    let idf = LineIdf {
        df: index.document_frequency,
        n_files: file_count.max(1) as f64,
    };
    (idf, stats, changed_lines, file_count, complete)
}

/// Deterministic ranking tie-break: a family's first site `(file, start line)`.
pub(crate) fn family_anchor(f: &nose_detect::RefactorFamily) -> (&str, u32) {
    f.locations
        .first()
        .map(|l| (l.file.as_str(), l.start_line))
        .unwrap_or(("", 0))
}

/// Memoizes file contents (split into lines) so ranking many families that touch the
/// same files reads each file at most once. `None` for files that fail to read.
#[derive(Default)]
pub(crate) struct FileLineCache(pub(crate) FxHashMap<String, Option<Vec<String>>>);

impl FileLineCache {
    /// Read a set of files concurrently before a batch of family analyses.
    /// Incremental updates normally stay lazy, but a mutation can create many
    /// new families at once; serial first-touch reads then cost more than the
    /// clean path's parallel corpus preload.
    pub(crate) fn preload(&mut self, paths: Vec<String>) {
        let paths = paths
            .into_iter()
            .filter(|path| !self.0.contains_key(path))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let loaded = paths
            .into_par_iter()
            .map(|path| {
                let lines = std::fs::read_to_string(&path)
                    .ok()
                    .map(|text| text.lines().map(str::to_string).collect());
                (path, lines)
            })
            .collect::<Vec<_>>();
        self.0.extend(loaded);
    }

    /// All lines of `file`, reading and caching on first touch. `None` if unreadable.
    pub(crate) fn whole(&mut self, file: &str) -> Option<&[String]> {
        self.0
            .entry(file.to_string())
            .or_insert_with(|| {
                std::fs::read_to_string(file)
                    .ok()
                    .map(|t| t.lines().map(str::to_string).collect())
            })
            .as_deref()
    }

    /// Lines `start..=end` (1-based) of `file`.
    pub(crate) fn slice(&mut self, file: &str, start: u32, end: u32) -> Option<Vec<String>> {
        let all = self.whole(file)?;
        let (s, e) = (
            start.saturating_sub(1) as usize,
            (end as usize).min(all.len()),
        );
        (s < e).then(|| all[s..e].to_vec())
    }
}

/// Read lines `start..=end` (1-based) of `file` as raw strings.
pub(crate) fn read_lines(file: &str, start: u32, end: u32) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(file).ok()?;
    let lines: Vec<&str> = text.lines().collect();
    let (s, e) = (
        start.saturating_sub(1) as usize,
        (end as usize).min(lines.len()),
    );
    (s < e).then(|| lines[s..e].iter().map(|l| l.to_string()).collect())
}

/// Minimal LCS line diff → `(' '|'-'|'+', line)`. Caps each side so the O(n·m)
/// table stays small on large members (the differing lines are what matter).
pub(crate) fn line_diff(a: &[&str], b: &[&str]) -> Vec<(char, String)> {
    const CAP: usize = 120;
    let a = &a[..a.len().min(CAP)];
    let b = &b[..b.len().min(CAP)];
    let (n, m) = (a.len(), b.len());
    let width = m + 1;
    let mut dp = vec![0u16; (n + 1) * width];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let here = i * width + j;
            dp[here] = if a[i] == b[j] {
                dp[(i + 1) * width + j + 1] + 1
            } else {
                dp[(i + 1) * width + j].max(dp[here + 1])
            };
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            out.push((' ', a[i].to_string()));
            i += 1;
            j += 1;
        } else if dp[(i + 1) * width + j] >= dp[i * width + j + 1] {
            out.push(('-', a[i].to_string()));
            i += 1;
        } else {
            out.push(('+', b[j].to_string()));
            j += 1;
        }
    }
    out.extend(a[i..].iter().map(|l| ('-', l.to_string())));
    out.extend(b[j..].iter().map(|l| ('+', l.to_string())));
    out
}

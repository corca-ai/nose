//! Source provenance. Every IL node carries a [`Span`] so a match found in IL
//! space can be traced back to the exact original bytes/lines (sourcemap-style).

use serde::{Deserialize, Serialize};

/// Index into [`crate::Corpus::files`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct FileId(pub u32);

/// A half-open byte range plus 1-based inclusive line range in one source file.
///
/// Line numbers are 1-based to match editor/grep conventions and the prediction
/// JSON consumed by downstream tooling.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Span {
    pub file: FileId,
    pub start_byte: u32,
    pub end_byte: u32,
    pub start_line: u32,
    pub end_line: u32,
}

impl Span {
    pub fn new(
        file: FileId,
        start_byte: u32,
        end_byte: u32,
        start_line: u32,
        end_line: u32,
    ) -> Self {
        Span {
            file,
            start_byte,
            end_byte,
            start_line,
            end_line,
        }
    }

    /// A zero-width placeholder span. Used for synthetic nodes that have no
    /// natural source location (rare; desugaring normally inherits a real span).
    pub fn synthetic(file: FileId) -> Self {
        Span {
            file,
            start_byte: 0,
            end_byte: 0,
            start_line: 0,
            end_line: 0,
        }
    }

    /// Smallest span covering both `self` and `other`. Both must be in the same
    /// file; if not, `self` is returned unchanged.
    pub fn merge(self, other: Span) -> Span {
        if self.file != other.file {
            return self;
        }
        Span {
            file: self.file,
            start_byte: self.start_byte.min(other.start_byte),
            end_byte: self.end_byte.max(other.end_byte),
            start_line: self.start_line.min(other.start_line),
            end_line: self.end_line.max(other.end_line),
        }
    }

    pub fn line_count(self) -> u32 {
        self.end_line.saturating_sub(self.start_line) + 1
    }
}

/// Per-file metadata, indexed by [`FileId`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileMeta {
    /// Path as given on the command line / discovered during a walk.
    pub path: String,
    /// Source language, used by frontends and for reporting.
    pub lang: Lang,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Lang {
    Python,
    JavaScript,
    TypeScript,
    Go,
    Rust,
    Java,
    C,
    Ruby,
    /// Vue single-file component — the `<script>` block is analyzed as JS/TS.
    Vue,
    /// Svelte component — the `<script>` block is analyzed as JS/TS.
    Svelte,
    /// HTML — inline `<script>` blocks are analyzed as JS/TS.
    Html,
}

impl Lang {
    /// Canonical lowercase name (`"python"`, `"rust"`, …). Single source of truth
    /// for the per-language label used across detection, coverage, and reports.
    pub fn name(self) -> &'static str {
        match self {
            Lang::Python => "python",
            Lang::JavaScript => "javascript",
            Lang::TypeScript => "typescript",
            Lang::Go => "go",
            Lang::Rust => "rust",
            Lang::Java => "java",
            Lang::C => "c",
            Lang::Ruby => "ruby",
            Lang::Vue => "vue",
            Lang::Svelte => "svelte",
            Lang::Html => "html",
        }
    }

    /// Detect language from a file extension (no leading dot needed).
    pub fn from_extension(ext: &str) -> Option<Lang> {
        Some(match ext {
            "py" | "pyi" => Lang::Python,
            "js" | "jsx" | "mjs" | "cjs" => Lang::JavaScript,
            "ts" | "tsx" | "mts" | "cts" => Lang::TypeScript,
            "go" => Lang::Go,
            "rs" => Lang::Rust,
            "java" => Lang::Java,
            "c" | "h" => Lang::C,
            "rb" => Lang::Ruby,
            "vue" => Lang::Vue,
            "svelte" => Lang::Svelte,
            "html" | "htm" => Lang::Html,
            _ => return None,
        })
    }

    pub fn from_path(path: &str) -> Option<Lang> {
        let ext = path.rsplit('.').next()?;
        if ext == path {
            return None; // no '.' in path
        }
        Lang::from_extension(ext)
    }
}

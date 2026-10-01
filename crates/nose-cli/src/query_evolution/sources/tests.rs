use super::*;
use nose_detect::regions::evolution::FamilyObservation;
use nose_il::SourceDocument;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Files(PathBuf);

impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "nose-source-bounds-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }

    fn reader(&self) -> Sources {
        Sources {
            root: self.0.clone(),
            path_base: self.0.clone(),
            captured_base: self.0.clone(),
            remaining_bytes: MAX_TOTAL_BYTES,
            files: BTreeMap::new(),
        }
    }

    fn member(&self, file: &str, bytes: Vec<u8>, selected: usize) -> MemberObservation {
        std::fs::write(self.0.join(file), &bytes).unwrap();
        let document = SourceDocument::new(bytes);
        let source = document.region(0, selected.try_into().unwrap()).unwrap();
        MemberObservation {
            file: file.into(),
            lang: "python".into(),
            kind: "function".into(),
            name: Some("compute".into()),
            start_line: 1,
            end_line: 1,
            in_test: false,
            content_key: Some(source.content_digest),
            source: Some(source),
            analysis_key: None,
            review_key: None,
        }
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn file_budget_accepts_the_boundary_and_rejects_one_extra_byte() {
    let files = Files::new();
    for size in [MAX_FILE_BYTES - 1, MAX_FILE_BYTES, MAX_FILE_BYTES + 1] {
        let member = files.member(&format!("{size}.py"), vec![b'x'; size as usize], 1);
        let result = files.reader().text(&member);
        if size <= MAX_FILE_BYTES {
            assert_eq!(result.unwrap(), "x");
        } else {
            assert!(result.unwrap_err().to_string().contains("16 MiB"));
        }
    }
}

#[test]
fn total_budget_allows_exact_fill_and_reuses_cached_files_after_exhaustion() {
    let files = Files::new();
    let mut reader = files.reader();
    let mut cached = None;
    // Leave one byte, then exercise both sides of the remaining budget.
    for i in 0..4 {
        let size = MAX_FILE_BYTES as usize - usize::from(i == 3);
        let member = files.member(&format!("{i}.py"), vec![b'x'; size], 1);
        assert_eq!(reader.text(&member).unwrap(), "x");
        cached = Some(member);
    }
    let too_large = files.member("two.py", b"xx".to_vec(), 1);
    assert!(reader
        .text(&too_large)
        .unwrap_err()
        .to_string()
        .contains("remaining read budget"));
    let exact = files.member("one.py", b"x".to_vec(), 1);
    assert_eq!(reader.text(&exact).unwrap(), "x");
    let exhausted = files.member("extra.py", b"x".to_vec(), 1);
    assert!(reader
        .text(&exhausted)
        .unwrap_err()
        .to_string()
        .contains("exhausted"));
    assert_eq!(reader.text(&cached.unwrap()).unwrap(), "x");
    assert_eq!(reader.text(&exact).unwrap(), "x");
}

#[test]
fn region_budget_accepts_the_boundary_and_rejects_one_extra_byte() {
    let files = Files::new();
    let mut reader = files.reader();
    for size in [MAX_REGION_BYTES - 1, MAX_REGION_BYTES, MAX_REGION_BYTES + 1] {
        let member = files.member(&format!("{size}.py"), vec![b'x'; size], size);
        let result = reader.text(&member);
        if size <= MAX_REGION_BYTES {
            assert_eq!(result.unwrap(), "x".repeat(size));
        } else {
            assert!(result.unwrap_err().to_string().contains("64 KiB"));
        }
    }
}

#[test]
fn failed_verification_never_projects_source_text() {
    let files = Files::new();
    let member = files.member("a.py", b"secret".to_vec(), 6);
    let mut cases = Vec::new();
    let mut changed_buffer = member.clone();
    changed_buffer.source.as_mut().unwrap().source_digest = ContentDigest::sha256(b"old");
    cases.push((changed_buffer, "source snapshot mismatch"));
    let mut changed_region = member.clone();
    changed_region.source.as_mut().unwrap().content_digest = ContentDigest::sha256(b"wrong");
    cases.push((changed_region, "selected content digest mismatch"));
    for (start, end) in [(0, 7), (5, 2)] {
        let mut invalid = member.clone();
        let region = invalid.source.as_mut().unwrap();
        region.start_byte = start;
        region.end_byte = end;
        cases.push((invalid, "captured byte range is unavailable"));
    }
    cases.push((files.member("binary.py", vec![0xff], 1), "not UTF-8"));
    cases.push((
        files.member(
            "large.py",
            vec![b'x'; MAX_REGION_BYTES + 1],
            MAX_REGION_BYTES + 1,
        ),
        "64 KiB",
    ));
    let mut missing = member.clone();
    missing.file = "missing.py".into();
    cases.push((missing, "source file unavailable"));
    let mut no_address = member;
    no_address.source = None;
    cases.push((no_address, "captured source address unavailable"));
    for (member, reason) in cases {
        let family = FamilyObservation {
            id: ContentDigest::sha256(b"family"),
            review_key: None,
            scope: "prod".into(),
            witness: "exact-value-graph".into(),
            value_nodes: None,
            members: vec![member],
            evidence: BTreeMap::new(),
            pack_rows: Vec::new(),
            laws: Vec::new(),
            near_provenance: Vec::new(),
            exact_provenance: Vec::new(),
            abstraction_template: Vec::new(),
        };
        let view = super::super::source_view::family(&family, Some(&mut files.reader()));
        let body = &view["members"][0]["source_body"];
        assert_eq!(body["status"], "unavailable", "{reason}: {body}");
        assert!(body["reason"].as_str().unwrap().contains(reason), "{body}");
        assert!(body.get("text").is_none(), "{reason}: {body}");
    }
}

#[test]
fn verified_unicode_and_crlf_are_returned_without_rewriting() {
    let files = Files::new();
    let text = "# 한글\r\nreturn '🙂'\r\n";
    let member = files.member("unicode.py", text.as_bytes().to_vec(), text.len());
    assert_eq!(files.reader().text(&member).unwrap(), text);
}

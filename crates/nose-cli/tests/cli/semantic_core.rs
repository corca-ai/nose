use super::*;

#[test]
fn scan_mode_syntax_reports_copy_paste_only() {
    let dir = make_mode_project("syntax");
    let p = dir.to_str().unwrap();
    let out = run(&[
        "scan",
        p,
        "--mode",
        "syntax",
        "--min-size",
        "12",
        "--format",
        "json",
    ]);
    assert!(
        out.contains("copy_a.py"),
        "syntax reports exact copies: {out}"
    );
    assert!(
        out.contains("copy_b.py"),
        "syntax reports exact copies: {out}"
    );
    assert!(
        !out.contains("renamed_a.py") && !out.contains("renamed_b.py"),
        "syntax must not report semantic renamed clones: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_syntax_min_tokens_controls_copy_paste_floor() {
    let dir = make_mode_project("syntax_floor");
    let p = dir.to_str().unwrap();
    let out = run(&[
        "scan",
        p,
        "--mode",
        "syntax",
        "--min-size",
        "80",
        "--format",
        "json",
    ]);
    let json = scan_json(&out);
    assert!(
        scan_families(&json).is_empty(),
        "a high syntax token floor suppresses the short copy-paste run: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_keeps_renamed_exact_clone_candidates() {
    let dir = make_mode_project("semantic_mode");
    let p = dir.to_str().unwrap();
    let out = run(&[
        "scan",
        p,
        "--mode",
        "semantic",
        "--min-size",
        "12",
        "--format",
        "json",
    ]);
    assert!(
        out.contains("renamed_a.py") && out.contains("renamed_b.py"),
        "semantic mode keeps exact value-fingerprint candidates: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_reports_c_u16_byte_pack_only_when_byte_buffer_proven() {
    let dir = std::env::temp_dir().join(format!("nose_c_u16_pack_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("add.c"),
        "typedef unsigned char u8;\nunsigned int add_pack(const u8 *a) {\n  return (a[0] << 8) + a[1];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("or.c"),
        "unsigned int or_pack(unsigned char *a) {\n  return (a[0] << 8) | a[1];\n}\n",
    )
    .unwrap();
    fs::write(dir.join("bytes.h"), "typedef unsigned char u8;\n").unwrap();
    fs::write(
        dir.join("include_add.c"),
        "#include \"bytes.h\"\nunsigned int include_add_pack(const u8 *a) {\n  return (a[0] << 8) + a[1];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_order.c"),
        "typedef unsigned char u8;\nunsigned int wrong_order(const u8 *a) {\n  return (a[1] << 8) | a[0];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("int_pointer.c"),
        "unsigned int int_pointer(const int *a) {\n  return (a[0] << 8) | a[1];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("missing_include.c"),
        "#include \"missing.h\"\nunsigned int missing_include_pack(const u8 *a) {\n  return (a[0] << 8) | a[1];\n}\n",
    )
    .unwrap();
    fs::write(dir.join("not_bytes.h"), "typedef unsigned short u8;\n").unwrap();
    fs::write(
        dir.join("wrong_include.c"),
        "#include \"not_bytes.h\"\nunsigned int wrong_include_pack(const u8 *a) {\n  return (a[0] << 8) | a[1];\n}\n",
    )
    .unwrap();

    let semantic = scan_min_json(&dir, "semantic");
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    let family_files = |family: &serde_json::Value| -> Vec<String> {
        family["locations"]
            .as_array()
            .expect("locations")
            .iter()
            .filter_map(|loc| loc["file"].as_str())
            .map(str::to_string)
            .collect()
    };
    let positive_family = semantic_families
        .iter()
        .find(|family| {
            let files = family_files(family);
            files.iter().any(|file| file.ends_with("add.c"))
                && files.iter().any(|file| file.ends_with("or.c"))
                && files.iter().any(|file| file.ends_with("include_add.c"))
        })
        .unwrap_or_else(|| panic!("missing proven byte-buffer u16 pack family: {semantic}"));
    let positive_text = positive_family.to_string();
    assert!(
        !positive_text.contains("wrong_order.c") && !positive_text.contains("int_pointer.c"),
        "semantic mode must preserve byte order and require a byte-buffer proof: {semantic}"
    );
    assert!(
        !positive_text.contains("missing_include.c") && !positive_text.contains("wrong_include.c"),
        "semantic mode must not guess missing or non-byte include aliases: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_reports_c_u32_byte_pack_only_when_unsigned_cast_proven() {
    let dir = std::env::temp_dir().join(format!("nose_c_u32_pack_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("recover.c"),
        "typedef unsigned char u8;\ntypedef unsigned int u32;\nu32 recover_get_u32(const u8 *a) {\n  return (((u32)a[0]) << 24) + (((u32)a[1]) << 16) + (((u32)a[2]) << 8) + ((u32)a[3]);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("dbdata.c"),
        "typedef unsigned char u8;\ntypedef unsigned int u32;\nu32 dbdata_get_u32(unsigned char *a) {\n  return ((u32)a[0] << 24) | ((u32)a[1] << 16) | ((u32)a[2] << 8) | ((u32)a[3]);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("bytes.h"),
        "typedef unsigned char u8;\ntypedef unsigned int u32;\n",
    )
    .unwrap();
    fs::write(
        dir.join("include_u32.c"),
        "#include \"bytes.h\"\nu32 include_get_u32(const u8 *a) {\n  return ((u32)a[0] << 24) + ((u32)a[1] << 16) + ((u32)a[2] << 8) + ((u32)a[3] << 0);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("unsigned_int.c"),
        "unsigned int direct_get_u32(unsigned char *a) {\n  return ((unsigned int)a[0] << 24) + ((unsigned int)a[1] << 16) + ((unsigned int)a[2] << 8) + (unsigned int)a[3];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("uncasted.c"),
        "typedef unsigned char u8;\ntypedef unsigned int u32;\nu32 uncasted_get_u32(const u8 *a) {\n  return (a[0] << 24) | (a[1] << 16) | (a[2] << 8) | a[3];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_order.c"),
        "typedef unsigned char u8;\ntypedef unsigned int u32;\nu32 wrong_order(const u8 *a) {\n  return ((u32)a[1] << 24) | ((u32)a[0] << 16) | ((u32)a[2] << 8) | ((u32)a[3]);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_alias.c"),
        "typedef unsigned char u8;\ntypedef signed int u32;\nu32 wrong_alias(const u8 *a) {\n  return ((u32)a[0] << 24) | ((u32)a[1] << 16) | ((u32)a[2] << 8) | ((u32)a[3]);\n}\n",
    )
    .unwrap();

    let semantic = scan_min_json(&dir, "semantic");
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    let family_files = |family: &serde_json::Value| -> Vec<String> {
        family["locations"]
            .as_array()
            .expect("locations")
            .iter()
            .filter_map(|loc| loc["file"].as_str())
            .map(str::to_string)
            .collect()
    };
    let positive_family = semantic_families
        .iter()
        .find(|family| {
            let files = family_files(family);
            files.iter().any(|file| file.ends_with("recover.c"))
                && files.iter().any(|file| file.ends_with("dbdata.c"))
                && files.iter().any(|file| file.ends_with("include_u32.c"))
                && files.iter().any(|file| file.ends_with("unsigned_int.c"))
        })
        .unwrap_or_else(|| panic!("missing proven unsigned-cast u32 pack family: {semantic}"));
    let positive_text = positive_family.to_string();
    assert!(
        !positive_text.contains("uncasted.c")
            && !positive_text.contains("wrong_order.c")
            && !positive_text.contains("wrong_alias.c"),
        "semantic mode must require unsigned-cast, lane-order, and alias proofs: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

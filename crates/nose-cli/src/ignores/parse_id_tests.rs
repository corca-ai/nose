use super::parse_family_id;

#[test]
fn rejects_non_hex_sign_prefix() {
    // '+' + 15 hex digits = 16 chars; u64::from_str_radix would accept the '+'.
    assert!(parse_family_id("+aaaaaaaaaaaaaaa").is_err());
    assert!(parse_family_id("-aaaaaaaaaaaaaaa").is_err());
}

#[test]
fn accepts_uppercase_0x_prefix() {
    let v = parse_family_id("0XAAAAAAAAAAAAAAAA").expect("uppercase 0X prefix is valid");
    assert_eq!(v, 0xAAAA_AAAA_AAAA_AAAA);
}

#[test]
fn accepts_plain_and_lowercase_prefixed_ids() {
    assert_eq!(
        parse_family_id("aaaaaaaaaaaaaaaa").unwrap(),
        0xAAAA_AAAA_AAAA_AAAA
    );
    assert_eq!(
        parse_family_id("0xaaaaaaaaaaaaaaaa").unwrap(),
        0xAAAA_AAAA_AAAA_AAAA
    );
}

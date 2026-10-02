pub(super) fn parse_byte_size(value: &str) -> Result<u64, String> {
    let trimmed = value.trim();
    let split = trimmed
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(trimmed.len());
    let (number, suffix) = trimmed.split_at(split);
    let number = number
        .parse::<u64>()
        .map_err(|_| format!("invalid cache size `{value}`"))?;
    let multiplier = match suffix.to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "kib" => 1024,
        "mib" => 1024 * 1024,
        "gib" => 1024 * 1024 * 1024,
        "tib" => 1024_u64.pow(4),
        _ => {
            return Err(format!(
                "invalid cache size suffix in `{value}`; use B, KiB, MiB, GiB, or TiB"
            ))
        }
    };
    number
        .checked_mul(multiplier)
        .ok_or_else(|| format!("cache size `{value}` is too large"))
}

#[cfg(test)]
mod tests {
    use super::parse_byte_size;

    #[test]
    fn cache_sizes_accept_exact_binary_units_and_reject_ambiguous_suffixes() {
        assert_eq!(parse_byte_size("512").unwrap(), 512);
        assert_eq!(parse_byte_size("2GiB").unwrap(), 2 * 1024 * 1024 * 1024);
        assert!(parse_byte_size("2GB").is_err());
        assert!(parse_byte_size("18446744073709551615TiB").is_err());
    }
}

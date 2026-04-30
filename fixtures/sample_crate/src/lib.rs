pub fn normalize_name(name: &str) -> String {
    let trimmed = name.trim();
    let lowercase = trimmed.to_ascii_lowercase();
    lowercase.replace(' ', "_")
}

pub fn sum_even(values: &[i32]) -> i32 {
    values
        .iter()
        .copied()
        .filter(|value| value % 2 == 0)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::{normalize_name, sum_even};

    #[test]
    fn normalizes_names() {
        assert_eq!(normalize_name(" Ferris Crab "), "ferris_crab");
    }

    #[test]
    fn sums_even_values() {
        assert_eq!(sum_even(&[1, 2, 3, 4]), 6);
    }
}

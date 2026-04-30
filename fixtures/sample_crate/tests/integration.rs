#[test]
fn sums_even_numbers_from_integration_test() {
    let values = [2, 5, 8];
    assert_eq!(sample_crate::sum_even(&values), 10);
}

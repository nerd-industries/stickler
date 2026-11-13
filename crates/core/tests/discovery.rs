use nelko_core::discovery::is_p21_name;

#[test]
fn matches_p21_names() {
    assert!(is_p21_name("P21"));
    assert!(is_p21_name("p21_A3F2"));
    assert!(!is_p21_name("P31"));
    assert!(!is_p21_name("AirPods"));
}

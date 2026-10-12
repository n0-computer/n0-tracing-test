//! Checks that an integration test captures the logs of the library crate of its package.

use n0_tracing_test::traced_test;

#[traced_test]
#[test]
fn captures_logs_of_the_library_crate() {
    tracing::info!("logged from the test");
    n0_tracing_test_macro_tests::log_from_library();
    assert!(logs_contain("logged from the test"));
    assert!(logs_contain("logged from the library"));
}

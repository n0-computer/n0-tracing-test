//! Helper functions and macros that allow for easier testing of crates that use `tracing`.
//!
//! The focus is on testing the logging, not on debugging the tests. That's why the
//! library ensures that the logs do not depend on external state. For example, the
//! `RUST_LOG` env variable is not used for log filtering.
//!
//! Similar crates:
//!
//! - [test-log](https://crates.io/crates/test-log): Initialize loggers before
//!   running tests
//! - [tracing-fluent-assertions](https://crates.io/crates/tracing-fluent-assertions):
//!   More powerful assertions that also allow analyzing spans
//!
//! ## Usage
//!
//! This crate should mainly be used through the
//! [`#[traced_test]`](attr.traced_test.html) macro.
//!
//! First, add a dependency on `n0-tracing-test` in `Cargo.toml`.
//! Then, annotate your test function with the `#[traced_test]` macro.
//!
//! ```rust
//! use tracing::{info, warn};
//! use n0_tracing_test::traced_test;
//!
//! #[tokio::test]
//! #[traced_test]
//! async fn test_logs_are_captured() {
//!     // Local log
//!     info!("This is being logged on the info level");
//!
//!     // Log from a spawned task (which runs in a separate thread)
//!     tokio::spawn(async {
//!         warn!("This is being logged on the warn level from a spawned task");
//!     })
//!     .await
//!     .unwrap();
//!
//!     // Ensure that certain strings are or aren't logged
//!     assert!(logs_contain("logged on the info level"));
//!     assert!(logs_contain("logged on the warn level"));
//!     assert!(!logs_contain("logged on the error level"));
//!
//!     // Ensure that the string `logged` is logged exactly twice
//!     logs_assert(|lines: &[&str]| {
//!         match lines.iter().filter(|line| line.contains("logged")).count() {
//!             2 => Ok(()),
//!             n => Err(format!("Expected two matching logs, but found {}", n)),
//!         }
//!     });
//! }
//! ```
//!
//! Done! You can write assertions using one of two injected functions:
//!
//! - `logs_contain(&str) -> bool`: Use this within an `assert!` call to ensure
//!   that a certain string is (or isn't) logged anywhere in the logs.
//! - `logs_assert(f: impl Fn(&[&str]) -> Result<(), String>)`:  Run a function
//!   against the log lines. If the function returns an `Err`, panic. This can
//!   be used to run arbitrary assertion logic against the logs.
//!
//! The [`assert_logs!`], [`assert_logs_ordered!`] and [`assert_no_logs!`] macros
//! assert several values at once, and name the value that fails the assertion.
//!
//! Logs are written to stdout, so they are captured by the cargo test runner
//! by default, but printed if the test fails.
//!
//! Of course, you can also annotate regular non-async tests:
//!
//! ```rust
//! use tracing::info;
//! use n0_tracing_test::traced_test;
//!
//! #[traced_test]
//! #[test]
//! fn plain_old_test() {
//!     assert!(!logs_contain("Logging from a non-async test"));
//!     info!("Logging from a non-async test");
//!     assert!(logs_contain("Logging from a non-async test"));
//!     assert!(!logs_contain("This was never logged"));
//! }
//! ```
//!
//! ## Rationale / Why You Need This
//!
//! Tracing allows you to set a default subscriber within a scope:
//!
//! ```rust
//! # let subscriber = tracing::Dispatch::new(tracing_subscriber::FmtSubscriber::new());
//! # let req = 123;
//! # fn get_response(fake_req: u8) {}
//! let response = tracing::dispatcher::with_default(&subscriber, || get_response(req));
//! ```
//!
//! This works fine, as long as no threads are involved. As soon as you use a
//! multi-threaded test runtime (e.g. the `#[tokio::test]` with the
//! `rt-multi-thread` feature) and spawn tasks, the tracing logs in those tasks
//! will not be captured by the subscriber.
//!
//! The macro provided in this crate registers a global default subscriber instead.
//! This subscriber contains a writer which logs into a global static in-memory buffer.
//!
//! At the beginning of every test, the macro injects span opening code. The span
//! uses the name of the test function (unless it's already taken, then a counter
//! is appended). This means that the logs from a test are prefixed with the test
//! name, which helps when debugging.
//!
//! Finally, a function called `logs_contain(value: &str)` is injected into every
//! annotated test. It filters the logs in the buffer to include only lines
//! containing ` {span_name}: ` and then searches the value in the matching log
//! lines. This can be used to assert that a message was logged during a test.
//!
//! ## Per-crate Filtering
//!
//! By default, `tracing-test` sets an env filter that filters out all logs
//! except the ones from your crate (equivalent to
//! `RUST_LOG=<your_crate>=trace`).
//!
//! Rust "integration tests" (in the `tests/` directory) are each built into a
//! separate crate from the crate they test. In an integration test, the filter
//! also lets through the library crate of the package, which is named after
//! the package with `-` replaced by `_` (equivalent to
//! `RUST_LOG=<test_crate>=trace,<your_crate>=trace`). If the library has a
//! different name, use `no-env-filter`.
//!
//! If you need to capture logs from other crates
//! as well, you can turn off this log filtering globally by enabling the
//! `no-env-filter` Cargo feature:
//!
//! ```toml
//! tracing-test = { version = "0.1", features = ["no-env-filter"] }
//! ```
//!
//! Note that this will result in _all_ logs from _all_ your dependencies being
//! captured! This means that the `logs_contain` function may become less
//! useful, and you might need to use `logs_assert` instead, with your own
//! custom filtering logic.
//!
//! ## Controlling log printing
//!
//! To suppress all log output, even when tests are run with `--nocapture`,
//! enable `no-log-printing` feature.
//!
//! To enable color output and support for filtering with `RUST_LOG` in the
//! terminal output, enable the `pretty-log-printing` feature.

pub mod internal;
mod subscriber;

pub use n0_tracing_test_macro::traced_test;

/// Asserts that the logs of a test contain each of the given values.
///
/// Use this in a test with [`#[traced_test]`](traced_test). Each value can be
/// anything that dereferences to `str`, such as `&str` or `String`. If a value
/// is missing from the logs, the macro panics with that value.
///
/// ```rust
/// use n0_tracing_test::{assert_logs, traced_test};
/// use tracing::info;
///
/// #[traced_test]
/// fn logs_requests() {
///     for id in 0..2 {
///         info!(id, "request sent");
///     }
///     assert_logs!("request sent id=0", format!("request sent id={}", 1));
/// }
/// # logs_requests();
/// ```
#[macro_export]
macro_rules! assert_logs {
    ($($value:expr),* $(,)?) => {{
        $(
            let value = $value;
            assert!(logs_contain(&value), "logs do not contain: {}", &*value);
        )*
    }};
}

/// Asserts that the logs of a test contain the given values in this order.
///
/// Each value must be on a later line than the value before it. Use this in a
/// test with [`#[traced_test]`](traced_test). If a value is missing after the
/// value before it, the macro panics with that value.
///
/// ```rust
/// use n0_tracing_test::{assert_logs_ordered, traced_test};
/// use tracing::info;
///
/// #[traced_test]
/// fn logs_in_order() {
///     info!("request sent");
///     info!("response received");
///     assert_logs_ordered!("request sent", "response received");
/// }
/// # logs_in_order();
/// ```
#[macro_export]
macro_rules! assert_logs_ordered {
    ($($value:expr),* $(,)?) => {{
        let values = [$(::std::string::ToString::to_string(&$value)),*];
        logs_assert(|lines: &[&str]| {
            let mut rest = lines;
            for value in &values {
                match rest.iter().position(|line| line.contains(value.as_str())) {
                    Some(i) => rest = &rest[i + 1..],
                    None => return Err(format!("logs do not contain in order: {}", value)),
                }
            }
            Ok(())
        });
    }};
}

/// Asserts that the logs of a test contain none of the given values.
///
/// Use this in a test with [`#[traced_test]`](traced_test). If the logs
/// contain a value, the macro panics with that value.
///
/// ```rust
/// use n0_tracing_test::{assert_no_logs, traced_test};
/// use tracing::info;
///
/// #[traced_test]
/// fn logs_no_errors() {
///     info!("request sent");
///     assert_no_logs!("ERROR", "request failed");
/// }
/// # logs_no_errors();
/// ```
#[macro_export]
macro_rules! assert_no_logs {
    ($($value:expr),* $(,)?) => {{
        $(
            let value = $value;
            assert!(!logs_contain(&value), "logs contain: {}", &*value);
        )*
    }};
}

use std::sync::Once;

use tracing::{Level};
use tracing_subscriber::{fmt::format, prelude::*};

static SETUP_LOGGING: Once = Once::new();

/// Initialize the logging & tracing systems.
pub(crate) fn setup_logger() -> () {
    // Ensure this function is only ever called once, otherwise things can get very ugly.
    SETUP_LOGGING.call_once(|| _setup_logger())
}

/// underlying implementation that sets up the logger & tracer
fn _setup_logger() -> () {
    // Format fields using the provided closure.
    let format = format::debug_fn(|writer, field, value| {
        // We'll format the field name and value separated with a colon.
        write!(writer, "{}: {:?}", field, value)
    })
        // Separate each field with a comma.
        // This method is provided by an extension trait in the
        // `tracing-subscriber` prelude.
        .delimited(", ");

    // Create a `fmt` subscriber that uses our custom event format, and set it
    // as the default.
    tracing_subscriber::fmt()
        .fmt_fields(format)
        .with_max_level(Level::DEBUG)
        .init();
}

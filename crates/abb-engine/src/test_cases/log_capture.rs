//! The one logger the engine's tests install. Records from every test in the
//! process land here, so an assertion filters on an id its own test created.
//! Holds no tests.

use std::sync::{Mutex, Once};

struct CapturingLogger {
    records: Mutex<Vec<(log::Level, String)>>,
}

impl log::Log for CapturingLogger {
    fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        self.records
            .lock()
            .expect("capturing logger lock")
            .push((record.level(), record.args().to_string()));
    }

    fn flush(&self) {}
}

static CAPTURING_LOGGER: CapturingLogger = CapturingLogger {
    records: Mutex::new(Vec::new()),
};

pub(crate) fn install() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        log::set_logger(&CAPTURING_LOGGER).expect("install capturing logger");
        log::set_max_level(log::LevelFilter::Info);
    });
}

pub(crate) fn records() -> Vec<String> {
    records_at_or_above(log::Level::Trace)
}

pub(crate) fn error_records() -> Vec<String> {
    records_at_or_above(log::Level::Error)
}

fn records_at_or_above(level: log::Level) -> Vec<String> {
    CAPTURING_LOGGER
        .records
        .lock()
        .expect("capturing logger lock")
        .iter()
        .filter(|(record_level, _)| *record_level <= level)
        .map(|(_, message)| message.clone())
        .collect()
}

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

pub fn install() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        log::set_logger(&CAPTURING_LOGGER).expect("install capturing logger");
        log::set_max_level(log::LevelFilter::Info);
    });
}

pub fn records() -> Vec<String> {
    records_at_or_above(log::Level::Trace)
}

pub fn error_records() -> Vec<String> {
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

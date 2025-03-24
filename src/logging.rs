use lazy_static::lazy_static;
use log;
use std::sync::{Arc, Mutex, MutexGuard};

lazy_static! {
    /// This is an example for using doc comment attributes
    static ref LOGGER: SimpleLogger = SimpleLogger{
        data: Mutex::new(vec![]).into(),
    };
}

struct SimpleLogger {
    data: Arc<Mutex<Vec<LogLine>>>,
}

pub struct LogLine {
    pub level: log::Level,
    pub target: String,
    pub text: String,
}

pub fn get_lines<'a>() -> MutexGuard<'a, Vec<LogLine>> {
    (*LOGGER).data.lock().unwrap()
}

pub fn setup_logger() {
    log::set_logger(&(*LOGGER)).expect("cant log");
    log::set_max_level(log::LevelFilter::Info);
}

impl log::Log for SimpleLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        self.data.lock().unwrap().push(LogLine {
            level: record.level(),
            target: record.target().to_string(),
            text: record.args().to_string(),
        });
    }

    fn flush(&self) {}
}

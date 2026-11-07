use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const CURRENT_LOG: &str = "abb.log";
pub const PREVIOUS_LOG: &str = "abb.previous.log";
pub const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

pub fn log_dir(
    identifier: &str,
    home: Option<PathBuf>,
    xdg_data_home: Option<PathBuf>,
) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        return home.map(|home| home.join("Library").join("Logs").join(identifier));
    }
    let data_home = xdg_data_home
        .filter(|path| path.is_absolute())
        .or_else(|| home.map(|home| home.join(".local").join("share")))?;
    Some(data_home.join(identifier).join("logs"))
}

pub fn default_log_dir(identifier: &str) -> Option<PathBuf> {
    log_dir(
        identifier,
        std::env::var_os("HOME").map(PathBuf::from),
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
    )
}

#[expect(
    clippy::disallowed_methods,
    reason = "the previous log is disposable; a crash mid-rename loses only a log"
)]
pub fn open_log(dir: &Path) -> io::Result<File> {
    fs::create_dir_all(dir)?;
    let current = dir.join(CURRENT_LOG);
    if fs::metadata(&current).is_ok_and(|log| log.len() > MAX_LOG_BYTES) {
        if let Err(error) = fs::rename(&current, dir.join(PREVIOUS_LOG)) {
            #[expect(
                clippy::let_underscore_must_use,
                reason = "the logger does not exist yet and stderr is the only place to say so"
            )]
            let _ = writeln!(
                io::stderr(),
                "Log rotation failed, so ABB keeps adding to the current log: {error}"
            );
        }
    }
    OpenOptions::new().create(true).append(true).open(current)
}

pub struct StderrAndFile {
    file: File,
}

impl StderrAndFile {
    pub fn new(file: File) -> Self {
        Self { file }
    }
}

impl Write for StderrAndFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        #[expect(
            clippy::let_underscore_must_use,
            reason = "stderr is a convenience copy; the log file write is the one reported"
        )]
        let _ = io::stderr().write_all(buf);
        self.file.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        #[expect(
            clippy::let_underscore_must_use,
            reason = "stderr is a convenience copy; the log file flush is the one reported"
        )]
        let _ = io::stderr().flush();
        self.file.flush()
    }
}

pub fn init_logger(identifier: &str) {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    let open_error = match default_log_dir(identifier)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home folder"))
        .and_then(|dir| open_log(&dir))
    {
        Ok(file) => {
            builder.target(env_logger::Target::Pipe(Box::new(StderrAndFile::new(file))));
            None
        }
        Err(error) => Some(error),
    };
    builder.init();
    if let Some(error) = open_error {
        log::warn!("Log file unavailable, logging to stderr only: {error}");
    }
    let print_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        log::error!("{panic}");
        print_panic(panic);
    }));
}

#[cfg(test)]
#[path = "log_file_tests.rs"]
mod tests;

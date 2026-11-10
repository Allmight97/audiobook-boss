use std::ffi::{c_char, c_int, CStr};
use std::fmt;

use ffmpeg_sys_next as sys;

pub type Result<T> = std::result::Result<T, Error>;

/// The layer operation that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    OpenInput,
    ReadStreamInfo,
    CreateOutput,
    OpenOutputFile,
    AddStream,
    CopyStreamParameters,
    AddChapter,
    SetTags,
    WriteHeader,
    ReadPacket,
    WritePacket,
    WriteTrailer,
    CloseOutput,
    AllocatePacket,
}

impl Step {
    fn describe(self) -> &'static str {
        match self {
            Self::OpenInput => "open the input",
            Self::ReadStreamInfo => "read the input's streams",
            Self::CreateOutput => "create the output",
            Self::OpenOutputFile => "open the output file",
            Self::AddStream => "add an output stream",
            Self::CopyStreamParameters => "copy stream parameters",
            Self::AddChapter => "add a chapter",
            Self::SetTags => "set tags",
            Self::WriteHeader => "write the output header",
            Self::ReadPacket => "read a packet",
            Self::WritePacket => "write a packet",
            Self::WriteTrailer => "write the output trailer",
            Self::CloseOutput => "close the output file",
            Self::AllocatePacket => "allocate a packet",
        }
    }
}

/// A layer failure. Its text never holds a path or an option value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Ffmpeg { step: Step, code: c_int },
    OutOfMemory { step: Step },
    NulInText { step: Step },
    MissingStream { step: Step, index: usize },
    EmptyPacket,
    ChapterEndsBeforeStart,
}

impl Error {
    pub(crate) fn check(step: Step, code: c_int) -> Result<c_int> {
        if code < 0 {
            Err(Self::Ffmpeg { step, code })
        } else {
            Ok(code)
        }
    }

    pub fn is_end_of_file(&self) -> bool {
        matches!(self, Self::Ffmpeg { code, .. } if *code == sys::AVERROR_EOF)
    }
}

fn ffmpeg_text(code: c_int) -> String {
    let mut buffer = [0 as c_char; 128];
    // SAFETY: `buffer` is writable for its full length, and av_strerror always
    // NUL-terminates within the size it is given.
    let written = unsafe { sys::av_strerror(code, buffer.as_mut_ptr(), buffer.len()) };
    if written < 0 {
        return format!("FFmpeg error {code}");
    }
    // SAFETY: av_strerror wrote a NUL-terminated string into `buffer`.
    unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ffmpeg { step, code } => {
                write!(
                    f,
                    "FFmpeg could not {}: {}",
                    step.describe(),
                    ffmpeg_text(*code)
                )
            }
            Self::OutOfMemory { step } => write!(f, "out of memory to {}", step.describe()),
            Self::NulInText { step } => {
                write!(
                    f,
                    "cannot {}: a text value holds a NUL byte",
                    step.describe()
                )
            }
            Self::MissingStream { step, index } => {
                write!(f, "cannot {}: no stream {index}", step.describe())
            }
            Self::EmptyPacket => f.write_str("refused to write a packet with no data"),
            Self::ChapterEndsBeforeStart => f.write_str("a chapter ends before it starts"),
        }
    }
}

impl std::error::Error for Error {}

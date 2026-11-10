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
    AllocateFrame,
    OpenDecoder,
    Decode,
    OpenEncoder,
    Encode,
    ReadEncoderOption,
    CreateResampler,
    Resample,
    AddSideData,
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
            Self::AllocateFrame => "allocate an audio frame",
            Self::OpenDecoder => "open the decoder",
            Self::Decode => "decode audio",
            Self::OpenEncoder => "open the encoder",
            Self::Encode => "encode audio",
            Self::ReadEncoderOption => "read an encoder option",
            Self::CreateResampler => "create the resampler",
            Self::Resample => "resample audio",
            Self::AddSideData => "add packet side data",
        }
    }
}

/// A layer failure. Its text never holds a path or an option value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Ffmpeg {
        step: Step,
        code: c_int,
    },
    OutOfMemory {
        step: Step,
    },
    NulInText {
        step: Step,
    },
    MissingStream {
        step: Step,
        index: usize,
    },
    EmptyPacket,
    ChapterEndsBeforeStart,
    CodecNotFound {
        step: Step,
    },
    NotAudio {
        step: Step,
    },
    UnsupportedSampleFormat {
        step: Step,
    },
    UnsupportedChannelOrder {
        step: Step,
    },
    OutOfRange {
        step: Step,
    },
    WrongFrameShape,
    /// Option names only; values are never listed.
    OptionsNotTaken {
        step: Step,
        names: Vec<String>,
    },
}

impl Error {
    pub(crate) fn check(step: Step, code: c_int) -> Result<c_int> {
        if code < 0 {
            Err(Self::Ffmpeg { step, code })
        } else {
            Ok(code)
        }
    }

    /// The operating-system error number FFmpeg reported, or `None` for an
    /// error of FFmpeg's own, such as invalid data.
    pub fn os_error(&self) -> Option<c_int> {
        match self {
            Self::Ffmpeg { code, .. } if (-4095..0).contains(code) => Some(-code),
            _ => None,
        }
    }

    pub fn is_invalid_data(&self) -> bool {
        matches!(self, Self::Ffmpeg { code, .. } if *code == sys::AVERROR_INVALIDDATA)
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
                write!(f, "cannot {}: {}", step.describe(), ffmpeg_text(*code))
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
            Self::CodecNotFound { step } => {
                write!(
                    f,
                    "cannot {}: this FFmpeg build lacks the codec",
                    step.describe()
                )
            }
            Self::NotAudio { step } => {
                write!(f, "cannot {}: the stream is not audio", step.describe())
            }
            Self::UnsupportedSampleFormat { step } => {
                write!(f, "cannot {}: unsupported sample format", step.describe())
            }
            Self::UnsupportedChannelOrder { step } => {
                write!(f, "cannot {}: unsupported channel order", step.describe())
            }
            Self::OutOfRange { step } => {
                write!(f, "cannot {}: a value is out of range", step.describe())
            }
            Self::WrongFrameShape => f.write_str("the audio frame lacks the requested plane"),
            Self::OptionsNotTaken { step, names } => write!(
                f,
                "cannot {}: FFmpeg did not take the options {}",
                step.describe(),
                names.join(", ")
            ),
        }
    }
}

impl std::error::Error for Error {}

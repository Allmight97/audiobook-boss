mod context;
mod faac;
mod options;
mod session;
mod write;

pub(crate) use context::setup_encoder;
pub(crate) use session::EncoderSession;

/// Preflight uses the same library resolution/readback as the encoding session.
pub(super) fn validate_faac_configuration(
    settings: &crate::audio::EncoderSettings,
    rate: u32,
    channels: u32,
) -> crate::errors::Result<()> {
    faac::FaacEncoder::open(rate, channels as i32, settings).map(|_| ())
}

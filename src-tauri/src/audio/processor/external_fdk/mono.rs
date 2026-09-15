//! Explicitly declare that ABB's mono HE-AAC output has no parametric stereo.
//!
//! FDK omits the PS-present flag; FFmpeg then assumes stereo for a mono SBR
//! core. Correct only our freshly encoded output, before metadata finalization.

use crate::errors::{AppError, Result};
use crate::processing::ProcessingContext;
use ffmpeg_next as ff;
use std::path::Path;

pub(super) fn declare_mono(
    source: &Path,
    destination: &Path,
    context: &ProcessingContext,
) -> Result<()> {
    let mut input = ff::format::input(source)?;
    if input.nb_streams() != 1 {
        return Err(AppError::General(
            "FDK worker output must contain one audio stream.".into(),
        ));
    }
    let stream = input
        .stream(0)
        .ok_or_else(|| AppError::General("FDK output has no audio stream.".into()))?;
    let mut parameters = stream.parameters().clone();
    set_mono_configuration(&mut parameters)?;
    let input_time_base = stream.time_base();
    let mut output = ff::format::output(destination)?;
    let mut audio = output.add_stream(ff::encoder::find(ff::codec::Id::AAC))?;
    audio.set_parameters(parameters);
    audio.set_time_base(input_time_base);
    output.write_header()?;
    let output_time_base = output.stream(0).expect("created audio stream").time_base();
    loop {
        if context.is_cancelled() {
            return Err(AppError::cancelled());
        }
        let mut packet = ff::Packet::empty();
        match packet.read(&mut input) {
            Ok(()) => {
                packet.set_position(-1);
                packet.rescale_ts(input_time_base, output_time_base);
                packet.write_interleaved(&mut output)?;
            }
            Err(ff::Error::Eof) => break,
            Err(error) => return Err(error.into()),
        }
    }
    output.write_trailer()?;
    Ok(())
}

fn set_mono_configuration(parameters: &mut ff::codec::Parameters) -> Result<()> {
    // SAFETY: parameters owns this mutable codec-parameter allocation. The copied
    // configuration is validated before replacing its FFmpeg-owned extradata.
    unsafe {
        let raw = parameters.as_mut_ptr();
        if (*raw).codec_id != ff::sys::AVCodecID::AV_CODEC_ID_AAC
            || (*raw).extradata_size != 5
            || (*raw).extradata.is_null()
        {
            return Err(AppError::General(
                "FDK mono output has an unexpected AAC configuration.".into(),
            ));
        }
        let asc = std::slice::from_raw_parts((*raw).extradata, 5);
        let corrected = explicit_mono_asc(asc)?;
        let data =
            ff::sys::av_mallocz(corrected.len() + ff::sys::AV_INPUT_BUFFER_PADDING_SIZE as usize)
                .cast::<u8>();
        if data.is_null() {
            return Err(AppError::General(
                "Cannot allocate FDK mono configuration.".into(),
            ));
        }
        std::ptr::copy_nonoverlapping(corrected.as_ptr(), data, corrected.len());
        ff::sys::av_free((*raw).extradata.cast());
        (*raw).extradata = data;
        (*raw).extradata_size = corrected.len() as i32;
        ff::sys::av_channel_layout_uninit(&mut (*raw).ch_layout);
        ff::sys::av_channel_layout_default(&mut (*raw).ch_layout, 1);
        (*raw).codec_tag = 0;
    }
    Ok(())
}

fn explicit_mono_asc(asc: &[u8]) -> Result<[u8; 7]> {
    // FDK's explicit_sbr ASC: LC core + indexed rate + mono + GA flags,
    // followed by syncExtensionType 0x2b7, AOT 5, SBR-present and output rate.
    // The final three bits are byte padding, not part of the configuration.
    if asc.len() != 5
        || asc[0] >> 3 != 2
        || asc[1] & 0x7f != 0x08
        || asc[2] != 0x56
        || asc[3] != 0xe5
        || asc[4] & 0x80 == 0
    {
        return Err(AppError::General(
            "FDK mono output did not use explicit SBR signaling.".into(),
        ));
    }
    let mut result = [0u8; 7];
    result[..5].copy_from_slice(asc);
    // Append syncExtensionType 0x548 and psPresentFlag=0 at bit offset 37.
    result[4] = (asc[4] & 0xf8) | 0x05;
    result[5] = 0x48;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::explicit_mono_asc;

    #[test]
    fn fdk_mono_configuration_declares_ps_absent() {
        // Captured 44.1 kHz mono FDK HE configuration with explicit_sbr.
        assert_eq!(
            explicit_mono_asc(&[0x13, 0x88, 0x56, 0xe5, 0xa0]).expect("captured FDK mono ASC"),
            [0x13, 0x88, 0x56, 0xe5, 0xa5, 0x48, 0x00]
        );
        assert!(explicit_mono_asc(&[0x2b, 0x8a, 0x08, 0x00]).is_err());
        assert!(explicit_mono_asc(&[0x13, 0x90, 0x56, 0xe5, 0xa0]).is_err());
    }
}

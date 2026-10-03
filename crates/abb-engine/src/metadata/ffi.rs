use crate::errors::Result;
use ffmpeg_next as ff;

/// Sets the ATTACHED_PIC disposition on a stream using FFI
///
/// This uses unsafe FFI to access the underlying AVStream and set the disposition
/// flag directly, which is necessary because ffmpeg-next doesn't expose this functionality.
pub(crate) fn set_attached_pic_disposition(
    octx: &mut ff::format::context::Output,
    stream_index: usize,
) -> Result<()> {
    use crate::errors::AppError;

    unsafe {
        // Get the format context
        let format_ctx = octx.as_mut_ptr();
        if format_ctx.is_null() {
            return Err(AppError::General("Invalid format context".to_string()));
        }

        // Access the streams array
        let streams_ptr = (*format_ctx).streams;
        if streams_ptr.is_null() || stream_index >= (*format_ctx).nb_streams as usize {
            return Err(AppError::General("Invalid stream index".to_string()));
        }

        // Get the specific stream
        let stream_ptr = *streams_ptr.add(stream_index);
        if stream_ptr.is_null() {
            return Err(AppError::General("Invalid stream pointer".to_string()));
        }

        // Set the ATTACHED_PIC disposition without clobbering existing flags
        (*stream_ptr).disposition |= ff::format::stream::Disposition::ATTACHED_PIC.bits();

        log::debug!("Set ATTACHED_PIC disposition on stream {}", stream_index);
        Ok(())
    }
}

pub(crate) fn set_stream_disposition_and_clear_codec_tag(
    ostream: &mut ff::format::stream::StreamMut,
    disposition: ff::format::stream::Disposition,
) {
    unsafe {
        let ptr = ostream.as_mut_ptr();
        (*ptr).disposition = disposition.bits();
        if !(*ptr).codecpar.is_null() {
            (*(*ptr).codecpar).codec_tag = 0;
        }
    }
}

use std::ffi::{c_int, CString};
use std::path::{Path, PathBuf};
use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use crate::codec::CodecId;
use crate::context::{self, Chapter, Disposition, Rational, StreamFacts};
use crate::encoder::AudioEncoder;
use crate::error::{Error, Result, Step};
use crate::input::{path_text, Input};
use crate::packet::Packet;
use crate::text::{Dictionary, Tags, Text};

/// An AAC stream whose packets come from outside FFmpeg's encoders.
pub struct AacStream<'a> {
    pub high_efficiency: bool,
    pub rate: u32,
    pub channels: u16,
    pub bit_rate: i64,
    pub frame_samples: u32,
    /// Samples the decoder drops before the first audible one.
    pub priming_samples: u32,
    /// The AudioSpecificConfig.
    pub config: &'a [u8],
}

/// The image format of an attached picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureKind {
    Jpeg,
    Png,
}

/// The output file the layer created. Until `keep`, dropping it closes the
/// file, frees the context, and removes the file.
struct OutputFile {
    ctx: NonNull<sys::AVFormatContext>,
    path: PathBuf,
    kept: bool,
}

impl OutputFile {
    fn close(&mut self) -> c_int {
        // SAFETY: `ctx` is live; avio_closep tolerates a null `pb` and nulls it.
        unsafe { sys::avio_closep(&mut (*self.ctx.as_ptr()).pb) }
    }
}

impl Drop for OutputFile {
    fn drop(&mut self) {
        self.close();
        // SAFETY: this value owns the context and nothing uses it after this.
        unsafe { sys::avformat_free_context(self.ctx.as_ptr()) };
        if !self.kept {
            #[expect(
                clippy::let_underscore_must_use,
                reason = "a caller that sees the failure removes the file again and logs it"
            )]
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// An output file before its header: streams, chapters, and tags are set here.
pub struct Output(OutputFile);

/// An output file after its header: packets are written here, then `finish`.
pub struct Muxer(OutputFile);

impl Output {
    pub fn create(path: &Path, muxer: &str) -> Result<Self> {
        let muxer = CString::new(muxer).map_err(|_| Error::NulInText {
            step: Step::CreateOutput,
        })?;
        Self::create_with(path, Some(&muxer))
    }

    fn create_with(path: &Path, muxer: Option<&CString>) -> Result<Self> {
        let url = path_text(path, Step::CreateOutput)?;
        let muxer = muxer.map_or(ptr::null(), |muxer| muxer.as_ptr());
        let mut ctx = ptr::null_mut();
        // SAFETY: `url` is NUL-terminated and `muxer` is null or
        // NUL-terminated; FFmpeg allocates `ctx`.
        let code = unsafe {
            sys::avformat_alloc_output_context2(&mut ctx, ptr::null(), muxer, url.as_ptr())
        };
        Error::check(Step::CreateOutput, code)?;
        let ctx = NonNull::new(ctx).ok_or(Error::OutOfMemory {
            step: Step::CreateOutput,
        })?;
        let mut file = OutputFile {
            ctx,
            path: path.to_path_buf(),
            kept: false,
        };
        // SAFETY: `ctx` is live and its `pb` is null until this opens it.
        let code =
            unsafe { sys::avio_open(&mut (*ctx.as_ptr()).pb, url.as_ptr(), sys::AVIO_FLAG_WRITE) };
        if code < 0 {
            file.kept = true;
        }
        Error::check(Step::OpenOutputFile, code)?;
        Ok(Self(file))
    }

    /// Creates the file with the muxer FFmpeg picks for the file name.
    pub fn create_for_file_name(path: &Path) -> Result<Self> {
        Self::create_with(path, None)
    }

    pub fn muxer_name(&self) -> String {
        // SAFETY: an output context always has an output format with a name.
        unsafe { Text::copied_from((*(*self.ctx()).oformat).name) }.display()
    }

    pub fn streams(&self) -> Vec<StreamFacts> {
        // SAFETY: `self` owns a live context.
        unsafe { context::stream_facts(self.ctx()) }
    }

    pub fn chapters(&self) -> Vec<Chapter> {
        // SAFETY: `self` owns a live context.
        unsafe { context::chapters(self.ctx()) }
    }

    /// Adds a stream with the input stream's codec parameters, as a stream
    /// copy needs them, and returns the new stream's index.
    pub fn add_stream_copied_from(&mut self, input: &Input, input_index: usize) -> Result<usize> {
        // SAFETY: `input` owns a live context.
        let source = unsafe { context::stream(input.as_ptr(), input_index, Step::AddStream)? };
        let stream = self.new_stream()?;
        // SAFETY: both streams are live and own their `codecpar`.
        let code = unsafe { sys::avcodec_parameters_copy((*stream).codecpar, (*source).codecpar) };
        Error::check(Step::CopyStreamParameters, code)?;
        Ok(self.last_stream_index())
    }

    /// Whether the muxer keeps codec setup data in its header, which an
    /// encoder must then be opened for.
    pub fn wants_global_header(&self) -> bool {
        // SAFETY: an output context always has an output format.
        let flags = unsafe { (*(*self.ctx()).oformat).flags };
        flags & sys::AVFMT_GLOBALHEADER != 0
    }

    /// Adds an audio stream with the opened encoder's parameters and returns
    /// its index.
    pub fn add_stream_for_encoder(&mut self, encoder: &AudioEncoder) -> Result<usize> {
        let stream = self.new_stream()?;
        // SAFETY: the new stream owns its `codecpar`; the encoder is live.
        let code =
            unsafe { sys::avcodec_parameters_from_context((*stream).codecpar, encoder.as_ptr()) };
        Error::check(Step::CopyStreamParameters, code)?;
        Ok(self.last_stream_index())
    }

    /// Adds the stream of one attached picture, ticking once a second, and
    /// returns its index.
    pub fn add_attached_picture_stream(
        &mut self,
        kind: PictureKind,
        width: u32,
        height: u32,
    ) -> Result<usize> {
        let step = Step::AddStream;
        let size = |value: u32| c_int::try_from(value).map_err(|_| Error::OutOfRange { step });
        let (width, height) = (size(width)?, size(height)?);
        let stream = self.new_stream()?;
        // SAFETY: the new stream belongs to the live context `self` owns and
        // owns its `codecpar`.
        unsafe {
            let parameters = (*stream).codecpar;
            (*parameters).codec_type = sys::AVMediaType::AVMEDIA_TYPE_VIDEO;
            (*parameters).codec_id = match kind {
                PictureKind::Jpeg => sys::AVCodecID::AV_CODEC_ID_MJPEG,
                PictureKind::Png => sys::AVCodecID::AV_CODEC_ID_PNG,
            };
            (*parameters).format = match kind {
                PictureKind::Jpeg => sys::AVPixelFormat::AV_PIX_FMT_YUVJ420P,
                PictureKind::Png => sys::AVPixelFormat::AV_PIX_FMT_RGBA,
            } as c_int;
            (*parameters).width = width;
            (*parameters).height = height;
            (*stream).time_base = sys::AVRational { num: 1, den: 1 };
            (*stream).disposition |= sys::AV_DISPOSITION_ATTACHED_PIC;
        }
        Ok(self.last_stream_index())
    }

    /// Adds the audio stream `aac` describes and returns its index.
    pub fn add_aac_stream(&mut self, aac: &AacStream<'_>) -> Result<usize> {
        let step = Step::AddStream;
        let number = |value: u32| c_int::try_from(value).map_err(|_| Error::OutOfRange { step });
        let (rate, frame, priming) = (
            number(aac.rate)?,
            number(aac.frame_samples)?,
            number(aac.priming_samples)?,
        );
        let stream = self.new_stream()?;
        // SAFETY: the new stream belongs to the live context `self` owns and
        // owns its `codecpar`, whose blank layout holds no allocation.
        unsafe {
            let parameters = (*stream).codecpar;
            (*parameters).codec_type = sys::AVMediaType::AVMEDIA_TYPE_AUDIO;
            (*parameters).codec_id = sys::AVCodecID::AV_CODEC_ID_AAC;
            (*parameters).profile = if aac.high_efficiency {
                sys::AV_PROFILE_AAC_HE
            } else {
                sys::AV_PROFILE_AAC_LOW
            };
            (*parameters).sample_rate = rate;
            sys::av_channel_layout_default(&mut (*parameters).ch_layout, c_int::from(aac.channels));
            (*parameters).bit_rate = aac.bit_rate;
            (*parameters).frame_size = frame;
            (*parameters).initial_padding = priming;
            set_extradata(parameters, aac.config, step)?;
        }
        Ok(self.last_stream_index())
    }

    /// Adds a Matroska attachment holding `bytes` and returns its stream
    /// index. The muxer writes it with the header.
    pub fn add_attachment_stream(
        &mut self,
        codec: CodecId,
        bytes: &[u8],
        tags: &Tags,
    ) -> Result<usize> {
        let step = Step::AddStream;
        let dictionary = tags.to_dictionary(Step::SetTags)?;
        let stream = self.new_stream()?;
        // SAFETY: the new stream belongs to the live context `self` owns and
        // owns its `codecpar` and metadata slot.
        unsafe {
            let parameters = (*stream).codecpar;
            (*parameters).codec_type = sys::AVMediaType::AVMEDIA_TYPE_ATTACHMENT;
            codec.write(ptr::addr_of_mut!((*parameters).codec_id));
            set_extradata(parameters, bytes, step)?;
            replace_dictionary(&mut (*stream).metadata, dictionary);
        }
        Ok(self.last_stream_index())
    }

    /// Sets the samples per packet a muxer assumes for an audio stream.
    pub fn set_stream_frame_size(&mut self, index: usize, samples: u32) -> Result<()> {
        let step = Step::AddStream;
        let samples = c_int::try_from(samples).map_err(|_| Error::OutOfRange { step })?;
        let stream = self.stream(index)?;
        // SAFETY: `stream` belongs to the live context `self` owns, and every
        // stream FFmpeg creates has a `codecpar`.
        unsafe { (*(*stream).codecpar).frame_size = samples };
        Ok(())
    }

    /// Sets a picture stream's size in pixels.
    pub fn set_stream_picture_size(&mut self, index: usize, width: u32, height: u32) -> Result<()> {
        let step = Step::AddStream;
        let size = |value: u32| c_int::try_from(value).map_err(|_| Error::OutOfRange { step });
        let (width, height) = (size(width)?, size(height)?);
        let stream = self.stream(index)?;
        // SAFETY: `stream` belongs to the live context `self` owns, and every
        // stream FFmpeg creates has a `codecpar`.
        unsafe {
            (*(*stream).codecpar).width = width;
            (*(*stream).codecpar).height = height;
        }
        Ok(())
    }

    /// Restates an audio stream's sample rate, frame size, and initial
    /// padding on a clock of `rate` ticks a second.
    pub fn restate_stream_sample_clock(&mut self, index: usize, rate: u32) -> Result<()> {
        let step = Step::AddStream;
        let rate = c_int::try_from(rate).map_err(|_| Error::OutOfRange { step })?;
        let stream = self.stream(index)?;
        // SAFETY: `stream` belongs to the live context `self` owns, and every
        // stream FFmpeg creates has a `codecpar`.
        unsafe {
            let parameters = (*stream).codecpar;
            let old = (*parameters).sample_rate;
            if old <= 0 {
                return Err(Error::OutOfRange { step });
            }
            (*parameters).initial_padding = (*parameters).initial_padding * rate / old;
            (*parameters).frame_size = (*parameters).frame_size * rate / old;
            (*parameters).sample_rate = rate;
        }
        Ok(())
    }

    fn new_stream(&mut self) -> Result<*mut sys::AVStream> {
        // SAFETY: `self` owns a live context; a new stream belongs to it.
        let stream = unsafe { sys::avformat_new_stream(self.ctx(), ptr::null()) };
        if stream.is_null() {
            return Err(Error::OutOfMemory {
                step: Step::AddStream,
            });
        }
        Ok(stream)
    }

    fn last_stream_index(&self) -> usize {
        // SAFETY: `self` owns a live context with at least one stream.
        unsafe { (*self.ctx()).nb_streams as usize - 1 }
    }

    pub fn set_stream_time_base(&mut self, index: usize, time_base: Rational) -> Result<()> {
        let stream = self.stream(index)?;
        // SAFETY: `stream` belongs to the live context `self` owns.
        unsafe { (*stream).time_base = time_base.to_sys() };
        Ok(())
    }

    pub fn set_stream_tags(&mut self, index: usize, tags: &Tags) -> Result<()> {
        let stream = self.stream(index)?;
        let dictionary = tags.to_dictionary(Step::SetTags)?;
        // SAFETY: `stream` belongs to the live context `self` owns.
        unsafe { replace_dictionary(&mut (*stream).metadata, dictionary) };
        Ok(())
    }

    /// Sets the stream's disposition and clears its codec tag, so the muxer
    /// picks the tag its container uses.
    pub fn set_stream_disposition_and_clear_codec_tag(
        &mut self,
        index: usize,
        disposition: Disposition,
    ) -> Result<()> {
        let stream = self.stream(index)?;
        // SAFETY: `stream` belongs to the live context `self` owns, and every
        // stream FFmpeg creates has a `codecpar`.
        unsafe {
            (*stream).disposition = disposition.bits();
            (*(*stream).codecpar).codec_tag = 0;
        }
        Ok(())
    }

    pub fn set_tags(&mut self, tags: &Tags) -> Result<()> {
        let dictionary = tags.to_dictionary(Step::SetTags)?;
        // SAFETY: `self` owns a live context.
        unsafe { replace_dictionary(&mut (*self.ctx()).metadata, dictionary) };
        Ok(())
    }

    pub fn add_chapter(&mut self, chapter: &Chapter) -> Result<()> {
        if chapter.start > chapter.end {
            return Err(Error::ChapterEndsBeforeStart);
        }
        let metadata = chapter.tags.to_dictionary(Step::AddChapter)?;
        let step = Step::AddChapter;
        // SAFETY: av_mallocz returns zeroed memory sized for one chapter or null.
        let added =
            unsafe { sys::av_mallocz(size_of::<sys::AVChapter>()) }.cast::<sys::AVChapter>();
        if added.is_null() {
            return Err(Error::OutOfMemory { step });
        }
        // SAFETY: `added` is a zeroed chapter this function owns until the
        // context's array takes it; `self` owns a live context; on failure the
        // nofree variant leaves the array as it was.
        unsafe {
            (*added).id = chapter.id;
            (*added).time_base = chapter.time_base.to_sys();
            (*added).start = chapter.start;
            (*added).end = chapter.end;
            (*added).metadata = metadata.into_raw();
            let ctx = self.ctx();
            let mut count = c_int::try_from((*ctx).nb_chapters).unwrap_or(c_int::MAX);
            let code = sys::av_dynarray_add_nofree(
                ptr::addr_of_mut!((*ctx).chapters).cast(),
                &mut count,
                added.cast(),
            );
            if code < 0 {
                sys::av_dict_free(&mut (*added).metadata);
                sys::av_free(added.cast());
                return Err(Error::Ffmpeg { step, code });
            }
            (*ctx).nb_chapters = count as u32;
        }
        Ok(())
    }

    /// Writes the header and returns the muxer with the names of the options
    /// the muxer did not take. Values are never listed.
    pub fn write_header(self, options: &Tags) -> Result<(Muxer, Vec<String>)> {
        let mut dictionary = options.to_dictionary(Step::WriteHeader)?;
        // SAFETY: `self` owns a live context with an open file; FFmpeg replaces
        // the dictionary with the options it did not take.
        let code = unsafe { sys::avformat_write_header(self.ctx(), dictionary.as_mut_slot()) };
        Error::check(Step::WriteHeader, code)?;
        let unused = dictionary.keys();
        Ok((Muxer(self.0), unused))
    }

    fn ctx(&self) -> *mut sys::AVFormatContext {
        self.0.ctx.as_ptr()
    }

    fn stream(&self, index: usize) -> Result<*mut sys::AVStream> {
        // SAFETY: `self` owns a live context.
        unsafe { context::stream(self.ctx(), index, Step::AddStream) }
    }
}

impl Muxer {
    /// The stream's time base as the muxer chose it in the header.
    pub fn stream_time_base(&self, index: usize) -> Result<Rational> {
        // SAFETY: `self` owns a live context.
        let stream = unsafe { context::stream(self.0.ctx.as_ptr(), index, Step::WritePacket)? };
        // SAFETY: `stream` belongs to that context.
        Ok(Rational::from_sys(unsafe { (*stream).time_base }))
    }

    /// Sets one container tag. A muxer that writes its tags in the trailer,
    /// as MP4 does, still takes it.
    pub fn set_tag(&mut self, key: &str, value: &str) -> Result<()> {
        let step = Step::SetTags;
        let key = Text::new(key, step)?;
        let value = Text::new(value, step)?;
        // SAFETY: `self` owns a live context; FFmpeg copies both
        // NUL-terminated strings into the context's dictionary.
        let code = unsafe {
            sys::av_dict_set(
                &mut (*self.0.ctx.as_ptr()).metadata,
                key.as_ptr(),
                value.as_ptr(),
                0,
            )
        };
        Error::check(step, code).map(drop)
    }

    /// Writes the packet in interleaved order; the packet is blank afterwards.
    pub fn write_interleaved(&mut self, packet: &mut Packet) -> Result<()> {
        if packet.lacks_payload_and_side_data() {
            return Err(Error::EmptyPacket);
        }
        // SAFETY: `self` owns a live context after its header; FFmpeg takes
        // the packet's reference and leaves it blank.
        let code =
            unsafe { sys::av_interleaved_write_frame(self.0.ctx.as_ptr(), packet.as_mut_ptr()) };
        Error::check(Step::WritePacket, code).map(drop)
    }

    /// Writes the trailer and closes the file, keeping it only when both
    /// succeed.
    pub fn finish(mut self) -> Result<()> {
        // SAFETY: `self` owns a live context after its header.
        let trailer = unsafe { sys::av_write_trailer(self.0.ctx.as_ptr()) };
        let closed = self.0.close();
        Error::check(Step::WriteTrailer, trailer)?;
        Error::check(Step::CloseOutput, closed)?;
        self.0.kept = true;
        Ok(())
    }
}

/// Gives `parameters` an owned, padded copy of `bytes` as its extradata.
///
/// # Safety
/// `parameters` is live and holds no extradata.
unsafe fn set_extradata(
    parameters: *mut sys::AVCodecParameters,
    bytes: &[u8],
    step: Step,
) -> Result<()> {
    let size = c_int::try_from(bytes.len()).map_err(|_| Error::OutOfRange { step })?;
    // SAFETY: av_mallocz returns zeroed memory of the padded size or null;
    // the copy fills exactly `bytes.len()` of it; the caller guarantees
    // `parameters`, which frees the buffer with the stream.
    unsafe {
        let data =
            sys::av_mallocz(bytes.len() + sys::AV_INPUT_BUFFER_PADDING_SIZE as usize).cast::<u8>();
        if data.is_null() {
            return Err(Error::OutOfMemory { step });
        }
        ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
        (*parameters).extradata = data;
        (*parameters).extradata_size = size;
    }
    Ok(())
}

/// # Safety
/// `slot` is a live context's dictionary field.
unsafe fn replace_dictionary(slot: &mut *mut sys::AVDictionary, dictionary: Dictionary) {
    // SAFETY: the caller guarantees `slot`; the old dictionary is freed before
    // the new one takes its place.
    unsafe { sys::av_dict_free(slot) };
    *slot = dictionary.into_raw();
}

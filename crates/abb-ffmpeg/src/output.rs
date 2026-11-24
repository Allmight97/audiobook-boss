use std::ffi::{c_int, CStr, CString};
use std::path::{Path, PathBuf};
use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use crate::codec::CodecId;
use crate::context::{Chapter, Disposition, FormatContext, Rational, StreamFacts};
use crate::encoder::AudioEncoder;
use crate::error::{Error, Result, Step};
use crate::input::{path_text, Input};
use crate::packet::Packet;
use crate::text::{Dictionary, Tags, Text};

pub struct AacStream<'a> {
    pub high_efficiency: bool,
    pub rate: u32,
    pub channels: u16,
    pub bit_rate: i64,
    pub frame_samples: u32,
    pub priming_samples: u32,
    pub config: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureKind {
    Jpeg,
    Png,
}

struct OutputFile {
    ctx: FormatContext,
    path: PathBuf,
    kept: bool,
}

impl OutputFile {
    fn close(&mut self) -> c_int {
        // SAFETY: avio_closep tolerates a null `pb` and nulls it.
        unsafe { sys::avio_closep(&mut self.ctx.raw_mut().pb) }
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

pub struct Output(OutputFile);

pub struct Muxer(OutputFile);

impl Output {
    pub fn create(path: &Path, muxer: &str) -> Result<Self> {
        let muxer = CString::new(muxer).map_err(|_| Error::NulInText {
            step: Step::CreateOutput,
        })?;
        let url = path_text(path, Step::CreateOutput)?;
        let mut ctx = ptr::null_mut();
        // SAFETY: `url` and `muxer` are NUL-terminated; FFmpeg allocates `ctx`.
        let code = unsafe {
            sys::avformat_alloc_output_context2(&mut ctx, ptr::null(), muxer.as_ptr(), url.as_ptr())
        };
        Error::check(Step::CreateOutput, code)?;
        let ctx = FormatContext::new(NonNull::new(ctx).ok_or(Error::OutOfMemory {
            step: Step::CreateOutput,
        })?);
        let mut file = OutputFile {
            ctx,
            path: path.to_path_buf(),
            kept: false,
        };
        // SAFETY: `pb` is null until avio_open opens it for writing.
        let code = unsafe {
            sys::avio_open(
                &mut file.ctx.raw_mut().pb,
                url.as_ptr(),
                sys::AVIO_FLAG_WRITE,
            )
        };
        if code < 0 {
            file.kept = true;
        }
        Error::check(Step::OpenOutputFile, code)?;
        Ok(Self(file))
    }

    pub fn muxer_name(&self) -> String {
        // SAFETY: an output context always has an output format with a static NUL-terminated name.
        let name = unsafe { CStr::from_ptr((*self.0.ctx.raw().oformat).name) };
        Text::copied_from(name).display()
    }

    pub fn streams(&self) -> Vec<StreamFacts> {
        self.0.ctx.stream_facts()
    }

    pub fn chapters(&self) -> Vec<Chapter> {
        self.0.ctx.chapters()
    }

    pub fn add_stream_copied_from(&mut self, input: &Input, input_index: usize) -> Result<usize> {
        let source = input.context().stream(input_index, Step::AddStream)?;
        let (index, stream) = self.new_stream()?;
        // SAFETY: both streams are live and own their `codecpar`.
        let code = unsafe { sys::avcodec_parameters_copy(stream.codecpar, source.codecpar) };
        Error::check(Step::CopyStreamParameters, code)?;
        Ok(index)
    }

    pub fn wants_global_header(&self) -> bool {
        // SAFETY: an output context always has an output format.
        let flags = unsafe { (*self.0.ctx.raw().oformat).flags };
        flags & sys::AVFMT_GLOBALHEADER != 0
    }

    pub fn add_stream_for_encoder(&mut self, encoder: &AudioEncoder) -> Result<usize> {
        let (index, stream) = self.new_stream()?;
        // SAFETY: the new stream owns its `codecpar`; the encoder is live.
        let code =
            unsafe { sys::avcodec_parameters_from_context(stream.codecpar, encoder.as_ptr()) };
        Error::check(Step::CopyStreamParameters, code)?;
        Ok(index)
    }

    pub fn add_attached_picture_stream(
        &mut self,
        kind: PictureKind,
        width: u32,
        height: u32,
    ) -> Result<usize> {
        let step = Step::AddStream;
        let size = |value: u32| c_int::try_from(value).map_err(|_| Error::OutOfRange { step });
        let (width, height) = (size(width)?, size(height)?);
        let (index, stream) = self.new_stream()?;
        // SAFETY: every stream FFmpeg creates owns its `codecpar`.
        let parameters = unsafe { &mut *stream.codecpar };
        parameters.codec_type = sys::AVMediaType::AVMEDIA_TYPE_VIDEO;
        parameters.codec_id = match kind {
            PictureKind::Jpeg => sys::AVCodecID::AV_CODEC_ID_MJPEG,
            PictureKind::Png => sys::AVCodecID::AV_CODEC_ID_PNG,
        };
        parameters.format = match kind {
            PictureKind::Jpeg => sys::AVPixelFormat::AV_PIX_FMT_YUVJ420P,
            PictureKind::Png => sys::AVPixelFormat::AV_PIX_FMT_RGBA,
        } as c_int;
        parameters.width = width;
        parameters.height = height;
        stream.time_base = sys::AVRational { num: 1, den: 1 };
        stream.disposition |= sys::AV_DISPOSITION_ATTACHED_PIC;
        Ok(index)
    }

    pub fn add_aac_stream(&mut self, aac: &AacStream<'_>) -> Result<usize> {
        let step = Step::AddStream;
        let number = |value: u32| c_int::try_from(value).map_err(|_| Error::OutOfRange { step });
        let (rate, frame, priming) = (
            number(aac.rate)?,
            number(aac.frame_samples)?,
            number(aac.priming_samples)?,
        );
        let (index, stream) = self.new_stream()?;
        // SAFETY: every stream FFmpeg creates owns its `codecpar`, whose blank
        // layout holds no allocation.
        let parameters = unsafe { &mut *stream.codecpar };
        parameters.codec_type = sys::AVMediaType::AVMEDIA_TYPE_AUDIO;
        parameters.codec_id = sys::AVCodecID::AV_CODEC_ID_AAC;
        parameters.profile = if aac.high_efficiency {
            sys::AV_PROFILE_AAC_HE
        } else {
            sys::AV_PROFILE_AAC_LOW
        };
        parameters.sample_rate = rate;
        // SAFETY: `parameters.ch_layout` is a blank layout FFmpeg fills without allocating.
        unsafe {
            sys::av_channel_layout_default(&mut parameters.ch_layout, c_int::from(aac.channels))
        };
        parameters.bit_rate = aac.bit_rate;
        parameters.frame_size = frame;
        parameters.initial_padding = priming;
        set_extradata(parameters, aac.config, step)?;
        Ok(index)
    }

    pub fn add_attachment_stream(
        &mut self,
        codec: CodecId,
        bytes: &[u8],
        tags: &Tags,
    ) -> Result<usize> {
        let step = Step::AddStream;
        let dictionary = tags.to_dictionary(Step::SetTags)?;
        let (index, stream) = self.new_stream()?;
        // SAFETY: every stream FFmpeg creates owns its `codecpar`.
        let parameters = unsafe { &mut *stream.codecpar };
        parameters.codec_type = sys::AVMediaType::AVMEDIA_TYPE_ATTACHMENT;
        codec.write(&mut parameters.codec_id);
        set_extradata(parameters, bytes, step)?;
        replace_dictionary(&mut stream.metadata, dictionary);
        Ok(index)
    }

    pub fn set_stream_frame_size(&mut self, index: usize, samples: u32) -> Result<()> {
        let step = Step::AddStream;
        let samples = c_int::try_from(samples).map_err(|_| Error::OutOfRange { step })?;
        let stream = self.stream(index)?;
        // SAFETY: every stream FFmpeg creates has a `codecpar`.
        unsafe { (*stream.codecpar).frame_size = samples };
        Ok(())
    }

    pub fn set_stream_picture_size(&mut self, index: usize, width: u32, height: u32) -> Result<()> {
        let step = Step::AddStream;
        let size = |value: u32| c_int::try_from(value).map_err(|_| Error::OutOfRange { step });
        let (width, height) = (size(width)?, size(height)?);
        let stream = self.stream(index)?;
        // SAFETY: every stream FFmpeg creates has a `codecpar`.
        unsafe {
            (*stream.codecpar).width = width;
            (*stream.codecpar).height = height;
        }
        Ok(())
    }

    pub fn restate_stream_sample_clock(&mut self, index: usize, rate: u32) -> Result<()> {
        let step = Step::AddStream;
        let rate = c_int::try_from(rate).map_err(|_| Error::OutOfRange { step })?;
        let stream = self.stream(index)?;
        // SAFETY: every stream FFmpeg creates has a `codecpar`.
        let parameters = unsafe { &mut *stream.codecpar };
        let old = parameters.sample_rate;
        if old <= 0 {
            return Err(Error::OutOfRange { step });
        }
        parameters.initial_padding = parameters.initial_padding * rate / old;
        parameters.frame_size = parameters.frame_size * rate / old;
        parameters.sample_rate = rate;
        Ok(())
    }

    fn new_stream(&mut self) -> Result<(usize, &mut sys::AVStream)> {
        let step = Step::AddStream;
        // SAFETY: `self` owns a live context; a new stream belongs to it.
        let stream = unsafe { sys::avformat_new_stream(self.0.ctx.as_ptr(), ptr::null()) };
        if stream.is_null() {
            return Err(Error::OutOfMemory { step });
        }
        // SAFETY: `stream` is non-null and exclusively borrowed through `&mut self`.
        let stream = unsafe { &mut *stream };
        let index = usize::try_from(stream.index).map_err(|_| Error::OutOfRange { step })?;
        Ok((index, stream))
    }

    pub fn set_stream_time_base(&mut self, index: usize, time_base: Rational) -> Result<()> {
        self.stream(index)?.time_base = time_base.to_sys();
        Ok(())
    }

    pub fn set_stream_tags(&mut self, index: usize, tags: &Tags) -> Result<()> {
        let dictionary = tags.to_dictionary(Step::SetTags)?;
        let stream = self.stream(index)?;
        replace_dictionary(&mut stream.metadata, dictionary);
        Ok(())
    }

    pub fn set_stream_disposition_and_clear_codec_tag(
        &mut self,
        index: usize,
        disposition: Disposition,
    ) -> Result<()> {
        let stream = self.stream(index)?;
        stream.disposition = disposition.bits();
        // SAFETY: every stream FFmpeg creates has a `codecpar`.
        unsafe { (*stream.codecpar).codec_tag = 0 };
        Ok(())
    }

    pub fn set_tags(&mut self, tags: &Tags) -> Result<()> {
        let dictionary = tags.to_dictionary(Step::SetTags)?;
        replace_dictionary(&mut self.0.ctx.raw_mut().metadata, dictionary);
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
            let ctx = self.0.ctx.as_ptr();
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

    pub fn write_header(self, options: &Tags) -> Result<(Muxer, Vec<String>)> {
        let mut dictionary = options.to_dictionary(Step::WriteHeader)?;
        // SAFETY: `self` owns a live context with an open file; FFmpeg replaces
        // the dictionary with the options it did not take.
        let code =
            unsafe { sys::avformat_write_header(self.0.ctx.as_ptr(), dictionary.as_mut_slot()) };
        Error::check(Step::WriteHeader, code)?;
        let unused = dictionary.keys();
        Ok((Muxer(self.0), unused))
    }

    fn stream(&mut self, index: usize) -> Result<&mut sys::AVStream> {
        self.0.ctx.stream_mut(index, Step::AddStream)
    }
}

impl Muxer {
    pub fn stream_time_base(&self, index: usize) -> Result<Rational> {
        let stream = self.0.ctx.stream(index, Step::WritePacket)?;
        Ok(Rational::from_sys(stream.time_base))
    }

    pub fn set_tag(&mut self, key: &str, value: &str) -> Result<()> {
        let step = Step::SetTags;
        let key = Text::new(key, step)?;
        let value = Text::new(value, step)?;
        // SAFETY: FFmpeg copies both NUL-terminated strings into the context's dictionary.
        let code = unsafe {
            sys::av_dict_set(
                &mut self.0.ctx.raw_mut().metadata,
                key.as_ptr(),
                value.as_ptr(),
                0,
            )
        };
        Error::check(step, code).map(drop)
    }

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

fn set_extradata(parameters: &mut sys::AVCodecParameters, bytes: &[u8], step: Step) -> Result<()> {
    let size = c_int::try_from(bytes.len()).map_err(|_| Error::OutOfRange { step })?;
    // SAFETY: av_mallocz returns zeroed memory of the padded size or null;
    // av_freep frees any previous extradata buffer and nulls the pointer;
    // the copy fills exactly `bytes.len()` bytes.
    unsafe {
        let data =
            sys::av_mallocz(bytes.len() + sys::AV_INPUT_BUFFER_PADDING_SIZE as usize).cast::<u8>();
        if data.is_null() {
            return Err(Error::OutOfMemory { step });
        }
        ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
        sys::av_freep(ptr::addr_of_mut!(parameters.extradata).cast());
        parameters.extradata = data;
        parameters.extradata_size = size;
    }
    Ok(())
}

fn replace_dictionary(slot: &mut *mut sys::AVDictionary, dictionary: Dictionary) {
    // SAFETY: av_dict_free tolerates null or a live dictionary owned by `slot`.
    unsafe { sys::av_dict_free(slot) };
    *slot = dictionary.into_raw();
}

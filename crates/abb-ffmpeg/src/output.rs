use std::ffi::{c_int, CString};
use std::path::{Path, PathBuf};
use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use crate::context::{self, Chapter, Disposition, Rational, StreamFacts};
use crate::error::{Error, Result, Step};
use crate::input::{path_text, Input};
use crate::packet::Packet;
use crate::text::{Dictionary, Tags, Text};

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
        let url = path_text(path, Step::CreateOutput)?;
        let muxer = CString::new(muxer).map_err(|_| Error::NulInText {
            step: Step::CreateOutput,
        })?;
        let mut ctx = ptr::null_mut();
        // SAFETY: both strings are NUL-terminated; FFmpeg allocates `ctx`.
        let code = unsafe {
            sys::avformat_alloc_output_context2(&mut ctx, ptr::null(), muxer.as_ptr(), url.as_ptr())
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

    pub fn muxer_name(&self) -> String {
        // SAFETY: an output context always has an output format with a name.
        unsafe { Text::copied_from((*(*self.ctx()).oformat).name) }.to_string_lossy()
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
        // SAFETY: `self` owns a live context; a new stream belongs to it.
        let stream = unsafe { sys::avformat_new_stream(self.ctx(), ptr::null()) };
        if stream.is_null() {
            return Err(Error::OutOfMemory {
                step: Step::AddStream,
            });
        }
        // SAFETY: both streams are live and own their `codecpar`.
        let code = unsafe { sys::avcodec_parameters_copy((*stream).codecpar, (*source).codecpar) };
        Error::check(Step::CopyStreamParameters, code)?;
        // SAFETY: `self` owns a live context with at least the new stream.
        Ok(unsafe { (*self.ctx()).nb_streams } as usize - 1)
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

/// # Safety
/// `slot` is a live context's dictionary field.
unsafe fn replace_dictionary(slot: &mut *mut sys::AVDictionary, dictionary: Dictionary) {
    // SAFETY: the caller guarantees `slot`; the old dictionary is freed before
    // the new one takes its place.
    unsafe { sys::av_dict_free(slot) };
    *slot = dictionary.into_raw();
}

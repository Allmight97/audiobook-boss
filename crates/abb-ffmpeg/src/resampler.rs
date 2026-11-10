use std::ffi::c_int;
use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use crate::audio::{AudioFrame, ChannelLayout, SampleFormat};
use crate::decoder::AudioDecoder;
use crate::error::{Error, Result, Step};
use crate::text::Tags;

/// One side of a conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioShape {
    pub format: SampleFormat,
    pub layout: ChannelLayout,
    pub rate: u32,
}

/// A sample format, layout, and rate converter. Freed on drop.
pub struct Resampler {
    ctx: NonNull<sys::SwrContext>,
    input_rate: u32,
    output: AudioShape,
}

impl Resampler {
    pub fn new(input: AudioShape, output: AudioShape, options: &Tags) -> Result<Self> {
        let step = Step::CreateResampler;
        let format = input.format.to_sys(step)?;
        Self::create(&input.layout.to_sys(), format, input.rate, output, options)
    }

    /// A resampler for the frames `decoder` yields, whatever channel order
    /// they have.
    pub fn for_decoder(decoder: &AudioDecoder, output: AudioShape, options: &Tags) -> Result<Self> {
        let step = Step::CreateResampler;
        let format = decoder.format().to_sys(step)?;
        // SAFETY: the decoder's layout lives as long as the borrow of
        // `decoder`, which outlasts this call; FFmpeg copies it.
        unsafe {
            Self::create_from(
                decoder.layout_ptr(),
                format,
                decoder.rate(),
                output,
                options,
            )
        }
    }

    fn create(
        input_layout: &sys::AVChannelLayout,
        input_format: sys::AVSampleFormat,
        input_rate: u32,
        output: AudioShape,
        options: &Tags,
    ) -> Result<Self> {
        // SAFETY: `input_layout` is a live reference for the whole call.
        unsafe { Self::create_from(input_layout, input_format, input_rate, output, options) }
    }

    /// # Safety
    /// `input_layout` points to a live channel layout.
    unsafe fn create_from(
        input_layout: *const sys::AVChannelLayout,
        input_format: sys::AVSampleFormat,
        input_rate: u32,
        output: AudioShape,
        options: &Tags,
    ) -> Result<Self> {
        let step = Step::CreateResampler;
        let rate = |rate: u32| c_int::try_from(rate).map_err(|_| Error::OutOfRange { step });
        let mut dictionary = options.to_dictionary(step)?;
        let out_format = output.format.to_sys(step)?;
        let (in_rate, out_rate) = (rate(input_rate)?, rate(output.rate)?);
        let mut ctx = ptr::null_mut();
        // SAFETY: `ctx` is null, so FFmpeg allocates it; the output layout is
        // a plain value that lives through the call, and the caller
        // guarantees the input layout; FFmpeg copies both.
        let code = unsafe {
            sys::swr_alloc_set_opts2(
                &mut ctx,
                &output.layout.to_sys(),
                out_format,
                out_rate,
                input_layout,
                input_format,
                in_rate,
                0,
                ptr::null_mut(),
            )
        };
        let resampler = NonNull::new(ctx).map(|ctx| Self {
            ctx,
            input_rate,
            output,
        });
        Error::check(step, code)?;
        let resampler = resampler.ok_or(Error::OutOfMemory { step })?;
        // SAFETY: the context is a live AVOptions object; FFmpeg replaces the
        // dictionary with the options it did not take, and `dictionary` frees
        // them.
        let code = unsafe { sys::av_opt_set_dict(ctx.cast(), dictionary.as_mut_slot()) };
        Error::check(step, code)?;
        let unused = dictionary.keys();
        if !unused.is_empty() {
            return Err(Error::OptionsNotTaken {
                step,
                names: unused,
            });
        }
        // SAFETY: the context is allocated and configured.
        let code = unsafe { sys::swr_init(ctx) };
        Error::check(step, code)?;
        Ok(resampler)
    }

    pub fn input_rate(&self) -> u32 {
        self.input_rate
    }

    pub fn output(&self) -> AudioShape {
        self.output
    }

    /// The samples the resampler still holds, counted at `rate`.
    pub fn delay_at(&self, rate: u32) -> i64 {
        // SAFETY: `self` owns a live, initialized context.
        unsafe { sys::swr_get_delay(self.ctx.as_ptr(), i64::from(rate)) }
    }

    /// Converts `input` into `output`, which keeps its buffers and gets the
    /// number of samples written. Samples that do not fit stay buffered.
    pub fn convert(&mut self, input: &AudioFrame, output: &mut AudioFrame) -> Result<()> {
        self.convert_frame(input.as_ptr(), output)
    }

    /// Writes buffered samples into `output`; it gets zero samples when the
    /// resampler holds none.
    pub fn flush(&mut self, output: &mut AudioFrame) -> Result<()> {
        self.convert_frame(ptr::null(), output)
    }

    fn convert_frame(&mut self, input: *const sys::AVFrame, output: &mut AudioFrame) -> Result<()> {
        let step = Step::Resample;
        let rate = c_int::try_from(self.output.rate).map_err(|_| Error::OutOfRange { step })?;
        // SAFETY: `self` owns a live context; `output` is a live frame the
        // caller holds exclusively; `input` is null or a live frame.
        let code = unsafe {
            (*output.as_mut_ptr()).sample_rate = rate;
            sys::swr_convert_frame(self.ctx.as_ptr(), output.as_mut_ptr(), input)
        };
        Error::check(step, code).map(drop)
    }
}

impl Drop for Resampler {
    fn drop(&mut self) {
        let mut ctx = self.ctx.as_ptr();
        // SAFETY: `self` owns the context and nothing uses it after this.
        unsafe { sys::swr_free(&mut ctx) };
    }
}

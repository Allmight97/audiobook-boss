use super::*;
use std::{ffi::CStr, mem::size_of, ptr};

struct Handle(*mut faac_encoder);

impl Drop for Handle {
    fn drop(&mut self) {
        // The test owns the handle, including when an assertion unwinds.
        unsafe { faac_encoder_close(&mut self.0) };
    }
}

#[test]
fn float_raw_encoder_opens_drains_short_input_and_closes() {
    for profile in [FAAC_OBJ_LOW, FAAC_OBJ_HE_AAC_V1] {
        for rate in [22_050, 32_000, 44_100, 48_000] {
            for channels in [1, 2] {
                smoke_encode(profile, rate, channels);
            }
        }
    }
}

fn smoke_encode(profile: faac_object_type, rate: u32, channels: u32) {
    let mut params = faac_params::default();
    assert_eq!(
        unsafe { faac_params_init(&mut params, size_of::<faac_params>() as u32) },
        FAAC_OK
    );
    params.sample_rate = rate;
    params.num_channels = channels;
    params.object_type = profile;
    params.bit_rate = 64_000 / channels;
    params.output_format = FAAC_STREAM_RAW;
    params.input_format = FAAC_INPUT_FLOAT;
    let mut handle = Handle(ptr::null_mut());
    let status = unsafe { faac_encoder_open(&params, &mut handle.0) };
    if profile == FAAC_OBJ_HE_AAC_V1 && rate < 32_000 {
        assert_eq!(status, FAAC_ERR_INVALID_ARGUMENT);
        assert!(handle.0.is_null());
        return;
    }
    assert_eq!(
        status, FAAC_OK,
        "profile={profile} rate={rate} channels={channels}"
    );
    assert!(!handle.0.is_null());

    let mut info = faac_encoder_info {
        struct_size: size_of::<faac_encoder_info>() as u32,
        ..Default::default()
    };
    assert_eq!(
        unsafe { faac_encoder_get_info(handle.0, &mut info) },
        FAAC_OK
    );
    assert_eq!(info.object_type, profile);
    assert_eq!(info.sample_rate, rate);
    assert_eq!(
        info.frame_samples,
        if profile == FAAC_OBJ_LOW { 1024 } else { 2048 }
    );
    assert!(info.max_output_bytes > 0);

    let mut asc = ptr::null();
    let mut asc_len = 0;
    assert_eq!(
        unsafe { faac_encoder_asc(handle.0, &mut asc, &mut asc_len) },
        FAAC_OK
    );
    assert!(!asc.is_null());
    assert!(asc_len >= 2);
    let asc_copy = unsafe { std::slice::from_raw_parts(asc, asc_len as usize) }.to_vec();
    assert_eq!((asc_copy[1] >> 3) & 15, channels as u8);

    // FLOAT takes signed-16-scaled samples. A sub-frame source exercises the
    // public flush-until-zero contract that formerly lost short recordings.
    let input: Vec<f32> = (0..257 * channels)
        .map(|index| (index as f32 * 0.07).sin() * 8000.0)
        .collect();
    let mut output = vec![0; info.max_output_bytes as usize];
    let mut written = 0;
    assert_eq!(
        unsafe {
            faac_encoder_encode(
                handle.0,
                input.as_ptr().cast(),
                input.len() as u32,
                output.as_mut_ptr(),
                info.max_output_bytes,
                &mut written,
            )
        },
        FAAC_OK
    );
    let mut packets = usize::from(written > 0);
    let mut drained = false;
    for _ in 0..16 {
        assert_eq!(
            unsafe {
                faac_encoder_encode(
                    handle.0,
                    ptr::null(),
                    0,
                    output.as_mut_ptr(),
                    info.max_output_bytes,
                    &mut written,
                )
            },
            FAAC_OK
        );
        assert!(written <= info.max_output_bytes);
        if written == 0 {
            drained = true;
            break;
        }
        packets += 1;
    }
    assert!(drained, "encoder must reach EOF");
    assert!(packets > 0, "short input must produce packets before EOF");
    assert_eq!(unsafe { faac_encoder_close(&mut handle.0) }, FAAC_OK);
    assert!(handle.0.is_null());
    // Drop repeats close on a null handle, as promised by the public API.
    assert!(!asc_copy.is_empty());
}

#[test]
fn library_info_reports_the_bundled_configuration() {
    let mut info = faac_library_info {
        struct_size: size_of::<faac_library_info>() as u32,
        ..Default::default()
    };
    assert_eq!(unsafe { faac_get_library_info(&mut info) }, FAAC_OK);
    assert_eq!(info.max_channels, 2);
    assert_eq!(info.sbr_decimation, 1);
    assert_eq!(
        unsafe { CStr::from_ptr(info.version) }.to_bytes(),
        b"2.1.0-dev.3aa4c6d"
    );
}

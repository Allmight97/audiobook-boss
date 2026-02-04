use crate::errors::{AppError, Result};
use crate::metadata::set_stream_disposition_and_clear_codec_tag;
use ffmpeg_next as ff;
use std::fs;
use std::path::{Path, PathBuf};

const ACTIVATION_BYTES_LENGTH: usize = 16;

pub fn decrypt_audible_files(
    file_paths: &[PathBuf],
    activation_bytes: &str,
    retain_original: bool,
) -> Result<Vec<PathBuf>> {
    if file_paths.is_empty() {
        return Err(AppError::InvalidInput(
            "No Audible files provided for decryption".to_string(),
        ));
    }

    let normalized_bytes = normalize_activation_bytes(activation_bytes)?;
    let output_dir = audible_temp_dir()?;

    ff::init().map_err(AppError::Ffmpeg)?;

    let mut decrypted_files = Vec::with_capacity(file_paths.len());
    for path in file_paths {
        let canonical = crate::audio::path_validation::validate_input_audio_path(path)?;
        let output_path = decrypt_single_audible_file(&canonical, &output_dir, &normalized_bytes)?;
        decrypted_files.push(output_path);

        if !retain_original {
            fs::remove_file(&canonical).map_err(|_| {
                AppError::ResourceCleanup("Failed to remove original Audible download".to_string())
            })?;
        }
    }

    Ok(decrypted_files)
}

fn normalize_activation_bytes(raw: &str) -> Result<String> {
    let trimmed: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    let cleaned = trimmed.strip_prefix("0x").unwrap_or(&trimmed);
    if cleaned.len() != ACTIVATION_BYTES_LENGTH {
        return Err(AppError::InvalidInput(format!(
            "Activation bytes must be {ACTIVATION_BYTES_LENGTH} hex characters"
        )));
    }

    if !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::InvalidInput(
            "Activation bytes must be hex characters only".to_string(),
        ));
    }

    Ok(cleaned.to_ascii_lowercase())
}

fn audible_temp_dir() -> Result<PathBuf> {
    let dir = std::env::temp_dir()
        .join(crate::audio::constants::TEMP_DIR_NAME)
        .join("audible-import");
    fs::create_dir_all(&dir).map_err(|_| {
        AppError::TempDirectoryCreation(
            "Failed to create temporary directory for Audible import".to_string(),
        )
    })?;
    Ok(dir)
}

fn decrypt_single_audible_file(
    input_path: &Path,
    output_dir: &Path,
    activation_bytes: &str,
) -> Result<PathBuf> {
    let mut opts = ff::Dictionary::new();
    opts.set("activation_bytes", activation_bytes);

    let mut ictx = ff::format::input_with_dictionary(input_path, opts).map_err(AppError::Ffmpeg)?;

    let file_stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(sanitize_file_stem)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "audible".to_string());
    let output_path = output_dir.join(format!("{}-{}.m4b", file_stem, uuid::Uuid::new_v4()));

    if output_path.exists() {
        fs::remove_file(&output_path).map_err(AppError::Io)?;
    }

    let mut octx = ff::format::output(&output_path).map_err(AppError::Ffmpeg)?;
    let stream_len = ictx.streams().len();
    let mut stream_mapping: Vec<isize> = vec![-1; stream_len];
    let mut output_time_bases: Vec<Option<ff::Rational>> = vec![None; stream_len];

    for (index, istream) in ictx.streams().enumerate() {
        if istream.parameters().medium() == ff::media::Type::Data {
            continue;
        }

        let codec_ctx = ff::codec::context::Context::from_parameters(istream.parameters())
            .map_err(AppError::Ffmpeg)?;
        let mut ostream = octx.add_stream_with(&codec_ctx).map_err(AppError::Ffmpeg)?;

        ostream.set_time_base(istream.time_base());
        ostream.set_metadata(istream.metadata().to_owned());
        set_stream_disposition_and_clear_codec_tag(&mut ostream, istream.disposition());

        stream_mapping[index] = ostream.index() as isize;
        output_time_bases[ostream.index()] = Some(ostream.time_base());
    }

    if ictx.nb_chapters() > 0 {
        for chapter in ictx.chapters() {
            let title = chapter.metadata().get("title").map(|s| s.to_string());
            if let Err(e) = octx.add_chapter(
                chapter.id(),
                chapter.time_base(),
                chapter.start(),
                chapter.end(),
                title.as_deref().unwrap_or(""),
            ) {
                log::warn!("Failed to add chapter id {}: {}", chapter.id(), e);
            }
        }
    }

    octx.set_metadata(ictx.metadata().to_owned());
    octx.write_header().map_err(AppError::Ffmpeg)?;

    for (input_stream, mut packet) in ictx.packets() {
        let in_index = input_stream.index();
        let out_index = *stream_mapping.get(in_index).unwrap_or(&-1);
        if out_index < 0 {
            continue;
        }

        let out_tb = output_time_bases
            .get(out_index as usize)
            .and_then(|tb| *tb)
            .unwrap_or(input_stream.time_base());

        packet.set_stream(out_index as usize);
        packet.rescale_ts(input_stream.time_base(), out_tb);
        packet
            .write_interleaved(&mut octx)
            .map_err(AppError::Ffmpeg)?;
    }

    octx.write_trailer().map_err(AppError::Ffmpeg)?;

    Ok(output_path)
}

fn sanitize_file_stem(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

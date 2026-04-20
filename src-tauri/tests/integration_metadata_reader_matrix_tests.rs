//! Baseline matrix comparing current metadata reads with ffmpeg-only and
//! mp4ameta-only observations for MP4/M4B reader experiments.

use audiobook_boss_lib::audio::AudioFile;
use audiobook_boss_lib::commands::metadata::{read_audio_metadata, save_metadata_to_file};
use audiobook_boss_lib::{
    add_chapters_to_output, extract_passthrough_metadata, ffmpeg_add_cover_art_stream_pre_header,
    ffmpeg_write_cover_art_packet_post_header, AudiobookMetadata, ChapterSpec,
};
use ffmpeg_next as ff;
use mp4ameta::{FreeformIdent, Tag, WriteConfig};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const MINIMAL_JPEG: &[u8] = include_bytes!("support/minimal.jpg");
const SERIES_KEYS: [&str; 4] = ["series", "----:com.apple.iTunes:SERIES", "show", "MVNM"];
const SERIES_PART_KEYS: [&str; 4] = [
    "series-part",
    "----:com.apple.iTunes:SERIES-PART",
    "episode_sort",
    "MVIN",
];
const TRACK_NUMBER_KEYS: [&str; 3] = ["track", "tracknumber", "trkn"];
const TRACK_TOTAL_KEYS: [&str; 3] = ["tracktotal", "totaltracks", "totaltrack"];
const DISK_NUMBER_KEYS: [&str; 4] = ["disc", "disk", "discnumber", "disknumber"];
const DISK_TOTAL_KEYS: [&str; 4] = ["disctotal", "disktotal", "totaldiscs", "totaldisks"];

#[derive(Debug)]
struct ReaderComparison {
    current: AudiobookMetadata,
    ffmpeg_only: AudiobookMetadata,
    mp4ameta_only: Result<AudiobookMetadata, String>,
}

fn sample_mp3_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("manifest parent")
        .join("media")
        .join("media_20sec.mp3")
}

fn write_minimal_m4b(output: &Path) {
    let codec = ff::encoder::find(ff::codec::Id::AAC).expect("aac encoder present");
    let mut octx = ff::format::output(output).expect("create output context");
    let time_base = ff::Rational(1, 44_100);

    let mut enc_ctx = ff::codec::context::Context::new()
        .encoder()
        .audio()
        .expect("encoder context");
    enc_ctx.set_rate(44_100);
    enc_ctx.set_channel_layout(ff::channel_layout::ChannelLayout::MONO);
    enc_ctx.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
    enc_ctx.set_time_base(time_base);
    let mut enc = enc_ctx.open_as(codec).expect("open encoder");

    let (stream_index, stream_time_base) = {
        let mut ost = octx.add_stream(codec).expect("add stream");
        ost.set_time_base(enc.time_base());
        ost.set_parameters(&enc);
        (ost.index(), ost.time_base())
    };
    octx.write_header().expect("write header");

    let mut frame = ff::frame::Audio::empty();
    frame.set_format(enc.format());
    frame.set_channel_layout(enc.channel_layout());
    frame.set_rate(enc.rate());
    frame.set_samples(1024);
    unsafe {
        frame.alloc(enc.format(), frame.samples(), enc.channel_layout());
    }
    frame.set_pts(Some(0));
    let plane = frame.data_mut(0);
    let samples: &mut [f32] =
        unsafe { std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut f32, frame.samples()) };
    samples.fill(0.0);

    let mut pkt = ff::Packet::empty();
    enc.send_frame(&frame).expect("send frame");
    while enc.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(stream_index);
        pkt.rescale_ts(enc.time_base(), stream_time_base);
        pkt.write_interleaved(&mut octx).expect("write packet");
    }

    enc.send_eof().ok();
    while enc.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(stream_index);
        pkt.rescale_ts(enc.time_base(), stream_time_base);
        pkt.write_interleaved(&mut octx).expect("write packet");
    }
    octx.write_trailer().expect("write trailer");
}

fn write_minimal_m4b_with_attached_pic(output: &Path, cover_bytes: &[u8]) {
    let codec = ff::encoder::find(ff::codec::Id::AAC).expect("aac encoder present");
    let mut octx = ff::format::output(output).expect("create output context");
    let time_base = ff::Rational(1, 44_100);

    let mut enc_ctx = ff::codec::context::Context::new()
        .encoder()
        .audio()
        .expect("encoder context");
    enc_ctx.set_rate(44_100);
    enc_ctx.set_channel_layout(ff::channel_layout::ChannelLayout::MONO);
    enc_ctx.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
    enc_ctx.set_time_base(time_base);
    let mut enc = enc_ctx.open_as(codec).expect("open encoder");

    let (stream_index, stream_time_base) = {
        let mut ost = octx.add_stream(codec).expect("add stream");
        ost.set_time_base(enc.time_base());
        ost.set_parameters(&enc);
        (ost.index(), ost.time_base())
    };
    let cover_stream_info = ffmpeg_add_cover_art_stream_pre_header(&mut octx, cover_bytes);
    octx.write_header().expect("write header");

    if let Some((cover_stream_index, format)) = cover_stream_info {
        ffmpeg_write_cover_art_packet_post_header(
            &mut octx,
            cover_stream_index,
            cover_bytes,
            format,
        );
    }

    let mut frame = ff::frame::Audio::empty();
    frame.set_format(enc.format());
    frame.set_channel_layout(enc.channel_layout());
    frame.set_rate(enc.rate());
    frame.set_samples(1024);
    unsafe {
        frame.alloc(enc.format(), frame.samples(), enc.channel_layout());
    }
    frame.set_pts(Some(0));
    let plane = frame.data_mut(0);
    let samples: &mut [f32] =
        unsafe { std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut f32, frame.samples()) };
    samples.fill(0.0);

    let mut pkt = ff::Packet::empty();
    enc.send_frame(&frame).expect("send frame");
    while enc.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(stream_index);
        pkt.rescale_ts(enc.time_base(), stream_time_base);
        pkt.write_interleaved(&mut octx).expect("write packet");
    }

    enc.send_eof().ok();
    while enc.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(stream_index);
        pkt.rescale_ts(enc.time_base(), stream_time_base);
        pkt.write_interleaved(&mut octx).expect("write packet");
    }
    octx.write_trailer().expect("write trailer");
}

fn write_minimal_m4b_with_chapters(output: &Path, chapters: &[ChapterSpec]) {
    let codec = ff::encoder::find(ff::codec::Id::AAC).expect("aac encoder present");
    let mut octx = ff::format::output(output).expect("create output context");
    let time_base = ff::Rational(1, 44_100);

    let mut enc_ctx = ff::codec::context::Context::new()
        .encoder()
        .audio()
        .expect("encoder context");
    enc_ctx.set_rate(44_100);
    enc_ctx.set_channel_layout(ff::channel_layout::ChannelLayout::MONO);
    enc_ctx.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
    enc_ctx.set_time_base(time_base);
    let mut enc = enc_ctx.open_as(codec).expect("open encoder");

    let (stream_index, stream_time_base) = {
        let mut ost = octx.add_stream(codec).expect("add stream");
        ost.set_time_base(enc.time_base());
        ost.set_parameters(&enc);
        (ost.index(), ost.time_base())
    };
    add_chapters_to_output(&mut octx, chapters).expect("add chapters");
    octx.write_header().expect("write header");

    let mut frame = ff::frame::Audio::empty();
    frame.set_format(enc.format());
    frame.set_channel_layout(enc.channel_layout());
    frame.set_rate(enc.rate());
    frame.set_samples(1024);
    unsafe {
        frame.alloc(enc.format(), frame.samples(), enc.channel_layout());
    }
    frame.set_pts(Some(0));
    let plane = frame.data_mut(0);
    let samples: &mut [f32] =
        unsafe { std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut f32, frame.samples()) };
    samples.fill(0.0);

    let mut pkt = ff::Packet::empty();
    enc.send_frame(&frame).expect("send frame");
    while enc.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(stream_index);
        pkt.rescale_ts(enc.time_base(), stream_time_base);
        pkt.write_interleaved(&mut octx).expect("write packet");
    }

    enc.send_eof().ok();
    while enc.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(stream_index);
        pkt.rescale_ts(enc.time_base(), stream_time_base);
        pkt.write_interleaved(&mut octx).expect("write packet");
    }
    octx.write_trailer().expect("write trailer");
}

fn normalize_publication_date(value: &str) -> Option<String> {
    let raw = value.trim();
    if raw.len() == 4 && raw.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(raw.to_string());
    }

    let bytes = raw.as_bytes();
    if bytes.len() < 7 {
        return None;
    }
    if !bytes[0..4].iter().all(u8::is_ascii_digit) || bytes[4] != b'-' {
        return None;
    }
    if !bytes[5..7].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let month = std::str::from_utf8(&bytes[5..7]).ok()?.parse::<u8>().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    if bytes.len() > 7 && !matches!(bytes[7], b'-' | b'T' | b' ') {
        return None;
    }

    Some(format!("{}-{}", &raw[0..4], &raw[5..7]))
}

fn split_series_list(value: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(raw) = value else {
        return (None, None);
    };
    let mut parts = raw
        .split(';')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty());
    let primary = parts.next().map(|part| part.to_string());
    let secondary = parts.next().map(|part| part.to_string());
    (primary, secondary)
}

fn first_tag(dict: &ff::DictionaryRef<'_>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| dict.get(key).map(str::to_string))
}

fn parse_number_pair(raw: &str) -> Option<(u32, Option<u32>)> {
    let mut parts = raw.split('/');
    let number = parts.next()?.trim().parse::<u32>().ok()?;
    let total = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<u32>().ok());
    Some((number, total))
}

fn parse_total_value(raw: &str) -> Option<u32> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        trimmed.parse::<u32>().ok()
    }
}

fn parse_position_field(primary: Option<&str>, total: Option<&str>) -> Option<(u32, Option<u32>)> {
    let primary = primary?.trim();
    if primary.is_empty() {
        return None;
    }

    if let Some((number, total)) = parse_number_pair(primary) {
        return Some((number, total));
    }

    let number = primary.parse::<u32>().ok()?;
    let total = total.and_then(parse_total_value);
    Some((number, total))
}

fn extract_attached_pic(ictx: &ff::format::context::Input) -> Option<Vec<u8>> {
    use ff::format::stream::Disposition;

    for stream in ictx.streams() {
        if stream.disposition().contains(Disposition::ATTACHED_PIC) {
            unsafe {
                let av_stream = stream.as_ptr();
                let pic = (*av_stream).attached_pic;
                if !pic.data.is_null() && pic.size > 0 {
                    let bytes = std::slice::from_raw_parts(pic.data, pic.size as usize);
                    return Some(bytes.to_vec());
                }
            }
        }
    }
    None
}

fn read_with_ffmpeg_only(path: &Path) -> Result<AudiobookMetadata, String> {
    ff::init().map_err(|e| e.to_string())?;

    let ictx = ff::format::input(path).map_err(|e| e.to_string())?;
    let dict = ictx.metadata();

    let mut metadata = AudiobookMetadata::new();
    metadata.title = dict.get("title").map(str::to_string);
    metadata.artist = dict.get("artist").map(str::to_string);
    metadata.album = dict.get("album").map(str::to_string);
    metadata.composer = dict.get("composer").map(str::to_string);
    metadata.genre = dict.get("genre").map(str::to_string);
    metadata.comment = dict.get("comment").map(str::to_string);
    metadata.description = dict.get("description").map(str::to_string);
    metadata.album_sort = dict.get("sort_album").map(str::to_string);
    metadata.track = parse_position_field(
        first_tag(&dict, &TRACK_NUMBER_KEYS).as_deref(),
        first_tag(&dict, &TRACK_TOTAL_KEYS).as_deref(),
    );
    metadata.disk = parse_position_field(
        first_tag(&dict, &DISK_NUMBER_KEYS).as_deref(),
        first_tag(&dict, &DISK_TOTAL_KEYS).as_deref(),
    );

    let series_raw = first_tag(&dict, &SERIES_KEYS);
    let series_part_raw = first_tag(&dict, &SERIES_PART_KEYS);
    let (series, subseries) = split_series_list(series_raw.as_deref());
    let (series_part, subseries_part) = split_series_list(series_part_raw.as_deref());
    metadata.series = series;
    metadata.series_part = series_part;
    metadata.subseries = subseries;
    metadata.subseries_part = subseries_part;
    metadata.date = dict
        .get("date")
        .or_else(|| dict.get("year"))
        .and_then(normalize_publication_date);
    metadata.cover_art = extract_attached_pic(&ictx);

    Ok(metadata)
}

fn read_with_mp4ameta_only(path: &Path) -> Result<AudiobookMetadata, String> {
    let tag = Tag::read_from_path(path).map_err(|e| e.to_string())?;
    let mut metadata = AudiobookMetadata::new();
    metadata.title = tag.title().map(str::to_string);
    metadata.album = tag.album().map(str::to_string);
    metadata.artist = tag
        .artist()
        .or_else(|| tag.album_artist())
        .map(str::to_string);
    metadata.composer = tag.composer().map(str::to_string);
    metadata.genre = tag.genre().map(str::to_string);
    metadata.comment = tag.comment().map(str::to_string);
    metadata.description = tag.description().map(str::to_string);
    metadata.album_sort = tag.album_sort_order().map(str::to_string);
    metadata.date = tag.year().and_then(normalize_publication_date);

    let series_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES");
    let series_part_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES-PART");
    let series_raw = tag
        .strings_of(&series_ident)
        .next()
        .map(str::to_string)
        .or_else(|| tag.tv_show_name().map(str::to_string))
        .or_else(|| tag.movement().map(str::to_string));
    let series_part_raw = tag
        .strings_of(&series_part_ident)
        .next()
        .map(str::to_string)
        .or_else(|| tag.tv_episode().map(|episode| episode.to_string()))
        .or_else(|| tag.movement_index().map(|idx| idx.to_string()));
    let (series, subseries) = split_series_list(series_raw.as_deref());
    let (series_part, subseries_part) = split_series_list(series_part_raw.as_deref());
    metadata.series = series;
    metadata.series_part = series_part;
    metadata.subseries = subseries;
    metadata.subseries_part = subseries_part;
    metadata.cover_art = tag.artwork().map(|img| img.data.to_vec());

    Ok(metadata)
}

async fn compare_readers(path: &Path) -> ReaderComparison {
    ReaderComparison {
        current: read_audio_metadata(path.to_string_lossy().to_string())
            .await
            .expect("current read"),
        ffmpeg_only: read_with_ffmpeg_only(path).expect("ffmpeg-only read"),
        mp4ameta_only: read_with_mp4ameta_only(path),
    }
}

fn assert_metadata_eq(actual: &AudiobookMetadata, expected: &AudiobookMetadata, context: &str) {
    assert_eq!(actual.title, expected.title, "{context}: title");
    assert_eq!(actual.artist, expected.artist, "{context}: artist");
    assert_eq!(actual.album, expected.album, "{context}: album");
    assert_eq!(actual.composer, expected.composer, "{context}: composer");
    assert_eq!(actual.genre, expected.genre, "{context}: genre");
    assert_eq!(actual.comment, expected.comment, "{context}: comment");
    assert_eq!(
        actual.description, expected.description,
        "{context}: description"
    );
    assert_eq!(actual.date, expected.date, "{context}: date");
    assert_eq!(actual.series, expected.series, "{context}: series");
    assert_eq!(
        actual.series_part, expected.series_part,
        "{context}: series_part"
    );
    assert_eq!(actual.subseries, expected.subseries, "{context}: subseries");
    assert_eq!(
        actual.subseries_part, expected.subseries_part,
        "{context}: subseries_part"
    );
    assert_eq!(
        actual.album_sort, expected.album_sort,
        "{context}: album_sort"
    );
    assert_eq!(actual.track, expected.track, "{context}: track");
    assert_eq!(actual.disk, expected.disk, "{context}: disk");
    assert_eq!(
        actual.cover_art.as_ref().map(|bytes| bytes.is_empty()),
        expected.cover_art.as_ref().map(|bytes| bytes.is_empty()),
        "{context}: cover_art emptiness"
    );
    match (&actual.cover_art, &expected.cover_art) {
        (Some(actual), Some(expected)) => assert_eq!(actual, expected, "{context}: cover_art"),
        (None, None) => {}
        _ => panic!("{context}: cover_art presence mismatch"),
    }
}

fn write_legacy_alias_series(output: &Path) {
    write_minimal_m4b(output);

    let mut tag = Tag::read_from_path(output).expect("read tag");
    let series_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES");
    let part_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES-PART");
    tag.remove_data_of(&series_ident);
    tag.remove_data_of(&part_ident);
    tag.remove_movement();
    tag.remove_movement_index();
    tag.remove_show_movement();
    tag.set_tv_show_name("Legacy Show Series");
    tag.set_tv_episode(42);
    let config = WriteConfig {
        write_meta_items: true,
        ..WriteConfig::NONE
    };
    tag.write_with_path(output, &config)
        .expect("write legacy alias tags");
}

fn remux_with_container_metadata(input: &Path, output: &Path, metadata: &[(&str, &str)]) {
    ff::init().expect("ffmpeg init");

    let mut ictx = ff::format::input(input).expect("open source input");
    let mut octx = ff::format::output(output).expect("create output context");
    let mut stream_mapping = vec![-1isize; ictx.streams().len()];
    let mut output_time_bases: Vec<Option<ff::Rational>> = vec![None; ictx.streams().len()];

    for (index, istream) in ictx.streams().enumerate() {
        let codec_ctx = ff::codec::context::Context::from_parameters(istream.parameters())
            .expect("build stream context");
        let mut ostream = octx.add_stream_with(&codec_ctx).expect("add output stream");
        ostream.set_time_base(istream.time_base());
        ostream.set_metadata(istream.metadata().to_owned());
        stream_mapping[index] = ostream.index() as isize;
        output_time_bases[ostream.index()] = Some(ostream.time_base());
    }

    let mut dict = ictx.metadata().to_owned();
    for (key, value) in metadata {
        dict.set(key, value);
    }
    octx.set_metadata(dict);
    octx.write_header().expect("write output header");

    for (input_stream, mut packet) in ictx.packets() {
        let out_index = stream_mapping[input_stream.index()];
        if out_index < 0 {
            continue;
        }

        let out_time_base =
            output_time_bases[out_index as usize].unwrap_or_else(|| input_stream.time_base());
        packet.set_stream(out_index as usize);
        packet.rescale_ts(input_stream.time_base(), out_time_base);
        packet
            .write_interleaved(&mut octx)
            .expect("stream-copy packet with metadata");
    }

    octx.write_trailer().expect("write output trailer");
}

#[tokio::test]
async fn canonical_mp4_metadata_matches_ffmpeg_only() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("rich-canonical.m4b");

    write_minimal_m4b(&output);
    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            title: Some("Canonical Title".into()),
            artist: Some("Canonical Author".into()),
            album: Some("Canonical Album".into()),
            composer: Some("Canonical Narrator".into()),
            genre: Some("Sci-Fi".into()),
            date: Some("2026-03".into()),
            comment: Some("Short note".into()),
            description: Some("Longer description".into()),
            series: Some("Series Prime".into()),
            series_part: Some("7".into()),
            album_sort: Some("Series Prime 07 - Canonical Title".into()),
            cover_art: Some(MINIMAL_JPEG.to_vec()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("save metadata");

    let comparison = compare_readers(&output).await;
    assert_metadata_eq(
        &comparison.current,
        &comparison.ffmpeg_only,
        "canonical mp4 current vs ffmpeg_only",
    );
    let mp4a = comparison.mp4ameta_only.expect("mp4ameta-only read");
    assert_metadata_eq(
        &comparison.current,
        &mp4a,
        "canonical mp4 current vs mp4ameta_only",
    );
}

#[tokio::test]
async fn legacy_show_episode_sort_gap_is_closed_for_mp4ameta_reads() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("legacy-series.m4b");

    write_legacy_alias_series(&output);
    let comparison = compare_readers(&output).await;

    assert_eq!(
        comparison.ffmpeg_only.series.as_deref(),
        Some("Legacy Show Series")
    );
    assert_eq!(comparison.ffmpeg_only.series_part.as_deref(), Some("42"));
    assert_eq!(
        comparison.current.series.as_deref(),
        comparison.ffmpeg_only.series.as_deref()
    );
    assert_eq!(
        comparison.current.series_part.as_deref(),
        comparison.ffmpeg_only.series_part.as_deref()
    );

    let mp4a = comparison.mp4ameta_only.expect("mp4ameta-only read");
    assert_eq!(mp4a.series.as_deref(), Some("Legacy Show Series"));
    assert_eq!(mp4a.series_part.as_deref(), Some("42"));
    assert_eq!(comparison.current.series, mp4a.series);
    assert_eq!(comparison.current.series_part, mp4a.series_part);
}

#[tokio::test]
async fn movement_only_series_is_shared_compatibility_case() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("movement-only.m4b");

    write_minimal_m4b(&output);
    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            series: Some("Movement Series".into()),
            series_part: Some("9".into()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("seed movement tags");

    let mut tag = Tag::read_from_path(&output).expect("read tag");
    let series_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES");
    let part_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES-PART");
    tag.remove_data_of(&series_ident);
    tag.remove_data_of(&part_ident);
    tag.remove_tv_show_name();
    tag.remove_tv_episode();
    tag.remove_tv_episode_name();
    tag.set_movement("Movement Series");
    tag.set_movement_index(9);
    tag.set_show_movement();
    let config = WriteConfig {
        write_meta_items: true,
        ..WriteConfig::NONE
    };
    tag.write_with_path(&output, &config)
        .expect("write movement-only series tags");

    let raw_tag = Tag::read_from_path(&output).expect("read movement-only tag");
    assert_eq!(raw_tag.movement(), Some("Movement Series"));
    assert_eq!(raw_tag.movement_index(), Some(9));

    let comparison = compare_readers(&output).await;

    assert_eq!(
        comparison.current.series.as_deref(),
        Some("Movement Series")
    );
    assert_eq!(comparison.current.series_part.as_deref(), Some("9"));
    assert!(
        comparison.ffmpeg_only.series.is_none(),
        "ffmpeg-only currently misses movement-only series in this fixture"
    );
    assert!(
        comparison.ffmpeg_only.series_part.is_none(),
        "ffmpeg-only currently misses movement-only series_part in this fixture"
    );

    let mp4a = comparison.mp4ameta_only.expect("mp4ameta-only read");
    assert_eq!(mp4a.series.as_deref(), Some("Movement Series"));
    assert_eq!(mp4a.series_part.as_deref(), Some("9"));
}

#[tokio::test]
async fn covr_only_cover_is_visible_to_ffmpeg_only() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("covr-only.m4b");

    write_minimal_m4b(&output);
    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            cover_art: Some(MINIMAL_JPEG.to_vec()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("save cover art");

    let comparison = compare_readers(&output).await;
    assert!(
        comparison.ffmpeg_only.cover_art.is_some(),
        "ffmpeg-only should read covr cover art"
    );
    assert_metadata_eq(
        &comparison.current,
        &comparison.ffmpeg_only,
        "covr-only current vs ffmpeg_only",
    );
}

#[tokio::test]
async fn attached_pic_only_cover_matches_mp4ameta_on_synthetic_fixture() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("attached-pic-only.m4b");

    write_minimal_m4b_with_attached_pic(&output, MINIMAL_JPEG);
    let comparison = compare_readers(&output).await;

    assert!(
        comparison.current.cover_art.is_some(),
        "current reader should recover attached_pic cover art"
    );
    assert_metadata_eq(
        &comparison.current,
        &comparison.ffmpeg_only,
        "attached-pic current vs ffmpeg_only",
    );
    let mp4a = comparison.mp4ameta_only.expect("mp4ameta-only read");
    assert_eq!(
        mp4a.cover_art, comparison.current.cover_art,
        "this synthetic attached-pic fixture is not a discriminating mp4ameta gap"
    );
}

#[tokio::test]
async fn mislabeled_mp3_as_m4b_is_a_routing_artifact() {
    let temp = TempDir::new().expect("temp dir");
    let source = sample_mp3_path();
    let mp3_path = temp.path().join("fallback.mp3");
    std::fs::copy(&source, &mp3_path).expect("copy mp3 fixture");

    save_metadata_to_file(
        mp3_path.to_string_lossy().to_string(),
        AudiobookMetadata {
            title: Some("Fallback Title".into()),
            artist: Some("Fallback Author".into()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("save metadata");

    let spoofed = temp.path().join("fallback.m4b");
    std::fs::rename(&mp3_path, &spoofed).expect("rename to m4b");

    let current = read_audio_metadata(spoofed.to_string_lossy().to_string())
        .await
        .expect("current read");
    let ffmpeg_only = read_with_ffmpeg_only(&spoofed).expect("ffmpeg-only read");
    let mp4ameta_only = read_with_mp4ameta_only(&spoofed);

    assert_eq!(current.title.as_deref(), Some("Fallback Title"));
    assert_eq!(current.artist.as_deref(), Some("Fallback Author"));
    assert_eq!(ffmpeg_only.title.as_deref(), Some("Fallback Title"));
    assert_eq!(ffmpeg_only.artist.as_deref(), Some("Fallback Author"));
    assert!(mp4ameta_only.is_err(), "mp4ameta-only should fail");
}

#[tokio::test]
async fn forced_ffmpeg_fallback_reads_track_and_disk_truthfully() {
    let temp = TempDir::new().expect("temp dir");
    let staged_mp3 = temp.path().join("track-disk-source.mp3");
    remux_with_container_metadata(
        &sample_mp3_path(),
        &staged_mp3,
        &[
            ("title", "Fallback Track Test"),
            ("artist", "Fallback Artist"),
            ("track", "3/12"),
            ("disc", "1/2"),
        ],
    );

    let spoofed = temp.path().join("fallback-track-disk.m4b");
    std::fs::rename(&staged_mp3, &spoofed).expect("rename spoofed fallback source");

    let current = read_audio_metadata(spoofed.to_string_lossy().to_string())
        .await
        .expect("current read");
    let ffmpeg_only = read_with_ffmpeg_only(&spoofed).expect("ffmpeg-only read");
    let mp4ameta_only = read_with_mp4ameta_only(&spoofed);

    assert_eq!(current.title.as_deref(), Some("Fallback Track Test"));
    assert_eq!(current.artist.as_deref(), Some("Fallback Artist"));
    assert_eq!(current.track, Some((3, Some(12))));
    assert_eq!(current.disk, Some((1, Some(2))));
    assert_eq!(ffmpeg_only.track, Some((3, Some(12))));
    assert_eq!(ffmpeg_only.disk, Some((1, Some(2))));
    assert!(mp4ameta_only.is_err(), "mp4ameta-only should fail");
}

#[tokio::test]
async fn clear_cover_roundtrip_stays_clear_across_current_and_ffmpeg_only() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("cover-clear.m4b");

    write_minimal_m4b(&output);
    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            cover_art: Some(MINIMAL_JPEG.to_vec()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("seed cover art");

    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            cover_art: Some(Vec::new()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("clear cover art");

    let comparison = compare_readers(&output).await;
    assert!(comparison.current.cover_art.is_none(), "current cover_art");
    assert!(
        comparison.ffmpeg_only.cover_art.is_none(),
        "ffmpeg-only cover_art"
    );
    assert!(
        comparison
            .mp4ameta_only
            .expect("mp4ameta-only read")
            .cover_art
            .is_none(),
        "mp4ameta-only cover_art"
    );
}

#[tokio::test]
async fn chaptered_source_compares_metadata_reads_and_probe_truth() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("chaptered.m4b");
    write_minimal_m4b_with_chapters(
        &output,
        &[
            ChapterSpec {
                title: Some("One".into()),
                start_ms: 0,
                end_ms: 1000,
            },
            ChapterSpec {
                title: Some("Two".into()),
                start_ms: 1000,
                end_ms: 2000,
            },
        ],
    );

    let comparison = compare_readers(&output).await;
    assert_metadata_eq(
        &comparison.current,
        &comparison.ffmpeg_only,
        "chaptered source current vs ffmpeg_only",
    );
    let mp4a = comparison.mp4ameta_only.expect("mp4ameta-only read");
    assert_eq!(
        mp4a.title, comparison.current.title,
        "chaptered source title"
    );
    assert_eq!(
        mp4a.cover_art, comparison.current.cover_art,
        "chaptered source cover_art"
    );

    let mut file = AudioFile::new(output);
    file.is_valid = true;
    file.duration = Some(2.0);

    let passthrough = extract_passthrough_metadata(&[file]);
    assert_eq!(passthrough.chapters.len(), 2);
    assert_eq!(passthrough.chapters[0].title.as_deref(), Some("One"));
    assert_eq!(passthrough.chapters[1].title.as_deref(), Some("Two"));
}

#[tokio::test]
async fn invalid_mp4_family_input_fails_for_all_readers() {
    let temp = TempDir::new().expect("temp dir");
    let path = temp.path().join("invalid.m4b");
    std::fs::write(&path, b"not audio").expect("write invalid file");

    let current = read_audio_metadata(path.to_string_lossy().to_string()).await;
    let ffmpeg_only = read_with_ffmpeg_only(&path);
    let mp4ameta_only = read_with_mp4ameta_only(&path);

    assert!(current.is_err(), "current reader should fail");
    assert!(ffmpeg_only.is_err(), "ffmpeg-only reader should fail");
    assert!(mp4ameta_only.is_err(), "mp4ameta-only reader should fail");
}

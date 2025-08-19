# Commands Ran
```bash
ABB_DISABLE_FASTPATH=1 RUST_LOG=debug npm run tauri dev
```
# Results

## Test 1: several files into one M4B
- Bitrate honored (chose 56 kbps), sample rate (passthrough) honored, channels (defaults to mono) honored.
- Encoding was slower than FDK AAC but acceptable. CBR used.
- Audio props of output (JSON):
     ```JSON
     {
     "creatingLibrary":{"name":"MediaInfoLib","version":"25.07","url":"https://mediaarea.net/MediaInfo"},
     "media":{"@ref":"/Users/jstar/Downloads/ABB Tests/BAWLZZZ (2025).m4b","track":[{"@type":"General","AudioCount":"1",
     "ImageCount":"1",
     "FileExtension":"m4b",
     "Format":"MPEG-4",
     "Format_Profile":"Apple audio with iTunes info",
     "CodecID":"M4A ",
     "CodecID_Compatible":"M4A /isom/iso2",
     "FileSize":"7529387",
     "Duration":"1024.488",
     "OverallBitRate_Mode":"CBR",
     "OverallBitRate":"58795",
     "StreamSize":"205572",
     "HeaderSize":"36",
     "DataSize":"7277129",
     "FooterSize":"252222",
     "IsStreamable":"No",
     "Title":"BAWLZZZ",
     "Album":"The War of Art",
     "Track":"BAWLZZZ",
     "Genre":"Audio Book",
     "ContentType":"Audiobook",
     "File_Created_Date":"2025-08-18 23:50:09 UTC",
     "File_Created_Date_Local":"2025-08-18 16:50:09",
     "File_Modified_Date":"2025-08-18 23:50:26 UTC",
     "File_Modified_Date_Local":"2025-08-18 16:50:26",
     "Cover":"Yes",
     "Cover_Type":"Cover"},{"@type":"Audio","StreamOrder":"0",
     "ID":"1",
     "Format":"AAC",
     "Format_Settings_SBR":"No (Explicit)",
     "Format_AdditionalFeatures":"LC",
     "CodecID":"mp4a-40-2",
     "Duration":"1024.488",
     "Source_Duration":"1024.511",
     "BitRate_Mode":"CBR",
     "BitRate":"56824",
     "Channels":"1",
     "ChannelPositions":"Front: C",
     "ChannelLayout":"M",
     "SamplesPerFrame":"1024",
     "SamplingRate":"44100",
     "SamplingCount":"45179921",
     "FrameRate":"43.066",
     "FrameCount":"44121",
     "Source_FrameCount":"44122",
     "Compression_Mode":"Lossy",
     "StreamSize":"7277117",
     "Source_StreamSize":"7277121",
     "Default":"Yes",
     "AlternateGroup":"1"},{"@type":"Image","Type":"Cover",
     "Format":"JPEG",
     "MuxingMode":"moov-meta-covr",
     "Width":"680",
     "Height":"680",
     "ColorSpace":"YUV",
     "ChromaSubsampling":"4:4:4",
     "BitDepth":"8",
     "Compression_Mode":"Lossy",
     "StreamSize":"46694"}]}
     }
     ```

## Test 2: Single Long file into one M4B
  - Bitrate honored (chose 56 kbps), sample rate (passthrough) honored, channels (defaults to mono) honored.
  - each single percentage point in progress display took about 6 seconds to increment (too long, much slower than AAC FDK)
  - All meta data and file size UI elements appeared to display correct elements and calculate sizes accurately enough.
    - Audio props of output (JSON):
```JSON
{
"creatingLibrary":{"name":"MediaInfoLib","version":"25.07","url":"https://mediaarea.net/MediaInfo"},
"media":{"@ref":"/Users/jstar/Downloads/ABB Tests/DRAGON TEST (2025).m4b","track":[{"@type":"General","AudioCount":"1",
"ImageCount":"1",
"FileExtension":"m4b",
"Format":"MPEG-4",
"Format_Profile":"Apple audio with iTunes info",
"CodecID":"M4A ",
"CodecID_Compatible":"M4A /isom/iso2",
"FileSize":"237477436",
"Duration":"35740.084",
"OverallBitRate_Mode":"VBR",
"OverallBitRate":"53157",
"StreamSize":"6161636",
"HeaderSize":"36",
"DataSize":"231198187",
"FooterSize":"6279213",
"IsStreamable":"No",
"Title":"DRAGON TEST",
"Album":"The Pragmatic Programmer: 20th Anniversary Edition, 2nd Edition: Your Journey to Mastery",
"Track":"DRAGON TEST",
"Genre":"Audiobook",
"ContentType":"Audiobook",
"File_Created_Date":"2025-08-18 23:52:55 UTC",
"File_Created_Date_Local":"2025-08-18 16:52:55",
"File_Modified_Date":"2025-08-19 00:06:56 UTC",
"File_Modified_Date_Local":"2025-08-18 17:06:56",
"Cover":"Yes",
"Cover_Type":"Cover",
"Comment":"Dave Thomas and Andy Hunt wrote the first edition of this influential book in 1999 to help their clients create better software and rediscover the joy of coding. These lessons have helped a generation of programmers examine the very essence of software d"},{"@type":"Audio","StreamOrder":"0",
"ID":"1",
"Format":"AAC",
"Format_Settings_SBR":"No (Explicit)",
"Format_AdditionalFeatures":"LC",
"CodecID":"mp4a-40-2",
"Duration":"35740.084",
"Source_Duration":"35740.108",
"BitRate_Mode":"VBR",
"BitRate":"51750",
"BitRate_Maximum":"56000",
"Channels":"1",
"ChannelPositions":"Front: C",
"ChannelLayout":"M",
"SamplesPerFrame":"1024",
"SamplingRate":"44100",
"SamplingCount":"1576137704",
"FrameRate":"43.066",
"FrameCount":"1539197",
"Source_FrameCount":"1539198",
"Compression_Mode":"Lossy",
"StreamSize":"231198016",
"Source_StreamSize":"231198179",
"Default":"Yes",
"AlternateGroup":"1"},{"@type":"Image","Type":"Cover",
"Format":"JPEG",
"MuxingMode":"moov-meta-covr",
"Width":"1280",
"Height":"720",
"ColorSpace":"YUV",
"ChromaSubsampling":"4:2:0",
"BitDepth":"8",
"Compression_Mode":"Lossy",
"StreamSize":"117621"}]}
}

```


## Terminal Output
- Thousands of lines of terminal output; impossible to parse here.
- Each decoded packet/frame was slipping by quickly in terminal during processing.
- Here's snippet of Last 100 lines of terminal output:
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::processor::frame_pipeline] ✓ Packet 1539196 sent to decoder successfully
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::processor::frame_pipeline] Processing decoded frames for packet 1539196
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::processor::frame_pipeline] ✓ Decoded frames processed successfully for packet 1539196
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::processor::frame_pipeline] Processing packet from stream 0
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::processor::frame_pipeline] Sending packet 1539197 to decoder
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::processor::frame_pipeline] ✓ Packet 1539197 sent to decoder successfully
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::processor::frame_pipeline] Processing decoded frames for packet 1539197
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::processor::frame_pipeline] ✓ Decoded frames processed successfully for packet 1539197
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor::frame_pipeline] ✓ Processed 1539197 packets total
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Input packets processed successfully
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::media_pipeline] Flushing decoder frames for: /Users/jstar/Downloads/ABB Tests/David Thomas, Andrew Hunt - The Pragmatic Programmer 20th Anniversary, 2nd Editiony.m4b
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Decoder frames flushed successfully (skipped for simplicity)
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Decoder frames flushed successfully
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::media_pipeline] ✅ Completed processing file: /Users/jstar/Downloads/ABB Tests/David Thomas, Andrew Hunt - The Pragmatic Programmer 20th Anniversary, 2nd Editiony.m4b
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Completed processing input file 1/1
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ All input files processed successfully
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::media_pipeline] 🏁 Starting encoding finalization...
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Encoding finalization completed successfully
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::cleanup::guard] Session 09461679-9d98-4d75-aa72-9bb27ee87d3c: Removed path from cleanup: /var/folders/29/g7wmd39x0pb75l_n1hjrttcw0000gn/T/audiobook-boss/09461679-9d98-4d75-aa72-9bb27ee87d3c/merged.m4b
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::media_pipeline] Audio processing completed with metadata integration
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::cleanup::guard] Session 09461679-9d98-4d75-aa72-9bb27ee87d3c: No paths to clean up
[aac @ 0x8ad5e1500] Qavg: 8576.884
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor::finalize] Starting finalize stage metadata writing for: /var/folders/29/g7wmd39x0pb75l_n1hjrttcw0000gn/T/audiobook-boss/09461679-9d98-4d75-aa72-9bb27ee87d3c/merged.m4b
[2025-08-19T00:06:56Z DEBUG lofty::probe] Probe: Opening `/var/folders/29/g7wmd39x0pb75l_n1hjrttcw0000gn/T/audiobook-boss/09461679-9d98-4d75-aa72-9bb27ee87d3c/merged.m4b` for reading
[2025-08-19T00:06:56Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mp4)` from extension
[2025-08-19T00:06:56Z DEBUG lofty::mp4::read] Verified to be an MP4 file. Major brand: M4A 
[2025-08-19T00:06:56Z DEBUG lofty::probe] Probe: Guessed file type: Some(Mp4)
[2025-08-19T00:06:56Z DEBUG lofty::mp4::ilst::write] Attempting to write `ilst` tag to file
[2025-08-19T00:06:56Z DEBUG lofty::mp4::read] Verified to be an MP4 file. Major brand: M4A 
[2025-08-19T00:06:56Z DEBUG lofty::mp4::ilst::write] Building `ilst` atom
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor::finalize] ✓ Basic metadata tags written successfully
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor::finalize] Attempting Lofty cover art embedding as fallback - 117639 bytes
[2025-08-19T00:06:56Z DEBUG lofty::probe] Probe: Opening `/var/folders/29/g7wmd39x0pb75l_n1hjrttcw0000gn/T/audiobook-boss/09461679-9d98-4d75-aa72-9bb27ee87d3c/merged.m4b` for reading
[2025-08-19T00:06:56Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mp4)` from extension
[2025-08-19T00:06:56Z DEBUG lofty::mp4::read] Verified to be an MP4 file. Major brand: M4A 
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::processor::finalize] Native cover art check: not found (file: /var/folders/29/g7wmd39x0pb75l_n1hjrttcw0000gn/T/audiobook-boss/09461679-9d98-4d75-aa72-9bb27ee87d3c/merged.m4b)
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor::finalize] Native cover art not detected - proceeding with Lofty fallback
[2025-08-19T00:06:56Z DEBUG lofty::probe] Probe: Opening `/var/folders/29/g7wmd39x0pb75l_n1hjrttcw0000gn/T/audiobook-boss/09461679-9d98-4d75-aa72-9bb27ee87d3c/merged.m4b` for reading
[2025-08-19T00:06:56Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mp4)` from extension
[2025-08-19T00:06:56Z DEBUG lofty::mp4::read] Verified to be an MP4 file. Major brand: M4A 
[2025-08-19T00:06:56Z DEBUG lofty::probe] Probe: Guessed file type: Some(Mp4)
[2025-08-19T00:06:56Z DEBUG lofty::mp4::ilst::write] Attempting to write `ilst` tag to file
[2025-08-19T00:06:56Z DEBUG lofty::mp4::read] Verified to be an MP4 file. Major brand: M4A 
[2025-08-19T00:06:56Z DEBUG lofty::mp4::ilst::write] Building `ilst` atom
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor::finalize] ✓ Lofty cover art fallback completed successfully
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor::finalize] ✓ Finalize stage metadata writing completed successfully
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor::finalize] 🚀 Starting complete_processing stage
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor::finalize] Temporary file: /var/folders/29/g7wmd39x0pb75l_n1hjrttcw0000gn/T/audiobook-boss/09461679-9d98-4d75-aa72-9bb27ee87d3c/merged.m4b
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor::finalize] Final output path: /Users/jstar/Downloads/ABB Tests/DRAGON TEST (2025).m4b
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor::finalize] Moving temporary file to final location...
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor::finalize] finalize_move method=rename status=ok dest=/Users/jstar/Downloads/ABB Tests/DRAGON TEST (2025).m4b
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor::finalize] ✓ File moved successfully to: /Users/jstar/Downloads/ABB Tests/DRAGON TEST (2025).m4b
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor::finalize] Cleaning up temporary directory...
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::processor::finalize] Cleaning up temporary directory for session 09461679-9d98-4d75-aa72-9bb27ee87d3c: /var/folders/29/g7wmd39x0pb75l_n1hjrttcw0000gn/T/audiobook-boss/09461679-9d98-4d75-aa72-9bb27ee87d3c
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::cleanup::guard] Creating cleanup guard for session: 09461679-9d98-4d75-aa72-9bb27ee87d3c
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::cleanup::guard] Session 09461679-9d98-4d75-aa72-9bb27ee87d3c: Adding path to cleanup: /var/folders/29/g7wmd39x0pb75l_n1hjrttcw0000gn/T/audiobook-boss/09461679-9d98-4d75-aa72-9bb27ee87d3c
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::cleanup::guard] Session 09461679-9d98-4d75-aa72-9bb27ee87d3c: Performing immediate cleanup of 1 paths
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::cleanup::ops] Session 09461679-9d98-4d75-aa72-9bb27ee87d3c: Removing directory: /var/folders/29/g7wmd39x0pb75l_n1hjrttcw0000gn/T/audiobook-boss/09461679-9d98-4d75-aa72-9bb27ee87d3c
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::cleanup::ops] Session 09461679-9d98-4d75-aa72-9bb27ee87d3c: All cleanup operations completed successfully
[2025-08-19T00:06:56Z DEBUG audiobook_boss_lib::audio::cleanup::guard] Session 09461679-9d98-4d75-aa72-9bb27ee87d3c: No paths to clean up
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor::finalize] ✓ Temporary directory cleaned up successfully
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor::finalize] 🎉 Successfully created audiobook: /Users/jstar/Downloads/ABB Tests/DRAGON TEST (2025).m4b
[2025-08-19T00:06:56Z INFO  audiobook_boss_lib::audio::processor] Processing Complete:
    - Files processed: 1
    - Audio duration: 9.93 hours
    - Data processed: 238.59 MB
    - Time elapsed: 14m 1s
    - Throughput: 0.28 MB/s
# Enhanced Encoder Mapping Guide

This note captures how the shrink.sh / opus.sh defaults map into the production encoder engine. It is intended for anyone maintaining the UI <-> Rust bridge or helping users install the required codecs (especially Fraunhofer FDK).

## Encoder presets and defaults

| UI selection        | Backend `EncoderType` | Bitrate mode                             | Channels (default) | Notes                                                                            |
| ------------------- | --------------------- | ---------------------------------------- | ------------------ | -------------------------------------------------------------------------------- |
| Auto                | `auto`                | VBR level 3 (prefers FDK)                | Auto (preserve)    | Prefers FDK → Apple → Native. Falls back to Native if nothing else is available. |
| Apple AAC (aac_at)  | `aac_at`              | CVBR (target = UI bitrate)               | Auto               | Uses `aac_at_mode=cvbr`, S16 sample format.                                      |
| FDK HE-AAC          | `fdk_he_aac`          | VBR (level slider, default 3)            | Auto               | Forces HE-AAC v1 profile and afterburner toggle.                                 |
| Native AAC (FFmpeg) | `native_aac`          | CBR                                      | Auto               | Twoloop coder enabled unless `ABB_DISABLE_TWOOLOOP=1`.                           |
| Opus (libopus)      | `opus`                | VBR (level slider exposed for info only) | Auto               | `compression_level=10`, `application=audio`. Target bitrate uses main dropdown.  |

- **Bitrate dropdown** feeds `bitrateKbps` for every encoder. For CVBR/CBR modes the selected value is the target bitrate. For VBR (FDK/Opus) it is used as an approximate bound and for UI messaging.
- **Channels dropdown** now offers `Auto` (preserve source), `Force Mono`, `Force Stereo`. Only the legacy validation path still requires 1/2; the runtime encoder honors Auto by probing the first input’s channel layout.
- **FDK controls** (quality slider + afterburner) are only visible when the encoder select is set to Fraunhofer. The slider writes `vbr.level` (1..5). Afterburner toggles the `afterburner` boolean sent to Rust.
- **Encoder availability hint** displays the result of `list_available_encoders` and disables unsupported options so the user understands why FDK/Apple/Opus might not be selectable.

## Command / type mapping

| UI field                            | Frontend type                    | Backend field                                           |
| ----------------------------------- | -------------------------------- | ------------------------------------------------------- |
| Encoder dropdown                    | `EncoderSettingsV2.flavor`       | `EncoderSettings.encoder_type` (via `toBoundary…`)      |
| Bitrate dropdown                    | `bitrateKbps`                    | `EncoderSettings.bitrate_kbps`                          |
| Bitrate mode select                 | `bitrateMode` (cbr/cvbr/vbr)     | `EncoderSettings.bitrate_mode`                          |
| Channel select                      | `channels` (`'auto' \| 1 \| 2`)  | `EncoderSettings.channels`                              |
| FDK slider / afterburner            | `vbr.level`, `fdkAfterburner`    | `EncoderSettings.bitrate_mode.vbr.level`, `afterburner` |
| Threads (future UI)                 | `threads`                        | `EncoderSettings.threads`                               |
| Encoder availability call-to-action | `list_available_encoders` result | Used to disable options + show guidance                 |

All UI state is serialized via `EncoderSettingsProvider` → `toBoundaryEncoderSettings` before invoking `process_audiobook_files_v2`.

## Installing FFmpeg with libfdk_aac (macOS)

Apple does not ship FDK, so the user must install an FFmpeg build that includes it:

1. **Homebrew (recommended)**

   ```bash
   brew tap homebrew-ffmpeg/ffmpeg
   brew install homebrew-ffmpeg/ffmpeg/ffmpeg --with-fdk-aac
   ```

   > Note: The default `brew install ffmpeg` omits FDK for licensing reasons. The tap above builds it from source.

2. **Manual build**

   ```bash
   git clone https://github.com/FFmpeg/FFmpeg.git
   cd FFmpeg
   ./configure --enable-libfdk_aac --enable-nonfree --enable-gpl
   make -j$(sysctl -n hw.ncpu)
   sudo make install
   ```

   Ensure `/usr/local/bin` (or the chosen prefix) is on the `PATH` so the app loads the correct libraries.

3. **Verification**  
   Run `ffmpeg -hide_banner -encoders | grep fdk` and confirm `AAC (Advanced Audio Coding) (codec aac)` lists `libfdk_aac`. The UI’s availability check (`list_available_encoders`) should now report “FDK detected”.

Opus support ships with the default Homebrew FFmpeg builds. If `list_available_encoders` reports `opus_available: false`, the user likely has an outdated binary and should reinstall with `brew install ffmpeg` (or rebuild with `--enable-libopus`).

## Troubleshooting checklist

- If the encoder dropdown option is disabled, hover the availability hint to see the reason (e.g., “FDK missing”). The text mirrors the `EncoderAvailability` flags from Rust.
- Apple AAC requires macOS and fails gracefully on Windows/Linux; the UI disables the option when `aac_at` is unavailable.
- “Auto” mode always sends `encoder_type='auto'` + `bitrate_mode.vbr`. The backend resolves to the best available encoder and logs the fallback chain (`encoder fallback: requested=FdkHeAac availability=…`).

Keep this document in sync with any future encoder changes so other agents can quickly map UI behavior to backend expectations.

# Audiobook Encoder Script

**Purpose**: Full audiobook encoding with testing capabilities for optimal compression settings.

**Goal**: Maximum file size reduction with minimal quality loss for speech content.

**Capability**: Handles complete audiobooks (20+ hours) AND provides testing tools.

## Quick Start

```bash
# Batch encode all audiobooks in directory
./protocoder.sh native medium

# Encode single specific file
./protocoder.sh native medium audiobook.mp3

# Test samples of all files
SAMPLE=1 ./protocoder.sh fdk low

# Test all encoders on specific file
SAMPLE=1 ./protocoder.sh test audiobook.mp3
```

## Core Concepts

### Encoders

**FDK (libfdk_aac)**: HE-AAC with SBR technology. Better compression at low bitrates.
- Uses CBR (Constant Bitrate)
- Requires special FFmpeg build

**Native**: Standard AAC included with FFmpeg.
- Uses VBR (Variable Bitrate) by default
- Available everywhere

### Quality Trade-offs

| Quality | Use Case | FDK CBR | Native VBR | File Size |
|---------|----------|---------|------------|-----------|
| `low` | Lectures, single speaker | 48k | ~48-56k | Smallest |
| `medium` | General audiobooks | 56k | ~64-72k | Balanced |
| `high` | Music, sound effects | 64k | ~80-96k | Larger |

## Common Scenarios

### "I want to encode all my audiobooks with smallest file size"

```bash
./protocoder.sh fdk low
```
Batch encodes ALL audio files in directory. Produces ~48k CBR with HE-AAC. Best for pure speech.

### "I want to encode one specific audiobook"

```bash
./protocoder.sh fdk low specific_book.mp3
```
Encodes only the specified file.

### "I want to test quality on all my files"

```bash
SAMPLE=1 ./protocoder.sh native medium
```
Creates 30-second samples of ALL audio files for listening tests.

### "I want to test quality on one specific file"

```bash
SAMPLE=1 ./protocoder.sh native medium audiobook.mp3
```
Creates 30-second sample of specific file for listening tests.

### "I want to compare everything quickly"

```bash
SAMPLE=1 ./protocoder.sh test audiobook.mp3
```
Generates 9 × 30-second samples for quick quality comparison.

### "I want to compare everything thoroughly"

```bash
./protocoder.sh test audiobook.mp3
```
Generates 9 COMPLETE versions of your file.
**Warning**: This encodes the full file 9 times.

### "I need consistent bitrate for streaming"

```bash
VBR=0 ./protocoder.sh native medium audiobook.mp3
```
Forces CBR mode for native encoder.

### "I want to see what command will run"

```bash
PREVIEW=1 ./protocoder.sh fdk high audiobook.mp3
```
Displays FFmpeg command without executing.

## Output Files

Files are named with encoding details:
- `book_fdk_medium.m4b` - FDK CBR 56k
- `book_native_vbr_low.m4b` - Native VBR q=0.5
- `book_native_cbr_low.m4b` - Native CBR 56k
- `book_native_vbr_sample_medium.m4b` - 30-second test

## Environment Variables

| Variable | Values | Purpose |
|----------|--------|---------|
| `SAMPLE` | 0/1 | Encode 30 seconds only |
| `PREVIEW` | 0/1 | Show command without running |
| `VBR` | 0/1 | Native AAC mode (1=VBR default) |
| `AFTERBURNER` | 0/1 | FDK quality boost (1=on default) |

## Warnings

### FDK Availability

Personal dev note (macOS/Homebrew): you may need an ffmpeg build that includes libfdk_aac; exact steps depend on the tap/build.
```bash
# Example (may vary with Homebrew changes):
brew install ffmpeg --with-fdk-aac
```
If unavailable, the script falls back to native AAC.

### VBR File Sizes

VBR produces variable file sizes. Actual bitrate depends on content complexity.

### Mono Conversion

Script forces mono output. Stereo effects will be lost.

## Technical Details

### Frequency Cutoffs

**FDK**: 18kHz (SBR reconstructs higher frequencies)

**Native**: 15-17kHz (must encode all frequencies directly)

### Encoding Profiles

FDK uses `aac_he` profile with afterburner.

Native uses `twoloop` coder for better low-bitrate quality.

## Troubleshooting

### "libfdk_aac not available"

Install with: `brew install ffmpeg --with-fdk-aac`

Or use native: `./protocoder.sh native medium file.mp3`

### "File not found"

Check path and extension. Script accepts `.mp3`, `.m4a`, `.m4b`.

### Large output files

Check if using CBR instead of VBR: `VBR=1 ./protocoder.sh native ...`

## Logs

Encoding details saved to `./logs/encode_YYYYMMDD_HHMMSS.log`.

Contains FFmpeg output and performance metrics.

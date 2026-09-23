# Opus build provenance

ABB uses FFmpeg's libopus encoder and statically links the platform's libopus
archive. Setup installs `opus` on macOS or `libopus-dev` on Linux. The bundled
FFmpeg build records the package version and archive content hash in
`abb-build-inputs`; replacing the archive invalidates the cache.

Upstream: https://opus-codec.org/ and https://github.com/xiph/opus.
The accompanying BSD license is from libopus 1.6.1, verified in the macOS
implementation lane. Opus's patent grants are at https://opus-codec.org/license/.
The linked library's license and this provenance file ship in app resources.
No Opus implementation source is copied into ABB.

Developer builds use the installed archive. Release builders must provide an
archive compatible with the release deployment target and architecture; the
app's existing portability inspection remains required. M4A and Matroska
playback compatibility is a separate client concern from libopus availability.

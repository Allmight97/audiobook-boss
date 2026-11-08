#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquisitionStrategy {
    DownloadImportReady,
    DownloadThenDecryptAax,
    DownloadThenDecryptAaxc,
    DownloadThenDecryptDash,
    ProtectedUnsupported,
    ProviderProtocolFailed,
}

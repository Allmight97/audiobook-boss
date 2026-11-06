import type { AcquisitionProgress, RemoteTitle } from '../../types/remoteSource';

export function formatReleaseSizeBytes(sizeBytes: number): string {
	if (sizeBytes <= 0) return 'Unknown size';
	const mb = sizeBytes / (1024 * 1024);
	if (mb >= 1024) {
		return `${(mb / 1024).toFixed(1)} GB`;
	}
	return `${mb.toFixed(1)} MB`;
}

export function releaseProtocolLabel(protocol: 'usenet' | 'torrent' | 'unknown'): string {
	switch (protocol) {
		case 'usenet':
			return 'nzb';
		case 'torrent':
			return 'torrent';
		default:
			return 'unknown';
	}
}

export function bytesLabel(progress: AcquisitionProgress): string | null {
	if (progress.bytesDownloaded == null) return null;
	const downloadedMb = progress.bytesDownloaded / (1024 * 1024);
	if (progress.bytesTotal == null || progress.bytesTotal <= 0) {
		return `${downloadedMb.toFixed(1)} MB downloaded`;
	}
	const totalMb = progress.bytesTotal / (1024 * 1024);
	return `${downloadedMb.toFixed(1)} / ${totalMb.toFixed(1)} MB`;
}

export function progressTitleLabel(
	progress: AcquisitionProgress,
	title: RemoteTitle | undefined,
): string {
	const ordinal =
		progress.currentItemIndex != null && progress.totalItems != null
			? `${progress.currentItemIndex}/${progress.totalItems}`
			: null;
	const titleLabel = title?.title ?? (progress.currentTitleId ? 'Selected title' : null);
	const context = [ordinal, titleLabel].filter(Boolean).join(' - ');
	if (!context) return progress.message;
	return `${progress.message.replace(/\.$/, '')}: ${context}`;
}

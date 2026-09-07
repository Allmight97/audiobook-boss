import type {
	AcquisitionJob,
	AcquisitionProgress,
	RemoteTitle,
	RemoteTitleAvailability,
} from '../../types/remoteSource';

export type AcquisitionJobWithProgress = AcquisitionJob & {
	progress?: AcquisitionProgress;
};

export type RemoteSourceDiagnostic = AcquisitionJob['diagnostics'][number];

export const acquisitionPollDelayMs = 100;

export function isAcquisitionTerminal(job: AcquisitionJobWithProgress): boolean {
	return (
		job.progress?.terminal === true ||
		job.status === 'failed' ||
		job.status === 'cancelled' ||
		job.status === 'validated' ||
		job.status === 'importedToFileList'
	);
}

export function uniqueDiagnosticMessage(diagnostics: RemoteSourceDiagnostic[]): string {
	const uniqueMessages: string[] = [];
	const seenMessages = new Set<string>();
	for (const diagnostic of diagnostics) {
		const message = diagnostic.message.trim();
		if (!message || seenMessages.has(message)) continue;
		seenMessages.add(message);
		uniqueMessages.push(message);
	}
	return uniqueMessages.join(' ');
}

export function statusFromAcquisitionJob(job: AcquisitionJobWithProgress): string {
	const diagnostics = uniqueDiagnosticMessage(job.diagnostics);
	if (job.progress?.terminal && diagnostics) return diagnostics;
	if (job.progress?.message) return job.progress.message;
	return diagnostics || 'Audible acquisition is running.';
}

export function progressPercent(job: AcquisitionJobWithProgress): number {
	return Math.max(0, Math.min(100, job.progress?.percentage ?? 0));
}

export function withClearedHandoffJob(job: AcquisitionJobWithProgress): AcquisitionJobWithProgress {
	return {
		...job,
		materializedFiles: [],
		supplementalAssets: [],
	};
}

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

export function progressTitleLabel(progress: AcquisitionProgress, titles: RemoteTitle[]): string {
	const title = titles.find((candidate) => candidate.titleId === progress.currentTitleId);
	const ordinal =
		progress.currentItemIndex != null && progress.totalItems != null
			? `${progress.currentItemIndex}/${progress.totalItems}`
			: null;
	const titleLabel = title?.title ?? (progress.currentTitleId ? 'Selected title' : null);
	const context = [ordinal, titleLabel].filter(Boolean).join(' - ');
	if (!context) return progress.message;
	return `${progress.message.replace(/\.$/, '')}: ${context}`;
}

export function titleAvailability(title: RemoteTitle): RemoteTitleAvailability {
	return (
		title.availability ?? {
			status: title.unsupportedReasons.length > 0 ? 'providerUnavailable' : 'available',
			acquirable: title.unsupportedReasons.length === 0,
			label: title.unsupportedReasons.length > 0 ? 'Unavailable from Audible' : 'Available',
			detail:
				title.unsupportedReasons.length > 0
					? 'Audible reports this title is not playable or downloadable for this account.'
					: undefined,
		}
	);
}

export function isTitleAcquirable(title: RemoteTitle): boolean {
	return titleAvailability(title).acquirable;
}

import { toUserMessage } from '../../lib/tauri/appError';
import type { AcquisitionJob, AcquisitionProgress, RemoteTitle } from '../../types/remoteSource';

export type AcquisitionJobWithProgress = AcquisitionJob & {
	progress?: AcquisitionProgress;
};

export type RemoteSourceDiagnostic = AcquisitionJob['diagnostics'][number];

export function isAcquisitionTerminal(job: AcquisitionJobWithProgress): boolean {
	return (
		job.progress?.terminal === true ||
		job.status === 'failed' ||
		job.status === 'cancelled' ||
		job.status === 'validated' ||
		job.status === 'importedToFileList'
	);
}

/** A job is settled once it failed, was cancelled, or its files were handed to the session. */
export function isAcquisitionSettled(job: AcquisitionJobWithProgress): boolean {
	if (job.handoff) return true;
	if (job.status === 'failed' || job.status === 'cancelled') return true;
	return isAcquisitionTerminal(job) && job.materializedFiles.length === 0;
}

const STAGED_FILES_REMOVED_SUFFIX =
	'Staged remote files were removed; retry acquisition after processing completes.';

/** Words how a settled job's files reached the session; `null` when it never got that far. */
export function handoffMessage(job: AcquisitionJobWithProgress): string | null {
	const handoff = job.handoff;
	if (!handoff) {
		if (job.status === 'failed' || job.status === 'cancelled') return null;
		return (
			uniqueDiagnosticMessage(job.diagnostics) ||
			'Audible acquisition did not materialize an importable file.'
		);
	}
	if (handoff.kind === 'imported') {
		return `${handoff.count} acquired title${handoff.count === 1 ? '' : 's'} imported.`;
	}
	const reason = handoff.reason;
	switch (reason.kind) {
		case 'orderLocked':
			return `Order locked while processing. Wait for completion to add files. ${STAGED_FILES_REMOVED_SUFFIX}`;
		case 'importFailed':
			return `${toUserMessage(reason.error)} ${STAGED_FILES_REMOVED_SUFFIX}`;
		case 'nothingAdded':
			return `Acquired titles were not added to the input session. ${STAGED_FILES_REMOVED_SUFFIX}`;
	}
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

export function isTitleAcquirable(title: RemoteTitle): boolean {
	return title.availability.acquirable;
}

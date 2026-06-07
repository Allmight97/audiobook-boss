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
	titles: RemoteTitle[],
): string {
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
	return title.availability ?? {
		status: title.unsupportedReasons.length > 0 ? 'providerUnavailable' : 'available',
		acquirable: title.unsupportedReasons.length === 0,
		label: title.unsupportedReasons.length > 0 ? 'Unavailable from Audible' : 'Available',
		detail:
			title.unsupportedReasons.length > 0
				? 'Audible reports this title is not playable or downloadable for this account.'
				: undefined,
	};
}

export function isTitleAcquirable(title: RemoteTitle): boolean {
	return titleAvailability(title).acquirable;
}

export function filterTitles(
	titles: RemoteTitle[],
	filters: {
		titleFilter: string;
		showSupplementalPdfOnly: boolean;
		hideUnavailableTitles: boolean;
	},
): RemoteTitle[] {
	const normalizedFilter = filters.titleFilter.trim().toLowerCase();
	let facetTitles = filters.showSupplementalPdfOnly
		? titles.filter((title) => title.supplementalPdfAvailable)
		: titles;
	if (filters.hideUnavailableTitles) {
		facetTitles = facetTitles.filter(isTitleAcquirable);
	}
	if (!normalizedFilter) return facetTitles;
	return facetTitles.filter((title) =>
		[title.title, title.authors.join(' '), title.narrators.join(' ')]
			.join(' ')
			.toLowerCase()
			.includes(normalizedFilter),
	);
}

export function countSelectedOutsideFilter(
	titles: RemoteTitle[],
	selectedTitleIds: Set<string>,
	filteredTitles: RemoteTitle[],
): number {
	if (!selectedTitleIds.size) return 0;
	if (!titles.length) return 0;
	if (!filteredTitles.length) return selectedTitleIds.size;
	const visibleTitleIds = new Set(filteredTitles.map((title) => title.titleId));
	return [...selectedTitleIds].filter((titleId) => !visibleTitleIds.has(titleId)).length;
}

export function selectedTitleSummary(selectedTitleIds: Set<string>, hiddenCount: number): string {
	const count = selectedTitleIds.size;
	if (count === 0) return '0 selected';
	const titleLabel = count === 1 ? 'title' : 'titles';
	if (hiddenCount === 0) return `${count} ${titleLabel} selected`;
	const hiddenLabel = hiddenCount === 1 ? 'title' : 'titles';
	return `${count} ${titleLabel} selected (${hiddenCount} ${hiddenLabel} hidden by filter)`;
}

export function delay(ms: number): Promise<void> {
	return new Promise((resolve) => window.setTimeout(resolve, ms));
}

export function toggleTitleSelection(
	titles: Set<string>,
	title: RemoteTitle,
): Set<string> {
	const next = new Set(titles);
	if (next.has(title.titleId)) {
		next.delete(title.titleId);
		return next;
	}
	next.add(title.titleId);
	return next;
}

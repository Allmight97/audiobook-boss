import type { RemoteTitle } from '../../types/remoteSource';
import { isTitleAcquirable } from './remoteSourceAcquireDialogHelpers';

export type RemoteTitleFilterOptions = {
	titleFilter: string;
	showSupplementalPdfOnly: boolean;
	hideUnavailableTitles: boolean;
};

export function visibleRemoteTitles(
	titles: RemoteTitle[],
	options: RemoteTitleFilterOptions,
): RemoteTitle[] {
	const normalizedFilter = options.titleFilter.trim().toLowerCase();
	let facetTitles = options.showSupplementalPdfOnly
		? titles.filter((title) => title.supplementalPdfAvailable)
		: titles;

	if (options.hideUnavailableTitles) {
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

export function toggledRemoteTitleSelection(
	selectedTitleIds: ReadonlySet<string>,
	title: RemoteTitle,
): Set<string> {
	if (!isTitleAcquirable(title)) return new Set(selectedTitleIds);

	const next = new Set(selectedTitleIds);
	if (next.has(title.titleId)) {
		next.delete(title.titleId);
	} else {
		next.add(title.titleId);
	}
	return next;
}

export function toggledSupplementalPdfPreference(
	includePdfByTitleId: Readonly<Record<string, boolean>>,
	titleId: string,
): Record<string, boolean> {
	return {
		...includePdfByTitleId,
		[titleId]: !includePdfByTitleId[titleId],
	};
}

export function selectedRemoteTitleSummaryText(
	selectedTitleIds: ReadonlySet<string>,
	visibleTitles: RemoteTitle[],
): string {
	const count = selectedTitleIds.size;
	if (count === 0) return '0 selected';

	const visibleIds = new Set(visibleTitles.map((title) => title.titleId));
	const hiddenCount = [...selectedTitleIds].filter((id) => !visibleIds.has(id)).length;
	const titleLabel = count === 1 ? 'title' : 'titles';
	if (hiddenCount === 0) return `${count} ${titleLabel} selected`;

	const hiddenLabel = hiddenCount === 1 ? 'title' : 'titles';
	return `${count} ${titleLabel} selected (${hiddenCount} ${hiddenLabel} hidden by filter)`;
}

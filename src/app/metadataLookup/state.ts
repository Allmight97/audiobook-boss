import type {
	LookupApplyMode,
	LookupSnapshot,
	LookupStatus,
	MetadataSource,
	OnlineMetadataResult,
	QueueStep,
} from '../../lib/generated/tauri';

type MetadataLookupStatusVariant = 'error' | 'success' | 'info';
export type MetadataLookupSource = 'auto' | MetadataSource;

export type MetadataLookupState = {
	isOpen: boolean;
	titleQuery: string;
	authorQuery: string;
	source: MetadataLookupSource;
	applyMode: LookupApplyMode;
	replaceCoverArt: boolean;
	statusMessage: string;
	statusVariant: MetadataLookupStatusVariant;
	queueContext: string;
	results: OnlineMetadataResult[];
	isQueueMode: boolean;
	hasSearched: boolean;
};

type Status = { message: string; variant: MetadataLookupStatusVariant };

function afterStep(step: QueueStep | null): string {
	switch (step) {
		case null:
			return '';
		case 'applied':
			return 'Metadata applied. ';
		case 'appliedWithoutCover':
			return 'Metadata applied, but cover art failed to load. ';
		case 'skipped':
			return 'Skipped. ';
	}
}

const error = (message: string): Status => ({ message, variant: 'error' });

type FixedKind = Exclude<
	LookupStatus['kind'],
	'found' | 'searchFailed' | 'queryRequired' | 'nextTitleRejected' | 'applied' | 'queueComplete'
>;

const FIXED_STATUS: Record<FixedKind, Status> = {
	noValidTitle: error('Select a valid file to search metadata.'),
	searching: { message: 'Searching metadata sources…', variant: 'info' },
	searchSuperseded: error('Search stopped because it changed. Search again.'),
	applying: { message: 'Applying metadata…', variant: 'info' },
	applySuperseded: error('Apply stopped because the search changed. Apply again.'),
	noTitleQueued: error('Select at least one file before applying metadata.'),
	applyRejected: error(
		'Could not apply metadata to the selected file. Review pending edits and try again.',
	),
};

function statusOf(status: LookupStatus | null): Status {
	if (status === null) return { message: '', variant: 'info' };
	switch (status.kind) {
		case 'found': {
			const found = status.partial
				? `Found ${status.count} results. Some lookup data was unavailable; showing available results.`
				: `Found ${status.count} results.`;
			return {
				message: `${afterStep(status.after)}${found}`,
				variant:
					status.after === 'appliedWithoutCover' ? 'error' : status.partial ? 'info' : 'success',
			};
		}
		case 'queryRequired':
			return error(`${afterStep(status.after)}Enter a title, author, or ASIN to search.`);
		case 'nextTitleRejected':
			return error(
				`${status.coverFailed ? 'Cover art failed to load. ' : ''}Could not select the next file. Review pending metadata edits and try again.`,
			);
		case 'searchFailed':
			return error(`${afterStep(status.after)}Search failed. Check your query and try again.`);
		case 'applied':
			return status.coverFailed
				? error('Metadata applied to form, but cover art failed to load.')
				: { message: 'Metadata applied to form.', variant: 'success' };
		case 'queueComplete':
			return status.coverFailed
				? error('Queue complete, but cover art failed to load.')
				: { message: 'Queue complete.', variant: 'success' };
		default:
			return FIXED_STATUS[status.kind];
	}
}

export function toLookupState(
	lookup: LookupSnapshot,
	typed: { readonly titleQuery?: string; readonly authorQuery?: string },
): MetadataLookupState {
	const status = statusOf(lookup.status);
	const position = lookup.queuePosition;
	return {
		isOpen: lookup.open,
		titleQuery: typed.titleQuery ?? lookup.titleQuery,
		authorQuery: typed.authorQuery ?? lookup.authorQuery,
		source: lookup.source,
		applyMode: lookup.applyMode,
		replaceCoverArt: lookup.replaceCover,
		statusMessage: status.message,
		statusVariant: status.variant,
		queueContext: position
			? `${position.index + 1} of ${position.total} • ${position.label}`
			: 'No files selected.',
		results: lookup.results,
		isQueueMode: lookup.isQueueMode,
		hasSearched: lookup.hasSearched,
	};
}

import { pathBasename } from '../../lib/path/basename';
import type { MetadataSource, OnlineMetadataResult } from '../../types/metadata';
import type { LookupStatus, QueueStep, SessionLookup } from '../../types/session';

type MetadataLookupStatusVariant = 'error' | 'success' | 'info';
export type MetadataLookupApplyMode = 'current' | 'queue';
export type MetadataLookupSource = 'auto' | MetadataSource;

/** The lookup dialog as the view renders it. */
export type MetadataLookupState = {
	isOpen: boolean;
	titleQuery: string;
	authorQuery: string;
	source: MetadataLookupSource;
	applyMode: MetadataLookupApplyMode;
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

/** Words the engine's account of the last lookup action. */
function statusOf(status: LookupStatus | null): Status {
	const error = (message: string): Status => ({ message, variant: 'error' });
	switch (status?.kind) {
		case undefined:
			return { message: '', variant: 'info' };
		case 'noValidTitle':
			return error('Select a valid file to search metadata.');
		case 'queryRequired':
			return error('Enter a title, author, or ASIN to search.');
		case 'searching':
			return { message: 'Searching metadata sources…', variant: 'info' };
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
		case 'searchFailed':
			return error(`${afterStep(status.after)}Search failed. Check your query and try again.`);
		case 'noTitleQueued':
			return error('Select at least one file before applying metadata.');
		case 'applyRejected':
			return error(
				'Could not apply metadata to the selected file. Review pending edits and try again.',
			);
		case 'applied':
			return status.coverFailed
				? error('Metadata applied to form, but cover art failed to load.')
				: { message: 'Metadata applied to form.', variant: 'success' };
		case 'queueComplete':
			return status.coverFailed
				? error('Queue complete, but cover art failed to load.')
				: { message: 'Queue complete.', variant: 'success' };
		case 'nextTitleRejected':
			return error('Could not select the next file. Review pending metadata edits and try again.');
		case 'failed':
			return error('Metadata lookup failed. Check console and try again.');
	}
}

/**
 * `typed` holds query text the user has entered that the engine has not
 * confirmed yet.
 */
export function toLookupState(
	lookup: SessionLookup,
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
			? `${position.index + 1} of ${position.total} • ${pathBasename(position.path, { fallback: 'path' })}`
			: 'No files selected.',
		results: lookup.results,
		isQueueMode: lookup.isQueueMode,
		hasSearched: lookup.hasSearched,
	};
}

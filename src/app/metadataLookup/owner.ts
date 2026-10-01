import { createSignal, type Accessor } from 'solid-js';
import type { EngineLink } from '../engineLink';
import type { MetadataOwner } from '../metadataSession';
import {
	createCoverArtPreviewScheduler,
	type CoverArtPreviewState,
} from '../../lib/media/coverArtPreviewScheduler';
import type { SessionIntent } from '../../types/session';
import {
	toLookupState,
	type MetadataLookupApplyMode,
	type MetadataLookupSource,
	type MetadataLookupState,
} from './state';

export type MetadataLookupAction =
	| { type: 'applyResult'; index: number }
	| { type: 'close' }
	| { type: 'manualEntry' }
	| { type: 'open' }
	| { type: 'search' }
	| { type: 'skipQueueItem' };

/**
 * The engine owns the lookup: its queue, searches, and applying a result.
 * This owner shows the engine's snapshot, sends intents, and loads the
 * result thumbnails the dialog displays.
 */
export type MetadataLookupOwner = {
	readonly view: Accessor<MetadataLookupState>;
	coverPreview(coverUrl: string | null | undefined): CoverArtPreviewState;
	scheduleCoverPreviews(coverUrls: ReadonlyArray<string | null | undefined>): void;
	cancelCoverPreviews(): void;
	run(action: MetadataLookupAction): Promise<void>;
	setTitleQuery(value: string): void;
	setAuthorQuery(value: string): void;
	setSource(value: MetadataLookupSource): void;
	setApplyMode(value: MetadataLookupApplyMode): void;
	setReplaceCover(value: boolean): void;
	reset(): void;
};

const METADATA_TITLE_INPUT_ID = 'meta-title';

function intentFor(action: MetadataLookupAction): SessionIntent {
	switch (action.type) {
		case 'applyResult':
			return { kind: 'lookupApply', index: action.index };
		case 'close':
		case 'manualEntry':
			return { kind: 'lookupClose' };
		case 'open':
			return { kind: 'lookupOpen' };
		case 'search':
			return { kind: 'lookupSearch' };
		case 'skipQueueItem':
			return { kind: 'lookupSkip' };
	}
}

export function createMetadataLookupOwner(deps: {
	readonly link: EngineLink;
	readonly metadata: Pick<MetadataOwner, 'capability'>;
}): MetadataLookupOwner {
	const { link } = deps;
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	const [previewRev, bumpPreviews] = createSignal(0, { ownedWrite: true });
	// Query text entered and not yet confirmed by the engine.
	const typed: { titleQuery?: { value: string }; authorQuery?: { value: string } } = {};
	const previews = createCoverArtPreviewScheduler({
		load: (url) => deps.metadata.capability().loadCoverArtFromUrl(url),
		onChange: () => bumpPreviews((revision) => revision + 1),
		failureLogMessage: 'Failed to load metadata lookup cover preview:',
	});

	function changed(): void {
		bump((n) => n + 1);
	}

	function setQuery(key: 'titleQuery' | 'authorQuery', value: string): void {
		const entry = { value };
		typed[key] = entry;
		changed();
		link
			.send(
				key === 'titleQuery'
					? { kind: 'lookupSetTitleQuery', value }
					: { kind: 'lookupSetAuthorQuery', value },
			)
			.catch((error: unknown) => console.error('Failed to record the lookup query:', error))
			.finally(() => {
				if (typed[key] !== entry) return;
				delete typed[key];
				changed();
			});
	}

	return {
		view: () => {
			rev();
			return toLookupState(link.lookup(), {
				titleQuery: typed.titleQuery?.value,
				authorQuery: typed.authorQuery?.value,
			});
		},
		coverPreview(coverUrl) {
			previewRev();
			return previews.getState(coverUrl);
		},
		scheduleCoverPreviews(coverUrls) {
			previews.schedule(coverUrls);
		},
		cancelCoverPreviews() {
			previews.cancel();
		},
		async run(action) {
			try {
				await link.send(intentFor(action));
			} catch (error) {
				console.error('Metadata lookup failed:', error);
				return;
			}
			if (action.type === 'manualEntry') {
				queueMicrotask(() => document.getElementById(METADATA_TITLE_INPUT_ID)?.focus());
			}
		},
		setTitleQuery(value) {
			setQuery('titleQuery', value);
		},
		setAuthorQuery(value) {
			setQuery('authorQuery', value);
		},
		setSource(source) {
			link.post({ kind: 'lookupSetSource', source });
		},
		setApplyMode(mode) {
			link.post({ kind: 'lookupSetApplyMode', mode });
		},
		setReplaceCover(replace) {
			link.post({ kind: 'lookupSetReplaceCover', replace });
		},
		reset() {
			previews.clear();
			delete typed.titleQuery;
			delete typed.authorQuery;
			changed();
		},
	};
}

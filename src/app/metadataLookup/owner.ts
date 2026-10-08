import { createSignal, type Accessor } from 'solid-js';
import type { EngineLink } from '../engineLink';
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
 * This owner shows the engine's snapshot and sends intents.
 */
export type MetadataLookupOwner = {
	readonly view: Accessor<MetadataLookupState>;
	run(action: MetadataLookupAction): Promise<void>;
	setTitleQuery(value: string): void;
	setAuthorQuery(value: string): void;
	setSource(value: MetadataLookupSource): void;
	setApplyMode(value: MetadataLookupApplyMode): void;
	setReplaceCover(value: boolean): void;
	reset(): void;
};

const METADATA_TITLE_INPUT_ID = 'meta-title';

function intentFor(action: MetadataLookupAction, lookupRevision: number): SessionIntent {
	switch (action.type) {
		case 'applyResult':
			return { kind: 'lookupApply', index: action.index, revision: lookupRevision };
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
}): MetadataLookupOwner {
	const { link } = deps;
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	// Query text entered and not yet confirmed by the engine.
	type QueryEcho = { value: string; path: string | undefined; binding: number };
	const typed: { titleQuery?: QueryEcho; authorQuery?: QueryEcho } = {};

	function changed(): void {
		bump((n) => n + 1);
	}

	function setQuery(key: 'titleQuery' | 'authorQuery', value: string): void {
		const entry = {
			value,
			path: link.lookup().queuePosition?.path,
			binding: link.metadata().binding,
		};
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
			const lookup = link.lookup();
			const echo = (entry: QueryEcho | undefined) =>
				entry?.path === lookup.queuePosition?.path && entry?.binding === link.metadata().binding
					? entry?.value
					: undefined;
			return toLookupState(lookup, {
				titleQuery: echo(typed.titleQuery),
				authorQuery: echo(typed.authorQuery),
			});
		},
		async run(action) {
			try {
				await link.send(intentFor(action, link.lookup().revision));
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
			delete typed.titleQuery;
			delete typed.authorQuery;
			changed();
		},
	};
}

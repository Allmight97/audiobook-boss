import { createSignal, type Accessor } from 'solid-js';
import { createTypingEcho, type EngineLink } from '../engineLink';
import type { LookupApplyMode, SessionIntent } from '../../lib/generated/tauri';
import { toLookupState, type MetadataLookupSource, type MetadataLookupState } from './state';

type MetadataLookupAction =
	| { type: 'applyResult'; index: number }
	| { type: 'close' }
	| { type: 'manualEntry'; afterClose: () => void }
	| { type: 'open' }
	| { type: 'search' }
	| { type: 'skipQueueItem' };

export type MetadataLookupOwner = {
	readonly view: Accessor<MetadataLookupState>;
	run: (action: MetadataLookupAction) => Promise<void>;
	setTitleQuery: (value: string) => void;
	setAuthorQuery: (value: string) => void;
	setSource: (value: MetadataLookupSource) => void;
	setApplyMode: (value: LookupApplyMode) => void;
	setReplaceCover: (value: boolean) => void;
	reset: () => void;
};

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
	function changed(): void {
		bump((n) => n + 1);
	}
	const typed = createTypingEcho<'titleQuery' | 'authorQuery'>({
		scopeOf: () => [link.lookup().queuePosition?.path, link.metadata().binding],
		changed,
	});

	function setQuery(key: 'titleQuery' | 'authorQuery', value: string): void {
		const intent: SessionIntent =
			key === 'titleQuery'
				? { kind: 'lookupSetTitleQuery', value }
				: { kind: 'lookupSetAuthorQuery', value };
		typed.show(key, value, () =>
			link.send(intent).catch((error: unknown) => link.refuse(intent.kind, error)),
		);
	}

	return {
		view: () => {
			rev();
			return toLookupState(link.lookup(), {
				titleQuery: typed.valueOf('titleQuery'),
				authorQuery: typed.valueOf('authorQuery'),
			});
		},
		async run(action) {
			const intent = intentFor(action, link.lookup().revision);
			try {
				await link.send(intent);
			} catch (error) {
				link.refuse(intent.kind, error);
				return;
			}
			if (action.type === 'manualEntry') {
				queueMicrotask(action.afterClose);
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
			typed.clear();
		},
	};
}

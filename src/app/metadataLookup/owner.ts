import { createSignal, type Accessor } from 'solid-js';
import type { InputOwner } from '../inputSession';
import type { MetadataOwner } from '../metadataSession';
import {
	createMetadataLookupCoverPreviews,
	type MetadataLookupCoverPreviewState,
} from './coverPreview';
import { makeProductionLookupServices } from './services';
import {
	createMetadataLookupQueueState,
	createMetadataLookupState,
	snapshotMetadataLookupState,
	type MetadataLookupApplyMode,
	type MetadataLookupSource,
	type MetadataLookupState,
} from './state';
import {
	makeMetadataLookupWorkflowServicesLayer,
	runMetadataLookupWorkflow,
	type MetadataLookupWorkflowAction,
} from './workflow';

export type MetadataLookupOwner = {
	readonly view: Accessor<MetadataLookupState>;
	coverPreview(coverUrl: string | null | undefined): MetadataLookupCoverPreviewState;
	scheduleCoverPreviews(coverUrls: ReadonlyArray<string | null | undefined>): void;
	cancelCoverPreviews(): void;
	run(action: MetadataLookupWorkflowAction): Promise<void>;
	setTitleQuery(value: string): void;
	setAuthorQuery(value: string): void;
	setSource(value: MetadataLookupSource): void;
	setApplyMode(value: MetadataLookupApplyMode): void;
	setReplaceCover(value: boolean): void;
	reset(): void;
};

export function createMetadataLookupOwner(deps: {
	readonly input: InputOwner;
	readonly metadata: MetadataOwner;
}): MetadataLookupOwner {
	const lookupState = createMetadataLookupState();
	const queueState = createMetadataLookupQueueState();
	let pendingRequest: AbortController | undefined;
	let snapshot = snapshotMetadataLookupState(lookupState);
	const [viewRev, bumpView] = createSignal(0, { ownedWrite: true });
	const [previewRev, bumpPreviews] = createSignal(0, { ownedWrite: true });
	const previews = createMetadataLookupCoverPreviews({
		loadCoverArtFromUrl: (url) => deps.metadata.capability().loadCoverArtFromUrl(url),
		onChange: () => bumpPreviews((revision) => revision + 1),
	});

	function publish(): void {
		snapshot = snapshotMetadataLookupState(lookupState);
		bumpView((n) => n + 1);
	}

	function services(signal: AbortSignal) {
		return makeProductionLookupServices(
			{
				input: deps.input,
				metadata: deps.metadata,
				lookupState,
				queueState,
				coverPreviews: previews,
				signal,
			},
			publish,
		);
	}

	return {
		view: () => {
			viewRev();
			return snapshot;
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
			pendingRequest?.abort();
			const request = new AbortController();
			pendingRequest = request;
			const layer = makeMetadataLookupWorkflowServicesLayer(services(request.signal));
			try {
				await runMetadataLookupWorkflow(layer, action);
				if (!request.signal.aborted) publish();
			} catch (error) {
				if (request.signal.aborted) return;
				console.error('Metadata lookup failed:', error);
				publish();
			} finally {
				if (pendingRequest === request) pendingRequest = undefined;
			}
		},
		setTitleQuery(value) {
			lookupState.titleQuery = value;
			publish();
		},
		setAuthorQuery(value) {
			lookupState.authorQuery = value;
			publish();
		},
		setSource(value) {
			lookupState.source = value;
			publish();
		},
		setApplyMode(value) {
			lookupState.applyMode = value;
			publish();
		},
		setReplaceCover(value) {
			lookupState.replaceCoverArt = value;
			publish();
		},
		reset() {
			pendingRequest?.abort();
			pendingRequest = undefined;
			previews.clear();
			Object.assign(lookupState, createMetadataLookupState());
			Object.assign(queueState, createMetadataLookupQueueState());
			publish();
		},
	};
}

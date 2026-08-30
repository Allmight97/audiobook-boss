import { createSignal, type Accessor } from 'solid-js';
import type { InputOwner } from '../inputSession/owner';
import type { MetadataOwner } from '../metadataSession/owner';
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
	readonly previewRevision: Accessor<number>;
	run(action: MetadataLookupWorkflowAction): Promise<void>;
	setTitleQuery(value: string): void;
	setAuthorQuery(value: string): void;
	setSource(value: MetadataLookupSource): void;
	setApplyMode(value: MetadataLookupApplyMode): void;
	setReplaceCover(value: boolean): void;
	bumpPreview(): void;
	reset(): void;
};

export function createMetadataLookupOwner(deps: {
	readonly input: InputOwner;
	readonly metadata: MetadataOwner;
}): MetadataLookupOwner {
	const lookupState = createMetadataLookupState();
	const queueState = createMetadataLookupQueueState();
	const [view, setView] = createSignal(snapshotMetadataLookupState(lookupState));
	const [previewRevision, setPreviewRevision] = createSignal(0);

	function publish(): void {
		setView(snapshotMetadataLookupState(lookupState));
	}

	function services() {
		return makeProductionLookupServices(
			{
				input: deps.input,
				metadata: deps.metadata,
				lookupState,
				queueState,
			},
			publish,
		);
	}

	return {
		view,
		previewRevision,
		async run(action) {
			const layer = makeMetadataLookupWorkflowServicesLayer(services());
			try {
				await runMetadataLookupWorkflow(layer, action);
				publish();
			} catch (error) {
				console.error('Metadata lookup failed:', error);
				publish();
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
		bumpPreview() {
			setPreviewRevision((value) => value + 1);
		},
		reset() {
			Object.assign(lookupState, createMetadataLookupState());
			Object.assign(queueState, createMetadataLookupQueueState());
			setPreviewRevision(0);
			publish();
		},
	};
}

import { createSignal, type Accessor } from 'solid-js';
import type { InputOwner } from '../inputSession';
import type { MetadataOwner } from '../metadataSession';
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
	let snapshot = snapshotMetadataLookupState(lookupState);
	let previewRevision = 0;
	const [rev, bump] = createSignal(0, { ownedWrite: true });

	function publish(): void {
		snapshot = snapshotMetadataLookupState(lookupState);
		bump((n) => n + 1);
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
		view: () => {
			rev();
			return snapshot;
		},
		previewRevision: () => {
			rev();
			return previewRevision;
		},
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
			previewRevision += 1;
			bump((n) => n + 1);
		},
		reset() {
			Object.assign(lookupState, createMetadataLookupState());
			Object.assign(queueState, createMetadataLookupQueueState());
			previewRevision = 0;
			publish();
		},
	};
}

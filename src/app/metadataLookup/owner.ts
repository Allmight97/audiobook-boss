import { createSignal, type Accessor } from 'solid-js';
import type { InputOwner } from '../inputSession/owner';
import type { MetadataOwner } from '../metadataSession/owner';
import { makeProductionLookupServices } from './services';
import {
	createMetadataLookupState,
	metadataLookupState,
	resetMetadataLookupState,
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
	const [view, setView] = createSignal(createMetadataLookupState());
	const [previewRevision, setPreviewRevision] = createSignal(0);

	function publish(): void {
		setView(snapshotMetadataLookupState());
	}

	function services() {
		return makeProductionLookupServices(deps, publish);
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
			metadataLookupState.titleQuery = value;
			publish();
		},
		setAuthorQuery(value) {
			metadataLookupState.authorQuery = value;
			publish();
		},
		setSource(value) {
			metadataLookupState.source = value;
			publish();
		},
		setApplyMode(value) {
			metadataLookupState.applyMode = value;
			publish();
		},
		setReplaceCover(value) {
			metadataLookupState.replaceCoverArt = value;
			publish();
		},
		bumpPreview() {
			setPreviewRevision((value) => value + 1);
		},
		reset() {
			resetMetadataLookupState();
			setPreviewRevision(0);
			publish();
		},
	};
}

import { createSignal, type Accessor } from 'solid-js';
import type { AudioFile } from '../../types/audio';
import type { InputOwner } from '../inputSession/owner';
import { clearRemoteSourceCoverPreviewCache } from './coverPreview';
import { makeProductionRemoteSourceServices } from './services';
import {
	reconcileRemoteSourceSessionsWithInput,
	resetRemoteSourceSessionAssets,
} from './sessionAssets';
import { patchRemoteSourceState, resetRemoteSourceState, snapshotRemoteSourceView } from './state';
import type { RemoteSourceView } from './types';
import {
	invalidateRemoteSourceWorkflows,
	makeRemoteSourceWorkflowServicesLayer,
	runRemoteSourceWorkflow,
	type RemoteSourceWorkflowAction,
} from './workflow';

export type RemoteSourceOwner = {
	readonly view: Accessor<RemoteSourceView>;
	open(): void;
	close(): void;
	patch(patch: Parameters<typeof patchRemoteSourceState>[0]): void;
	runAction(action: RemoteSourceWorkflowAction): Promise<void>;
	reconcileWithInput(files: ReadonlyArray<AudioFile>): void;
	reset(): void;
};

export function createRemoteSourceOwner(deps: { readonly input: InputOwner }): RemoteSourceOwner {
	const [view, setView] = createSignal(snapshotRemoteSourceView());

	function publish(): void {
		setView(snapshotRemoteSourceView());
	}

	function services() {
		return makeProductionRemoteSourceServices({
			inputView: () => deps.input.view(),
			importPaths: (paths) => deps.input.importIntent({ type: 'importPaths', paths }),
		});
	}

	return {
		view,
		open() {
			patchRemoteSourceState({ isOpen: true });
			publish();
		},
		close() {
			patchRemoteSourceState({ isOpen: false, didHydrateOpenDialog: false });
			publish();
		},
		patch(patch) {
			patchRemoteSourceState(patch);
			publish();
		},
		async runAction(action) {
			const layer = makeRemoteSourceWorkflowServicesLayer(services());
			try {
				await runRemoteSourceWorkflow(layer, action, publish);
			} catch (error) {
				console.error('Remote source workflow failed:', error);
				publish();
			}
		},
		reconcileWithInput(files) {
			void reconcileRemoteSourceSessionsWithInput(files);
		},
		reset() {
			invalidateRemoteSourceWorkflows();
			resetRemoteSourceState();
			resetRemoteSourceSessionAssets();
			clearRemoteSourceCoverPreviewCache();
			publish();
		},
	};
}

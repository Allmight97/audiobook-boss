import { createSignal, type Accessor } from 'solid-js';
import type { AudioFile } from '../../types/audio';
import type { InputOwner } from '../inputSession';
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

export function resetRemoteSource(): void {
	invalidateRemoteSourceWorkflows();
	resetRemoteSourceState();
	resetRemoteSourceSessionAssets();
	clearRemoteSourceCoverPreviewCache();
}

export function createRemoteSourceOwner(deps: { readonly input: InputOwner }): RemoteSourceOwner {
	let snapshot = snapshotRemoteSourceView();
	const [rev, bump] = createSignal(0, { ownedWrite: true });

	function publish(): void {
		snapshot = snapshotRemoteSourceView();
		bump((n) => n + 1);
	}

	function services() {
		return makeProductionRemoteSourceServices({
			inputView: () => deps.input.view(),
			importPaths: (paths) => deps.input.importIntent({ type: 'importPaths', paths }),
		});
	}

	return {
		view: () => {
			rev();
			return snapshot;
		},
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
			try {
				await runRemoteSourceWorkflow(services(), action, publish);
			} catch (error) {
				console.error('Remote source workflow failed:', error);
				publish();
			}
		},
		reconcileWithInput(files) {
			void reconcileRemoteSourceSessionsWithInput(files);
		},
		reset() {
			resetRemoteSource();
			publish();
		},
	};
}

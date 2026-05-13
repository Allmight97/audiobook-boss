import { get } from 'svelte/store';

import { tauriClient } from '../../lib/tauri/client';
import { getCurrentFileList } from '../fileList/state.svelte';
import { persistPendingMetadataDraftsForCurrentSelection } from '../fileList/actions';
import { clearPendingMetadataForFile, getPendingMetadataIntentEntries } from '../metadataState';
import { metadataSaveInProgressStore } from '../metadataSaveState';
import { resetDirtyState } from '../metadataForm';
import {
	beginMetadataSaveInStatusPanel,
	completeMetadataSaveInStatusPanel,
	failMetadataSaveInStatusPanel,
	initStatusPanel,
	isStatusPanelProcessing,
	pushStatusPanelTransientStatus,
	triggerProcessFromStatusPanel,
} from '../statusPanel';

export async function saveMetadataFromUI(): Promise<void> {
	const fileList = getCurrentFileList();
	if (!fileList?.files.length) {
		console.log('No files loaded - nothing to save');
		return;
	}

	initStatusPanel();
	if (isStatusPanelProcessing()) {
		console.log('Processing in progress - cannot save metadata now');
		return;
	}

	pushStatusPanelTransientStatus('Preparing metadata save...', { ttlMs: 1_000 });

	if (get(metadataSaveInProgressStore)) {
		pushStatusPanelTransientStatus('Save already in progress...', { ttlMs: 1_500 });
		return;
	}

	try {
		metadataSaveInProgressStore.set(true);
		const staged = await persistPendingMetadataDraftsForCurrentSelection({ showStatus: false });
		if (!staged) {
			pushStatusPanelTransientStatus('Fix metadata validation errors before saving.', {
				ttlMs: 3_000,
			});
			return;
		}

		const validFilePaths = new Set(
			fileList.files.filter((file) => file.isValid).map((file) => file.path),
		);
		const pendingEntries = getPendingMetadataIntentEntries().filter(([filePath]) =>
			validFilePaths.has(filePath),
		);
		if (pendingEntries.length === 0) {
			pushStatusPanelTransientStatus('No pending metadata changes', { ttlMs: 2_000 });
			return;
		}

		await beginMetadataSaveInStatusPanel();
		const result = await tauriClient.saveMetadataBatch(
			pendingEntries.map(([filePath, metadataIntent]) => ({
				filePath,
				metadataPatch: metadataIntent,
			})),
		);

		for (const entry of result.results) {
			if (entry.status === 'success') {
				clearPendingMetadataForFile(entry.filePath);
			} else if (entry.status === 'failed') {
				console.error(`Failed metadata save for ${entry.filePath}:`, entry.error ?? entry.message);
			}
		}

		resetDirtyState();
		console.log(
			`Metadata save complete: success=${result.summary.succeeded}, failed=${result.summary.failed}, cancelled=${result.summary.cancelled}`,
		);
		completeMetadataSaveInStatusPanel(result);
	} catch (error) {
		console.error('Failed to save metadata:', error);
		failMetadataSaveInStatusPanel('Save failed - see console');
		pushStatusPanelTransientStatus('Save failed - see console', { ttlMs: 3_000 });
	} finally {
		metadataSaveInProgressStore.set(false);
	}
}

export function startPreviewAudio(duration: number): void {
	triggerProcessFromStatusPanel({ previewSeconds: duration });
}

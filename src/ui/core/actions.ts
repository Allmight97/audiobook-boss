import { get } from 'svelte/store';

import { tauriClient } from '../../lib/tauri/client';
import { getCurrentFileList } from '../fileList';
import { persistPendingMetadataDraftsForCurrentSelection } from '../fileList/actions';
import { clearPendingMetadataForFile, getPendingMetadataIntentEntries } from '../metadataState';
import { metadataSaveInProgressStore } from '../metadataSaveState';
import { resetDirtyState } from '../metadataForm';
import {
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
		await persistPendingMetadataDraftsForCurrentSelection({ showStatus: false });

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

		let successCount = 0;
		let failureCount = 0;
		const failedFiles: string[] = [];

		for (const [index, [filePath, metadataIntent]] of pendingEntries.entries()) {
			pushStatusPanelTransientStatus(`Saving ${index + 1}/${pendingEntries.length}...`, {
				ttlMs: 1_200,
			});

			try {
				await tauriClient.saveMetadataIntentToFile(filePath, metadataIntent);
				clearPendingMetadataForFile(filePath);
				successCount++;
			} catch (error) {
				failureCount++;
				failedFiles.push(filePath.split(/[\\/]/).pop() || filePath);
				console.error(`Failed metadata save for ${filePath}:`, error);
			}
		}

		resetDirtyState();
		console.log(`Metadata save complete: success=${successCount}, failed=${failureCount}`);

		const message =
			failureCount === 0
				? successCount > 1
					? `Metadata saved (${successCount} files)!`
					: 'Metadata saved!'
				: `Saved ${successCount}/${pendingEntries.length}. Failed: ${failedFiles.join(', ')}`;
		pushStatusPanelTransientStatus(message, { ttlMs: 3_000 });
	} catch (error) {
		console.error('Failed to save metadata:', error);
		pushStatusPanelTransientStatus('Save failed - see console', { ttlMs: 3_000 });
	} finally {
		metadataSaveInProgressStore.set(false);
	}
}

export function startPreviewAudio(duration: number): void {
	triggerProcessFromStatusPanel({ previewSeconds: duration });
}

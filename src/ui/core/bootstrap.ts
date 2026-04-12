import { tauriClient } from '../../lib/tauri/client';
import { initFileImport } from '../fileImport';
import { getCurrentFileList } from '../fileList';
import { persistPendingMetadataDraftsForCurrentSelection } from '../fileList/actions';
import { initEncoderPanel } from '../encoderPanel';
import { initOutputPanel } from '../outputPanel';
import { initStatusPanel, pushStatusPanelTransientStatus } from '../statusPanel';
import { initCoverArt } from '../coverArt';
import { initMetadataFormEvents, resetDirtyState } from '../metadataForm';
import { initTagPreview } from '../tagPreview';
import { initJobControls } from '../jobControls';
import { initMetadataLookup } from '../metadataLookup';
import { clearPendingMetadataForFile, getPendingMetadataIntentEntries } from '../metadataState';
import { isMetadataSaveInProgress, setMetadataSaveInProgress } from '../metadataSaveState';
import type { StatusPanel } from '../statusPanel';

let shellStatusPanel: StatusPanel | null = null;

function ensureShellStatusPanel(): StatusPanel {
	if (!shellStatusPanel) {
		shellStatusPanel = initStatusPanel();
	}
	return shellStatusPanel;
}

export async function saveMetadataFromUI(): Promise<void> {
	const fileList = getCurrentFileList();
	if (!fileList || fileList.files.length === 0) {
		console.log('No files loaded - nothing to save');
		return;
	}

	const statusPanel = ensureShellStatusPanel();
	if (statusPanel.isCurrentlyProcessing) {
		console.log('Processing in progress - cannot save metadata now');
		return;
	}

	pushStatusPanelTransientStatus('Preparing metadata save...', { ttlMs: 1_000 });

	if (isMetadataSaveInProgress()) {
		pushStatusPanelTransientStatus('Save already in progress...', { ttlMs: 1_500 });
		return;
	}

	try {
		setMetadataSaveInProgress(true);
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
		setMetadataSaveInProgress(false);
	}
}

export function startPreviewAudio(duration: number): void {
	ensureShellStatusPanel().startProcessing({ previewSeconds: duration });
}

export function initializeAppShell(): string | null {
	try {
		initFileImport();
		initEncoderPanel();
		initOutputPanel();
		ensureShellStatusPanel();
		initCoverArt();
		initMetadataFormEvents();
		initTagPreview();
		initMetadataLookup();
		initJobControls();
		console.log('UI initialized');
		return null;
	} catch (error) {
		const fatalMessage = `UI initialization failed. ${error instanceof Error ? error.message : String(error)}`;
		console.error('[ui:init] fatal failure', error);
		return fatalMessage;
	}
}

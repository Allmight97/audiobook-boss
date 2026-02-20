import { bridge } from './lib/bridge';
import { initFileImport } from './ui/fileImport';
import { getCurrentFileList } from './ui/fileList';
import { persistPendingMetadataDraftsForCurrentSelection } from './ui/fileList/actions';
import { initOutputPanel } from './ui/outputPanel';
import { initStatusPanel, getStatusPanel } from './ui/statusPanel/index';
import { initEncoderPanel } from './ui/encoderPanel';
import { initCoverArt } from './ui/coverArt';
import { initMetadataFormEvents, resetDirtyState } from './ui/metadataForm';
import { initTagPreview } from './ui/tagPreview';
import { initJobControls } from './ui/jobControls';
import { initMetadataLookup } from './ui/metadataLookup';
import { clearPendingMetadataForFile, getPendingMetadataIntentEntries } from './ui/metadataState';
import { isMetadataSaveInProgress, setMetadataSaveInProgress } from './ui/metadataSaveState';

// Initialize UI components when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
	initFileImport();
	// Initialize Advanced Encoder panel before output handlers so shared controls are present.
	initEncoderPanel();
	initOutputPanel();
	initStatusPanel();
	initCoverArt();
	initMetadataFormEvents();
	// Initialize tag preview grid
	initTagPreview();
	initMetadataLookup();
	// Initialize Cmd+S metadata save handler
	initMetadataSaveHandler();
	// Initialize Job Controls (Job Type, Max Concurrent)
	initJobControls();
	console.log('File import system initialized');
	console.log('Output panel initialized');
	console.log('Status panel initialized');
	console.log('Cover art system initialized');
	console.log('Tag preview initialized');
	console.log('Metadata save handler initialized (Cmd+S / Ctrl+S)');
});

function setUserStatusMessage(
	statusText: HTMLElement | null,
	message: string,
	holdMs = 1000,
): void {
	if (!statusText) return;
	statusText.dataset.userStatusLockUntil = String(Date.now() + holdMs);
	statusText.textContent = message;
}

function restoreStatusMessage(
	statusText: HTMLElement | null,
	expectedMessage: string,
	fallbackMessage: string,
): void {
	if (!statusText) return;
	if (statusText.textContent !== expectedMessage) return;
	delete statusText.dataset.userStatusLockUntil;
	statusText.textContent = fallbackMessage;
}

/**
 * Save workflow for Draft-First metadata UX:
 * 1) persist current-selection draft deltas into pending state
 * 2) save all pending drafts in one batch
 * 3) keep failed files pending for retry
 */

async function saveMetadataFromUI(): Promise<void> {
	const fileList = getCurrentFileList();
	// Check if we have files loaded
	if (!fileList || fileList.files.length === 0) {
		console.log('No files loaded - nothing to save');
		return;
	}

	// Check if processing is active
	const statusPanel = getStatusPanel();
	if (statusPanel?.isCurrentlyProcessing) {
		console.log('Processing in progress - cannot save metadata now');
		return;
	}

	const statusText = document.getElementById('status-text');
	const originalText = statusText?.textContent ?? '';
	setUserStatusMessage(statusText, 'Preparing metadata save...', 1000);

	if (isMetadataSaveInProgress()) {
		setUserStatusMessage(statusText, 'Save already in progress...', 1500);
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
			const msg = 'No pending metadata changes';
			setUserStatusMessage(statusText, msg, 2000);
			setTimeout(() => {
				restoreStatusMessage(statusText, msg, originalText);
			}, 2000);
			return;
		}

		let successCount = 0;
		let failureCount = 0;
		const failedFiles: string[] = [];

		for (const [index, [filePath, metadataIntent]] of pendingEntries.entries()) {
			setUserStatusMessage(statusText, `Saving ${index + 1}/${pendingEntries.length}...`, 1200);

			try {
				await bridge.saveMetadataIntentToFile(filePath, metadataIntent);
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

		const msg =
			failureCount === 0
				? successCount > 1
					? `Metadata saved (${successCount} files)!`
					: 'Metadata saved!'
				: `Saved ${successCount}/${pendingEntries.length}. Failed: ${failedFiles.join(', ')}`;
		setUserStatusMessage(statusText, msg, 3000);
		setTimeout(() => {
			restoreStatusMessage(statusText, msg, originalText);
		}, 3000);
	} catch (error) {
		console.error('Failed to save metadata:', error);
		setUserStatusMessage(statusText, 'Save failed - see console', 3000);
	} finally {
		setMetadataSaveInProgress(false);
	}
}

function initMetadataSaveHandler(): void {
	const saveButton = document.getElementById('metadata-save-btn') as HTMLButtonElement | null;
	if (saveButton) {
		saveButton.addEventListener('click', () => {
			void saveMetadataFromUI();
		});
	}

	document.addEventListener('keydown', (event) => {
		// Check for Cmd+S (Mac) or Ctrl+S (Windows/Linux)
		if ((event.metaKey || event.ctrlKey) && event.key === 's') {
			event.preventDefault(); // Prevent browser save dialog
			void saveMetadataFromUI();
		}
	});
}

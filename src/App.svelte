<script lang="ts">
	import { onMount } from 'svelte';
	import { tauriClient } from './lib/tauri/client';
	import { initFileImport } from './ui/fileImport';
	import { getCurrentFileList } from './ui/fileList';
	import { persistPendingMetadataDraftsForCurrentSelection } from './ui/fileList/actions';
	import { initOutputPanel } from './ui/outputPanel';
	import { initStatusPanel, getStatusPanel } from './ui/statusPanel/index';
	import { initEncoderPanel } from './ui/encoderPanel';
	import { initCoverArt } from './ui/coverArt';
	import {
		initMetadataFormEvents,
		resetDirtyState,
		setMetadataFormSaveHandler,
	} from './ui/metadataForm';
	import { initTagPreview } from './ui/tagPreview';
	import { initJobControls } from './ui/jobControls';
	import { initMetadataLookup } from './ui/metadataLookup';
	import { clearPendingMetadataForFile, getPendingMetadataIntentEntries } from './ui/metadataState';
	import { isMetadataSaveInProgress, setMetadataSaveInProgress } from './ui/metadataSaveState';

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

	async function saveMetadataFromUI(): Promise<void> {
		const fileList = getCurrentFileList();
		if (!fileList || fileList.files.length === 0) {
			console.log('No files loaded - nothing to save');
			return;
		}

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

	function handleGlobalKeyDown(event: KeyboardEvent): void {
		if ((event.metaKey || event.ctrlKey) && event.key === 's') {
			event.preventDefault();
			void saveMetadataFromUI();
		}
	}

	onMount(() => {
		setMetadataFormSaveHandler(() => {
			void saveMetadataFromUI();
		});

		initFileImport();
		initEncoderPanel();
		initOutputPanel();
		initStatusPanel();
		initCoverArt();
		initMetadataFormEvents();
		initTagPreview();
		initMetadataLookup();
		initJobControls();

		console.log('UI initialized');
	});
</script>

<svelte:window on:keydown={handleGlobalKeyDown} />

<div class="main-container">
	<div class="panel input-panel">
		<div class="flex flex-col gap-2 mb-2">
			<div class="flex items-center justify-between">
				<h3 class="section-title mb-0 whitespace-nowrap mr-2">Input and File Order</h3>
				<div id="job-controls-root"></div>
			</div>

			<div id="file-import-root"></div>
		</div>

		<div
			class="section-divider file-properties-pinned inspector-footer"
			role="region"
			aria-label="Selected File Properties"
		>
			<div class="inspector-header">
				<span class="inspector-context" id="prop-selected-context" aria-live="polite">
					<span class="context-empty">No file selected</span>
				</span>
			</div>

			<div class="grid grid-cols-2 gap-x-3 gap-y-0.5 text-xs">
				<span class="property-label">Bitrate:</span><span class="property-value" id="prop-bitrate"
					>--- kb/s</span
				>
				<span class="property-label">Sample Rate:</span><span class="property-value" id="prop-samplerate"
					>--- Hz</span
				>
				<span class="property-label">Channels:</span><span class="property-value" id="prop-channels"
					>---</span
				>
				<span class="property-label">File Size:</span><span class="property-value" id="prop-filesize"
					>--- MB</span
				>
				<span class="property-label">Combined Size:</span><span class="property-value" id="prop-combinedsize"
					>--- MB</span
				>
			</div>
		</div>
	</div>

	<div class="right-column-wrapper">
		<div class="panel metadata-output-panel">
			<div class="metadata-output-scroll">
				<div class="section-header">
					<h3>Metadata Manager</h3>
				</div>
				<div id="metadata-selection-count" class="text-xs muted-text mb-2" hidden></div>
				<div id="metadata-form" data-multi-select="false">
					<div class="grid grid-cols-4 gap-3 mb-3">
						<div id="cover-art-root" class="col-span-1"></div>
						<div id="metadata-form-fields-root" class="col-span-3"></div>
					</div>
				</div>

				<div id="encoder-panel-root"></div>
			</div>

			<div
				class="section-divider metadata-footer-pinned inspector-footer"
				role="region"
				aria-label="Output and preview"
			>
				<div id="output-panel-root"></div>

				<div class="section-divider">
					<div class="section-header justify-between">
						<h3>Preview Audio and Metadata Tags</h3>
						<div class="split-button">
							<button id="preview-button" class="btn-pill btn-pill-primary split-main">
								Preview Audio
							</button>
							<button id="preview-dropdown-toggle" class="btn-pill btn-pill-primary split-caret">
								▼
							</button>
							<div id="preview-dropdown" class="split-dropdown">
								<button class="split-option" data-duration="15">15 seconds</button>
								<button class="split-option" data-duration="30">30 seconds</button>
								<button class="split-option" data-duration="45">45 seconds</button>
								<button class="split-option" data-duration="60">60 seconds</button>
							</div>
						</div>
					</div>

					<div id="tag-preview-root"></div>
				</div>
			</div>
		</div>

		<div id="status-panel-root"></div>
	</div>
</div>
<div id="metadata-lookup-root"></div>

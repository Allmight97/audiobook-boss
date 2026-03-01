<script lang="ts">
	import { onMount } from 'svelte';
	import { tauriClient } from './lib/tauri/client';
	import { initFileImport } from './ui/fileImport';
	import FileImportIsland from './ui/fileImport/FileImportIsland.svelte';
	import { getCurrentFileList } from './ui/fileList';
	import { fileListViewState } from './ui/fileList/viewState.svelte';
	import { persistPendingMetadataDraftsForCurrentSelection } from './ui/fileList/actions';
	import EncoderPanelIsland from './ui/encoderPanel/EncoderPanelIsland.svelte';
	import { initOutputPanel } from './ui/outputPanel';
	import OutputPanelIsland from './ui/outputPanel/OutputPanelIsland.svelte';
	import {
		getStatusPanel,
		initStatusPanel,
		pushStatusPanelTransientStatus,
	} from './ui/statusPanel/index';
	import StatusPanelIsland from './ui/statusPanel/StatusPanelIsland.svelte';
	import { initEncoderPanel } from './ui/encoderPanel';
	import {
		initCoverArt,
		onClearCoverArt,
		onLoadCoverArtFromFilePicker,
		onLoadCoverArtFromInput,
	} from './ui/coverArt';
	import CoverArtIsland from './ui/coverArt/CoverArtIsland.svelte';
	import {
		initMetadataFormEvents,
		onMetadataFormActionSelectChange,
		onMetadataFormFieldInput,
		resetDirtyState,
		setMetadataFormSaveHandler,
		triggerMetadataFormSave,
	} from './ui/metadataForm';
	import MetadataFormFieldsIsland from './ui/metadataForm/MetadataFormFieldsIsland.svelte';
	import { initTagPreview } from './ui/tagPreview';
	import TagPreviewIsland from './ui/tagPreview/TagPreviewIsland.svelte';
	import {
		handleMaxConcurrentSelectionChange,
		handleMergeModeChange,
		initJobControls,
	} from './ui/jobControls';
	import JobControlsIsland from './ui/jobControls/JobControlsIsland.svelte';
	import { initMetadataLookup } from './ui/metadataLookup';
	import MetadataLookupIsland from './ui/metadataLookup/MetadataLookupIsland.svelte';
	import { clearPendingMetadataForFile, getPendingMetadataIntentEntries } from './ui/metadataState';
	import { isMetadataSaveInProgress, setMetadataSaveInProgress } from './ui/metadataSaveState';
	let previewDuration = 30;
	let previewDropdownOpen = false;
	let uiInitFatalMessage: string | null = null;
	let previewDropdownElement: HTMLDivElement | null = null;
	let previewDropdownToggleElement: HTMLButtonElement | null = null;

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

			const msg =
				failureCount === 0
					? successCount > 1
						? `Metadata saved (${successCount} files)!`
						: 'Metadata saved!'
					: `Saved ${successCount}/${pendingEntries.length}. Failed: ${failedFiles.join(', ')}`;
			pushStatusPanelTransientStatus(msg, { ttlMs: 3_000 });
		} catch (error) {
			console.error('Failed to save metadata:', error);
			pushStatusPanelTransientStatus('Save failed - see console', { ttlMs: 3_000 });
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

	function startPreviewAudio(duration: number): void {
		const panel = getStatusPanel();
		if (!panel) return;
		void panel.startProcessing({ previewSeconds: duration });
	}

	function handlePreviewButtonClick(): void {
		startPreviewAudio(previewDuration);
	}

	function handlePreviewDropdownToggle(event: MouseEvent): void {
		event.stopPropagation();
		previewDropdownOpen = !previewDropdownOpen;
	}

	function handlePreviewDurationSelect(duration: number): void {
		previewDuration = duration;
		previewDropdownOpen = false;
		startPreviewAudio(duration);
	}

	function handleWindowClick(event: MouseEvent): void {
		if (!previewDropdownOpen) return;
		const target = event.target;
		if (!(target instanceof Node)) return;
		if (previewDropdownElement?.contains(target) || previewDropdownToggleElement?.contains(target)) {
			return;
		}
		previewDropdownOpen = false;
	}

	onMount(() => {
		setMetadataFormSaveHandler(() => {
			void saveMetadataFromUI();
		});

		try {
			initFileImport();
			initEncoderPanel();
			initOutputPanel();
			initStatusPanel();
			initCoverArt();
			initMetadataFormEvents();
			initTagPreview();
			initMetadataLookup();
			initJobControls();
		} catch (error) {
			uiInitFatalMessage = `UI initialization failed. ${error instanceof Error ? error.message : String(error)}`;
			console.error('[ui:init] fatal failure', error);
			return;
		}

		console.log('UI initialized');
	});
</script>

<svelte:window on:keydown={handleGlobalKeyDown} on:click={handleWindowClick} />

{#if uiInitFatalMessage}
	<div class="fatal-init-banner" role="alert" aria-live="assertive">
		<h2>Initialization Error</h2>
		<p>{uiInitFatalMessage}</p>
		<p class="fatal-init-help">Restart the app. If this keeps happening, share the console logs.</p>
	</div>
{:else}
<div class="main-container">
	<div class="panel input-panel">
		<div class="flex flex-col gap-2 mb-2">
			<div class="flex items-center justify-between">
				<h3 class="section-title mb-0 whitespace-nowrap mr-2">Input and File Order</h3>
				<JobControlsIsland
					onMergeModeChange={handleMergeModeChange}
					onMaxConcurrentSelectionChange={handleMaxConcurrentSelectionChange}
				/>
			</div>

			<FileImportIsland />
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
				<span class="property-label">Codec:</span><span class="property-value" id="prop-codec"
					>---</span
				>
				<span class="property-label">Decoder:</span><span class="property-value" id="prop-decoder"
					>---</span
				>
				<span class="property-label">File Size:</span><span class="property-value" id="prop-filesize"
					>--- MB</span
				>
				<span class="property-label">Combined Size:</span><span class="property-value" id="prop-combinedsize"
					>{fileListViewState.combinedSizeText}</span
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
						<div class="col-span-1">
							<CoverArtIsland
								onLoadFromFile={onLoadCoverArtFromFilePicker}
								onLoadFromInput={onLoadCoverArtFromInput}
								onClearCoverArt={onClearCoverArt}
							/>
						</div>
						<div class="col-span-3">
							<MetadataFormFieldsIsland
								onFieldInput={onMetadataFormFieldInput}
								onActionChange={onMetadataFormActionSelectChange}
								onSaveMetadata={triggerMetadataFormSave}
							/>
						</div>
					</div>
				</div>

				<EncoderPanelIsland />
			</div>

			<div
				class="section-divider metadata-footer-pinned inspector-footer"
				role="region"
				aria-label="Output and preview"
			>
				<OutputPanelIsland />

				<div class="section-divider">
					<div class="section-header justify-between">
						<h3>Preview Audio and Metadata Tags</h3>
						<div class="split-button">
							<button
								id="preview-button"
								class="btn-pill btn-pill-primary split-main"
								on:click={handlePreviewButtonClick}
							>
								Preview Audio
							</button>
							<button
								id="preview-dropdown-toggle"
								class="btn-pill btn-pill-primary split-caret"
								bind:this={previewDropdownToggleElement}
								on:click={handlePreviewDropdownToggle}
							>
								▼
							</button>
							<div
								id="preview-dropdown"
								class="split-dropdown"
								style={`display: ${previewDropdownOpen ? 'block' : 'none'}`}
								bind:this={previewDropdownElement}
							>
								<button class="split-option" data-duration="15" on:click={() => handlePreviewDurationSelect(15)}
									>15 seconds</button
								>
								<button class="split-option" data-duration="30" on:click={() => handlePreviewDurationSelect(30)}
									>30 seconds</button
								>
								<button class="split-option" data-duration="45" on:click={() => handlePreviewDurationSelect(45)}
									>45 seconds</button
								>
								<button class="split-option" data-duration="60" on:click={() => handlePreviewDurationSelect(60)}
									>60 seconds</button
								>
							</div>
						</div>
					</div>

					<TagPreviewIsland />
				</div>
			</div>
		</div>

		<StatusPanelIsland />
	</div>
</div>
<MetadataLookupIsland />
{/if}

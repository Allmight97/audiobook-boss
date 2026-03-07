<script lang="ts">
	import { onMount } from 'svelte';
	import FileImportIsland from './ui/fileImport/FileImportIsland.svelte';
	import { fileListViewState } from './ui/fileList/viewState.svelte';
	import EncoderPanelIsland from './ui/encoderPanel/EncoderPanelIsland.svelte';
	import OutputPanelIsland from './ui/outputPanel/OutputPanelIsland.svelte';
	import StatusPanelIsland from './ui/statusPanel/StatusPanelIsland.svelte';
	import {
		onClearCoverArt,
		onLoadCoverArtFromFilePicker,
		onLoadCoverArtFromInput,
	} from './ui/coverArt';
	import CoverArtIsland from './ui/coverArt/CoverArtIsland.svelte';
	import {
		onMetadataFormActionSelectChange,
		onMetadataFormFieldInput,
	} from './ui/metadataForm';
	import { metadataFormState } from './ui/metadataForm/state.svelte';
	import MetadataFormFieldsIsland from './ui/metadataForm/MetadataFormFieldsIsland.svelte';
	import TagPreviewIsland from './ui/tagPreview/TagPreviewIsland.svelte';
	import {
		handleMaxConcurrentSelectionChange,
		handleMergeModeChange,
	} from './ui/jobControls';
	import JobControlsIsland from './ui/jobControls/JobControlsIsland.svelte';
	import MetadataLookupIsland from './ui/metadataLookup/MetadataLookupIsland.svelte';
	import { initializeAppShell, saveMetadataFromUI, startPreviewAudio } from './ui/core/bootstrap';
	import { shellState } from './ui/core/shellState.svelte';
	import { inspectorState } from './ui/fileList/inspectorState.svelte';
	let previewDropdownElement: HTMLDivElement | null = null;
	let previewDropdownToggleElement: HTMLButtonElement | null = null;

	function handleGlobalKeyDown(event: KeyboardEvent): void {
		if ((event.metaKey || event.ctrlKey) && event.key === 's') {
			event.preventDefault();
			void saveMetadataFromUI();
		}
	}

	function handlePreviewButtonClick(): void {
		startPreviewAudio(shellState.previewDuration);
	}

	function handlePreviewDropdownToggle(event: MouseEvent): void {
		event.stopPropagation();
		shellState.previewDropdownOpen = !shellState.previewDropdownOpen;
	}

	function handlePreviewDurationSelect(duration: number): void {
		shellState.previewDuration = duration;
		shellState.previewDropdownOpen = false;
		startPreviewAudio(duration);
	}

	function handleWindowClick(event: MouseEvent): void {
		if (!shellState.previewDropdownOpen) return;
		const target = event.target;
		if (!(target instanceof Node)) return;
		if (previewDropdownElement?.contains(target) || previewDropdownToggleElement?.contains(target)) {
			return;
		}
		shellState.previewDropdownOpen = false;
	}

	onMount(() => {
		shellState.uiInitFatalMessage = initializeAppShell();
	});
</script>

<svelte:window on:keydown={handleGlobalKeyDown} on:click={handleWindowClick} />

{#if shellState.uiInitFatalMessage}
	<div class="fatal-init-banner" role="alert" aria-live="assertive">
		<h2>Initialization Error</h2>
		<p>{shellState.uiInitFatalMessage}</p>
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
				<span class="inspector-context" aria-live="polite">
					{#if inspectorState.contextVariant === 'empty'}
						<span class="context-empty">{inspectorState.contextText}</span>
					{:else}
						<span class="context-filename" title={inspectorState.contextText}>
							{inspectorState.contextText}
						</span>
						{#if inspectorState.contextDetail}
							<span class="context-position">{inspectorState.contextDetail}</span>
						{/if}
					{/if}
				</span>
			</div>

			<div class="grid grid-cols-2 gap-x-3 gap-y-0.5 text-xs">
				<span class="property-label">Bitrate:</span><span class="property-value">{inspectorState.bitrateText}</span>
				<span class="property-label">Sample Rate:</span><span class="property-value">{inspectorState.sampleRateText}</span>
				<span class="property-label">Channels:</span><span class="property-value">{inspectorState.channelsText}</span>
				<span class="property-label">Codec:</span><span class="property-value">{inspectorState.codecText}</span>
				<span class="property-label">Decoder:</span><span class="property-value">{inspectorState.decoderText}</span>
				<span class="property-label">File Size:</span><span class="property-value">{inspectorState.fileSizeText}</span>
				<span class="property-label">Combined Size:</span><span class="property-value"
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
				<div
					id="metadata-selection-count"
					class="text-xs muted-text mb-2"
					hidden={metadataFormState.mode !== 'multi' || metadataFormState.selectionCount <= 1}
				>
					{metadataFormState.selectionCount} files selected
				</div>
				<div id="metadata-form" data-multi-select={metadataFormState.mode === 'multi'}>
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
								onSaveMetadata={() => {
									void saveMetadataFromUI();
								}}
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
								style={`display: ${shellState.previewDropdownOpen ? 'block' : 'none'}`}
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

<script lang="ts">
	import FileImportIsland from './ui/fileImport/FileImportIsland.svelte';
import {
	onClearCoverArt,
	onLoadCoverArtFromFilePicker,
	onLoadCoverArtFromInput,
} from './ui/coverArt';
	import {
		onMetadataFormActionSelectChange,
		onMetadataFormFieldInput,
	} from './ui/metadataForm';
	import { metadataFormState } from './ui/metadataForm/state.svelte';
	import MetadataFormFieldsIsland from './ui/metadataForm/MetadataFormFieldsIsland.svelte';
	import OutputPanelIsland from './ui/outputPanel/OutputPanelIsland.svelte';
	import StatusPanelIsland from './ui/statusPanel/StatusPanelIsland.svelte';
	import EncoderPanelIsland from './ui/encoderPanel/EncoderPanelIsland.svelte';
	import CoverArtIsland from './ui/coverArt/CoverArtIsland.svelte';
	import TagPreviewIsland from './ui/tagPreview/TagPreviewIsland.svelte';
	import {
		handleMaxConcurrentSelectionChange,
		handleMergeModeChange,
	} from './ui/jobControls';
	import JobControlsIsland from './ui/jobControls/JobControlsIsland.svelte';
	import MetadataLookupIsland from './ui/metadataLookup/MetadataLookupIsland.svelte';
	import CollisionDialogIsland from './ui/collisionDialog/CollisionDialogIsland.svelte';
	import { createHarnessFixture, type PartialHarnessFixture } from './harness/fixtures';
	import { inspectorState } from './ui/fileList/inspectorState.svelte';
	import { fileListViewState } from './ui/fileList/viewState.svelte';
	import { saveMetadataFromUI } from './ui/core/actions';
	import PreviewAudioControls from './ui/previewAudio/PreviewAudioControls.svelte';

	export let fixture: PartialHarnessFixture = {};

	$: harnessFixture = createHarnessFixture(fixture);
</script>

<main class="main-container">
	<div class="panel input-panel">
		<div class="section-header justify-between">
			<h3>{harnessFixture.labels.inputPanelTitle}</h3>
			{#if harnessFixture.islands.jobControls.enabled}
				<JobControlsIsland
					onMergeModeChange={handleMergeModeChange}
					onMaxConcurrentSelectionChange={handleMaxConcurrentSelectionChange}
				/>
			{/if}
		</div>
		{#if harnessFixture.islands.fileImport.enabled}
			<FileImportIsland />
		{/if}

		<div class="section-divider file-properties-pinned inspector-footer" aria-hidden="true">
			<div
				id="metadata-selection-count"
				class="text-xs muted-text mb-2"
				hidden={metadataFormState.mode !== 'multi' || metadataFormState.selectionCount <= 1}
			>
				{metadataFormState.selectionCount} files selected
			</div>
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
			<div class="grid grid-cols-4 gap-3 mb-3">
				<div class="col-span-1">
					{#if harnessFixture.islands.coverArt.enabled}
						<CoverArtIsland
							onLoadFromFile={onLoadCoverArtFromFilePicker}
							onLoadFromInput={onLoadCoverArtFromInput}
							onClearCoverArt={onClearCoverArt}
						/>
					{/if}
				</div>
				<div class="col-span-3">
					{#if harnessFixture.islands.metadataForm.enabled}
						<div id="metadata-form" data-multi-select={metadataFormState.mode === 'multi'}>
							<MetadataFormFieldsIsland
								onFieldInput={onMetadataFormFieldInput}
								onActionChange={onMetadataFormActionSelectChange}
								onSaveMetadata={() => {
									void saveMetadataFromUI();
								}}
							/>
						</div>
					{/if}
				</div>
			</div>
			{#if harnessFixture.islands.encoderPanel.enabled}
				<EncoderPanelIsland />
			{/if}
			{#if harnessFixture.islands.outputPanel.enabled}
				<OutputPanelIsland />
			{/if}
			<div class="section-divider">
				<div class="section-header justify-between">
					<h3>Preview Audio and Metadata Tags</h3>
					<PreviewAudioControls />
				</div>
				{#if harnessFixture.islands.tagPreview.enabled}
					<TagPreviewIsland />
				{/if}
			</div>
		</div>
		{#if harnessFixture.islands.statusPanel.enabled}
			<StatusPanelIsland />
		{/if}
	</div>
</main>

{#if harnessFixture.islands.metadataLookup.enabled}
	<div class="panel" style="margin: 1rem;" data-testid="harness-metadata-lookup">
		<h3>{harnessFixture.labels.metadataLookupTitle}</h3>
		<MetadataLookupIsland />
	</div>
{/if}

<CollisionDialogIsland />

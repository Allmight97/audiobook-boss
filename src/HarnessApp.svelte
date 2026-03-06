<script lang="ts">
	import FileImportIsland from './ui/fileImport/FileImportIsland.svelte';
	import {
		onMetadataFormActionSelectChange,
		onMetadataFormFieldInput,
		triggerMetadataFormSave,
	} from './ui/metadataForm';
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
	import { createHarnessFixture, type PartialHarnessFixture } from './harness/fixtures';

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
			<div id="metadata-selection-count" class="text-xs muted-text mb-2" hidden></div>
			<div class="grid grid-cols-2 gap-x-3 gap-y-0.5 text-xs">
				<span id="prop-selected-context"></span>
				<span></span>
				<span id="prop-bitrate">---</span>
				<span id="prop-samplerate">---</span>
				<span id="prop-channels">---</span>
				<span id="prop-codec">---</span>
				<span id="prop-decoder">---</span>
				<span id="prop-filesize">---</span>
				<span id="prop-combinedsize">---</span>
			</div>
		</div>
	</div>

	<div class="right-column-wrapper">
		<div class="panel metadata-output-panel">
			<div class="grid grid-cols-4 gap-3 mb-3">
				<div class="col-span-1">
					{#if harnessFixture.islands.coverArt.enabled}
						<CoverArtIsland />
					{/if}
				</div>
				<div class="col-span-3">
					{#if harnessFixture.islands.metadataForm.enabled}
						<div id="metadata-form" data-multi-select="false">
							<MetadataFormFieldsIsland
								onFieldInput={onMetadataFormFieldInput}
								onActionChange={onMetadataFormActionSelectChange}
								onSaveMetadata={triggerMetadataFormSave}
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
			{#if harnessFixture.islands.tagPreview.enabled}
				<TagPreviewIsland />
			{/if}
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

<script lang="ts">
	import FileImportIsland from './ui/fileImport/FileImportIsland.svelte';
	import MetadataFormFieldsIsland from './ui/metadataForm/MetadataFormFieldsIsland.svelte';
	import OutputPanelIsland from './ui/outputPanel/OutputPanelIsland.svelte';
	import StatusPanelIsland from './ui/statusPanel/StatusPanelIsland.svelte';
	import EncoderPanelIsland from './ui/encoderPanel/EncoderPanelIsland.svelte';
	import CoverArtIsland from './ui/coverArt/CoverArtIsland.svelte';
	import TagPreviewIsland from './ui/tagPreview/TagPreviewIsland.svelte';
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
				<JobControlsIsland />
			{/if}
		</div>
		{#if harnessFixture.islands.fileImport.enabled}
			<FileImportIsland />
		{/if}
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
						<MetadataFormFieldsIsland />
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

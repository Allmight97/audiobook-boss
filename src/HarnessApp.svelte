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
</script>

<main class="main-container">
	<div class="panel input-panel">
		<div class="section-header justify-between">
			<h3>Harness: Input</h3>
			<JobControlsIsland />
		</div>
		<FileImportIsland />
	</div>

	<div class="right-column-wrapper">
		<div class="panel metadata-output-panel">
			<div class="grid grid-cols-4 gap-3 mb-3">
				<div class="col-span-1"><CoverArtIsland /></div>
				<div class="col-span-3"><MetadataFormFieldsIsland /></div>
			</div>
			<EncoderPanelIsland />
			<OutputPanelIsland />
			<TagPreviewIsland />
		</div>
		<StatusPanelIsland />
	</div>
</main>

<div class="panel" style="margin: 1rem;">
	<h3>Harness: Metadata Lookup Modal</h3>
	<MetadataLookupIsland />
</div>

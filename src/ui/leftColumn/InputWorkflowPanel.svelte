<script lang="ts">
	import FileImportIsland from '../fileImport/FileImportIsland.svelte';
	import {
		handleMaxConcurrentSelectionChange,
		handleMergeModeChange,
	} from '../jobControls';
	import JobControlsIsland from '../jobControls/JobControlsIsland.svelte';
</script>

<section
	class="left-column-panel input-workflow input-workflow-panel flex flex-col gap-2 mb-2"
	data-testid="input-workflow-panel"
	aria-label="Input and File Order"
>
	<div class="flex items-center justify-between">
		<h3 class="section-title mb-0 whitespace-nowrap mr-2">Input and File Order</h3>
		<JobControlsIsland
			onMergeModeChange={handleMergeModeChange}
			onMaxConcurrentSelectionChange={handleMaxConcurrentSelectionChange}
		/>
	</div>

	<FileImportIsland />
</section>

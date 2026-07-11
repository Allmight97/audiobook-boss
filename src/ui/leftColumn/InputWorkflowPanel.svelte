<script lang="ts">
	import { FileListIsland, type ReadWorkActivityByInputId } from '../fileList';
	import FileImportIsland from '../fileImport/FileImportIsland.svelte';

	interface Props {
		readWorkActivityByInputId?: ReadWorkActivityByInputId;
	}

	let { readWorkActivityByInputId }: Props = $props();
</script>

<section
	class="left-column-panel input-workflow input-workflow-panel flex flex-col gap-2 mb-2"
	data-testid="input-workflow-panel"
	aria-label="Input and File Order"
>
	<h3 class="section-title mb-0 whitespace-nowrap mr-2">Input and File Order</h3>

	<FileImportIsland>
		{#snippet children(dropTarget)}
			<FileListIsland {...dropTarget} {readWorkActivityByInputId} />
		{/snippet}
	</FileImportIsland>
</section>

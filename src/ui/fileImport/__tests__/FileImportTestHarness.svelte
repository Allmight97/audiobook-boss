<script lang="ts">
	import { FileListIsland } from '../../fileList';
	import FileImportIsland from '../FileImportIsland.svelte';
</script>

<FileImportIsland>
	{#snippet children(dropTarget)}
		<FileListIsland {...dropTarget} />
	{/snippet}
</FileImportIsland>

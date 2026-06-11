<script lang="ts">
	import { onMount } from 'svelte';
	import { FileListIsland, getCurrentFileList } from '../fileList';
	import type { DragDropContext } from './handlers';
	import {
		attachTauriDragHandlers,
		handleClickToSelect,
		handleClickToSelectFolder,
	} from './handlers';
	import { fileImportUiState } from './state.svelte';
	import RemoteSourceAcquireIsland from '../remoteSource/RemoteSourceAcquireIsland.svelte';

	let fileManagementContainer = $state<HTMLDivElement | null>(null);

	const context: DragDropContext = {
		getCoverArtArea: () => document.getElementById('cover-art-area'),
		getFileManagementContainer: () =>
			fileManagementContainer ??
			document.querySelector<HTMLDivElement>('.file-management-container'),
		getVisibleFiles: () => getCurrentFileList()?.files ?? [],
	};

	onMount(() => {
		return attachTauriDragHandlers(context);
	});

	function handleHeaderClick(): void {
		void handleClickToSelect(getCurrentFileList()?.files ?? []);
	}

	function handleFolderClick(): void {
		void handleClickToSelectFolder(getCurrentFileList()?.files ?? []);
	}

	function handleHeaderKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			void handleClickToSelect(getCurrentFileList()?.files ?? []);
		}
	}
</script>

<div class="flex items-center justify-end gap-2 mb-2">
	<button
		id="add-folder-btn"
		class="btn-pill btn-pill-secondary"
		onclick={handleFolderClick}
	>
		Add Folder
	</button>
	<RemoteSourceAcquireIsland />
</div>

<div
	id="file-import-error"
	class="error-message mb-3"
	style:display={fileImportUiState.errorMessage ? 'block' : 'none'}
>
	{fileImportUiState.errorMessage}
</div>

<FileListIsland
	bind:fileManagementContainer
	isDragOver={fileImportUiState.isDragOver}
	supportText={fileImportUiState.supportText}
	onHeaderClick={handleHeaderClick}
	onHeaderKeydown={handleHeaderKeydown}
/>

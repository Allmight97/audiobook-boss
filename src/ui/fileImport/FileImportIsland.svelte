<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import { getCurrentFileList } from '../fileList';
	import type { DragDropContext } from './handlers';
	import { attachTauriDragHandlers, handleClickToSelect } from './handlers';
	import { fileImportUiState } from './state.svelte';

	type FileImportDropTargetProps = {
		isDragOver: boolean;
		supportText: string;
		onHeaderClick: () => void;
		onHeaderKeydown: (event: KeyboardEvent) => void;
		onFileManagementContainerChange: (container: HTMLDivElement | null) => void;
	};

	interface Props {
		children: Snippet<[FileImportDropTargetProps]>;
	}

	let { children }: Props = $props();
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

	function handleHeaderKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			void handleClickToSelect(getCurrentFileList()?.files ?? []);
		}
	}
</script>

<div
	id="file-import-error"
	class="error-message mb-3"
	style:display={fileImportUiState.errorMessage ? 'block' : 'none'}
>
	{fileImportUiState.errorMessage}
</div>

{@render children({
	isDragOver: fileImportUiState.isDragOver,
	supportText: fileImportUiState.supportText,
	onHeaderClick: handleHeaderClick,
	onHeaderKeydown: handleHeaderKeydown,
	onFileManagementContainerChange: (container) => {
		fileManagementContainer = container;
	},
})}

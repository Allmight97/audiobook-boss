<script lang="ts">
	import { onMount } from 'svelte';
	import { clearAllFiles, toggleFileSort } from '../fileList';
	import {
		onFileListClick,
		onFileListDragEnd,
		onFileListDragOver,
		onFileListDragStart,
		onFileListDrop,
		onFileListKeyDown,
	} from '../fileList/events';
	import type { DragDropContext } from './handlers';
	import { attachTauriDragHandlers, handleClickToSelect } from './handlers';
	import { fileImportUiState } from './state.svelte';

	let dropZoneHeader: HTMLDivElement | null = null;
	let fileManagementContainer: HTMLDivElement | null = null;

	const context: DragDropContext = {
		getDropZoneHeader: () => dropZoneHeader,
		getCoverArtArea: () => document.getElementById('cover-art-area'),
		getFileManagementContainer: () => fileManagementContainer,
	};

	onMount(() => {
		return attachTauriDragHandlers(context);
	});

	function handleHeaderClick(): void {
		void handleClickToSelect();
	}

	function handleHeaderKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			void handleClickToSelect();
		}
	}

	function handleSortClick(): void {
		toggleFileSort();
	}

	function handleClearClick(): void {
		clearAllFiles();
	}
</script>

<svelte:window on:keydown={onFileListKeyDown} />

<div class="flex flex-col gap-2 mb-2">
  <div class="flex items-center justify-end gap-2">
    <div class="flex items-center gap-2 mr-auto self-center pl-1">
      <span class="text-xs muted-text italic" id="file-count-display">0 files</span>
      <span
        class="text-xs muted-text italic"
        id="file-order-lock"
        style="display: none"
        data-testid="file-order-lock"
      >
        Order locked while processing
      </span>
    </div>
    <button
      id="sort-toggle-btn"
      class="btn-pill btn-pill-secondary"
      on:click={handleSortClick}
    >
      Sort: A-Z
    </button>
    <button
      id="clear-files-btn"
      class="btn-pill btn-pill-secondary"
      style="display: none"
      on:click={handleClearClick}
    >
      Clear
    </button>
  </div>
</div>

<div
  id="file-import-error"
  class="error-message mb-3"
  style:display={fileImportUiState.errorMessage ? 'block' : 'none'}
>
  {fileImportUiState.errorMessage}
</div>

<div
  class="file-management-container mb-3"
  role="region"
  aria-label="File list"
  bind:this={fileManagementContainer}
>
  <div
    bind:this={dropZoneHeader}
    class="drop-zone-header"
    data-has-files={fileImportUiState.hasFiles.toString()}
    role="button"
    aria-label="Add audio files"
    tabindex="0"
    on:click={handleHeaderClick}
    on:keydown={handleHeaderKeydown}
  >
    <p class="text-sm muted-text">Drag & Drop files here or Click to Select</p>
    <p class="text-xs muted-text mt-1">Supports: mp3, m4a, m4b, aac</p>
  </div>

  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
  <div
    class="file-list-content"
    role="list"
    aria-label="Audio files"
    on:click={onFileListClick}
    on:dragstart={onFileListDragStart}
    on:dragover={onFileListDragOver}
    on:drop={onFileListDrop}
    on:dragend={onFileListDragEnd}
  >
    <!-- File items rendered here -->
  </div>
</div>

<script lang="ts">
	import { onMount } from 'svelte';
	import { formatDuration, formatFileSize } from '../../types/audio';
	import { clearAllFiles, toggleFileSort } from '../fileList';
	import {
		onFileListClick,
		onFileListDragEnd,
		onFileListDragOver,
		onFileListDragStart,
		onFileListDrop,
		onFileListKeyDown,
		onFileListMoveDown,
		onFileListMoveUp,
		onFileListRemove,
	} from '../fileList/events';
	import { fileListViewState } from '../fileList/viewState.svelte';
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

	function getFileName(path: string): string {
		return path.split(/[\\/]/).pop() || path;
	}

	function formatFileDetails(file: (typeof fileListViewState.files)[number]): string {
		if (file.isValid && file.duration && file.size) {
			return `${formatDuration(file.duration)} • ${formatFileSize(file.size)} • ${file.format}`;
		}
		return `Error: ${file.error || 'Invalid file'}`;
	}
</script>

<svelte:window on:keydown={onFileListKeyDown} />

<div class="flex flex-col gap-2 mb-2">
  <div class="flex items-center justify-end gap-2">
    <div class="flex items-center gap-2 mr-auto self-center pl-1">
      <span class="text-xs muted-text italic" id="file-count-display">
        {fileListViewState.files.length} {fileListViewState.files.length === 1 ? 'file' : 'files'}
      </span>
      <span
        class="text-xs muted-text italic"
        id="file-order-lock"
        style:display={fileListViewState.orderLockVisible ? 'inline' : 'none'}
        data-testid="file-order-lock"
      >
        Order locked while processing
      </span>
    </div>
    <button
      id="sort-toggle-btn"
      class="btn-pill btn-pill-secondary"
      style:display={fileListViewState.showSortButton ? 'block' : 'none'}
      disabled={fileListViewState.sortDisabled}
      on:click={handleSortClick}
    >
      {fileListViewState.sortLabel}
    </button>
    <button
      id="clear-files-btn"
      class="btn-pill btn-pill-secondary"
      style:display={fileListViewState.showClearButton ? 'block' : 'none'}
      disabled={fileListViewState.clearDisabled}
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
  <div class="file-list-content" role="list" aria-label="Audio files">
    {#each fileListViewState.files as file, index (file.path)}
      <div
        class="file-list-item {file.isValid ? 'valid' : 'invalid'}"
        class:selected={fileListViewState.selectedIndices.includes(index)}
        draggable={fileListViewState.orderLockVisible ? 'false' : 'true'}
        role="listitem"
        aria-label={getFileName(file.path)}
        on:click={(event) => onFileListClick(index, event)}
        on:dragstart={(event) => onFileListDragStart(index, event)}
        on:dragover={(event) => onFileListDragOver(index, event)}
        on:drop={(event) => onFileListDrop(index, event)}
        on:dragend={onFileListDragEnd}
      >
        <div class="file-item-content">
          <div class="file-status {file.isValid ? 'text-green-500' : 'text-red-500'}">
            {file.isValid ? '✓' : '✗'}
          </div>
          <div class="file-info">
            <div class="file-name">{getFileName(file.path)}</div>
            <div class="file-details">{formatFileDetails(file)}</div>
          </div>
          <button
            class="move-up-btn"
            on:click={(event) => onFileListMoveUp(index, event)}
            disabled={index === 0 || fileListViewState.orderLockVisible}
          >
            ▲
          </button>
          <button
            class="move-down-btn"
            on:click={(event) => onFileListMoveDown(index, event)}
            disabled={index === fileListViewState.files.length - 1 || fileListViewState.orderLockVisible}
          >
            ▼
          </button>
          <button
            class="remove-file-btn"
            disabled={fileListViewState.orderLockVisible}
            on:click={(event) => onFileListRemove(index, event)}
          >
            ×
          </button>
        </div>
      </div>
    {/each}
  </div>
</div>

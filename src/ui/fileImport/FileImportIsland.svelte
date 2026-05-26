<script lang="ts">
	import { onMount } from 'svelte';
	import { formatDuration, formatFileSize } from '../../types/audio';
	import { clearAllFiles, toggleFileSort } from '../fileList/actions';
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
	import {
		attachTauriDragHandlers,
		handleClickToSelect,
		handleClickToSelectFolder,
	} from './handlers';
	import { fileImportUiState } from './state.svelte';

	let dropZoneHeader: HTMLDivElement | null = null;
	let fileManagementContainer: HTMLDivElement | null = null;

	const context: DragDropContext = {
		getCoverArtArea: () => document.getElementById('cover-art-area'),
		getFileManagementContainer: () => fileManagementContainer,
		getVisibleFiles: () => [...fileListViewState.files],
	};

	onMount(() => {
		return attachTauriDragHandlers(context);
	});

	function handleHeaderClick(): void {
		void handleClickToSelect([...fileListViewState.files]);
	}

	function handleFolderClick(): void {
		void handleClickToSelectFolder([...fileListViewState.files]);
	}

	function handleHeaderKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			void handleClickToSelect([...fileListViewState.files]);
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
      id="add-folder-btn"
      class="btn-pill btn-pill-secondary"
      on:click={handleFolderClick}
    >
      Add Folder
    </button>
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

<style>
	.file-management-container {
		display: flex;
		flex: 1 1 auto;
		flex-direction: column;
		min-height: 8rem;
		overflow: hidden;
		border: 1px solid var(--border-primary);
		border-radius: 0.375rem;
		background-color: var(--bg-input);
	}

	.drop-zone-header[data-has-files='false'] {
		display: flex;
		flex: 1 1 auto;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 2rem 1rem;
		border: 2px dashed var(--border-secondary);
		background-color: var(--bg-drag-area);
		cursor: pointer;
		transition: all 0.2s ease;
	}

	.drop-zone-header[data-has-files='false']:hover,
	.drop-zone-header[data-has-files='false'].drag-over {
		border-color: var(--accent-primary);
		background-color: var(--bg-hover);
	}

	.drop-zone-header[data-has-files='false'].drag-over {
		transform: scale(1.02);
	}

	.drop-zone-header[data-has-files='false']:focus {
		outline: 2px solid var(--border-focus);
		outline-offset: 2px;
	}

	.drop-zone-header[data-has-files='true'] {
		flex-shrink: 0;
		min-height: 2.5rem;
		padding: 0.5rem 1rem;
		border-bottom: 1px solid var(--border-primary);
		background-color: var(--bg-input);
		cursor: pointer;
		transition: all 0.2s ease;
	}

	.drop-zone-header[data-has-files='true']:hover {
		background-color: var(--bg-hover);
	}

	.drop-zone-header[data-has-files='true']:focus {
		outline: 2px solid var(--border-focus);
		outline-offset: -2px;
	}

	.file-list-content {
		flex: 1 1 auto;
		min-height: 0;
		overflow-y: auto;
	}

	.file-list-item {
		padding: 0.75rem;
		border-bottom: 1px solid var(--border-primary);
		cursor: pointer;
		transition: background-color 0.2s ease;
		user-select: none;
	}

	.file-list-item:last-child {
		border-bottom: none;
	}

	.file-list-item:hover {
		background-color: var(--bg-hover);
	}

	.file-list-item.selected {
		background-color: var(--accent-primary);
		color: var(--text-inverse);
	}

	.file-list-item.dragging {
		opacity: 0.5;
	}

	.file-list-item.drag-over {
		border-top: 2px solid var(--accent-primary);
	}

	.file-item-content {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		min-width: 0;
	}

	.file-status {
		min-width: 1rem;
		font-size: 1rem;
		font-weight: bold;
	}

	.file-info {
		flex: 1;
		min-width: 0;
	}

	.file-name {
		margin-bottom: 0.25rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 0.875rem;
		font-weight: 500;
	}

	.file-details {
		overflow: hidden;
		color: var(--text-muted);
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 0.75rem;
	}

	.file-list-item.selected .file-details {
		color: var(--text-inverse);
		opacity: 0.9;
	}

	.remove-file-btn,
	.move-up-btn,
	.move-down-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 1.5rem;
		height: 1.5rem;
		padding: 0.25rem;
		border: none;
		border-radius: 0.25rem;
		background: none;
		color: var(--text-muted);
		font-size: 1.25rem;
		font-weight: bold;
		cursor: pointer;
	}

	.remove-file-btn:hover {
		background-color: rgb(239 68 68 / 0.1);
		color: #ef4444;
	}

	.move-up-btn:hover,
	.move-down-btn:hover {
		background-color: var(--bg-hover);
		color: var(--accent-primary);
		transform: translateY(-1px);
	}

	.move-up-btn:focus-visible,
	.move-down-btn:focus-visible {
		outline: 2px solid var(--border-focus);
		outline-offset: 2px;
	}

	.move-up-btn:disabled,
	.move-down-btn:disabled {
		opacity: 0.3;
		cursor: not-allowed;
	}

	.move-up-btn:disabled:hover,
	.move-down-btn:disabled:hover {
		background-color: transparent;
		color: var(--text-muted);
		transform: none;
	}

	.file-list-item.selected .remove-file-btn,
	.file-list-item.selected .move-up-btn,
	.file-list-item.selected .move-down-btn {
		color: var(--text-inverse);
		opacity: 0.8;
	}

	.file-list-item.selected .remove-file-btn:hover,
	.file-list-item.selected .move-up-btn:hover,
	.file-list-item.selected .move-down-btn:hover {
		background-color: rgb(255 255 255 / 0.2);
		color: var(--text-inverse);
		opacity: 1;
	}

	.file-list-item.selected .move-up-btn:disabled,
	.file-list-item.selected .move-down-btn:disabled {
		color: var(--text-inverse);
	}
</style>

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
    class:drag-over={fileImportUiState.isDragOver}
    data-has-files={fileImportUiState.hasFiles.toString()}
    role="button"
    aria-label="Add audio files"
    tabindex="0"
    on:click={handleHeaderClick}
    on:keydown={handleHeaderKeydown}
  >
    <p class="text-sm muted-text">Drag & Drop files or folders here or Click to Select</p>
    <p class="text-xs muted-text mt-1">{fileImportUiState.supportText}</p>
  </div>

  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
  <div class="file-list-content" role="list" aria-label="Audio files">
    {#each fileListViewState.files as file, index (file.path)}
      <div
        class="file-list-item {file.isValid ? 'valid' : 'invalid'}"
        class:selected={fileListViewState.selectedIndices.includes(index)}
        class:dragging={fileListViewState.draggedIndex === index}
        class:drag-over={fileListViewState.hoveredIndex === index}
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

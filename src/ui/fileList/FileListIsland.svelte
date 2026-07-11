<script lang="ts">
	import { pathBasename } from '../../lib/path/basename';
	import { coverArtBytesToDataUrl } from '../../lib/media/coverArtDataUrl';
	import { formatDuration, formatFileSize } from '../../types/audio';
	import { getMetadataForFile } from '../metadataSession';
	import { hasSupplementalAssetsForInputId } from '../remoteSource';
	import { applySelectionIntent, clearAllFiles, toggleFileSort } from './actions';
	import {
		createFileListDragHandlers,
		onFileListKeyDown,
		onFileListMoveDown,
		onFileListMoveUp,
		onFileListRemove,
	} from './events';
	import { getCurrentFileList, getSelectedFileIndex } from './state.svelte';
	import {
		readFileListControlsSnapshot,
		readFileListOrderLockVisible,
		readFileListSelectedIndices,
		readFileListSortLabel,
		readFileListStatusBadge,
		readFileListViewFiles,
		type ReadWorkActivityByInputId,
	} from './viewState.svelte';

	interface Props {
		isDragOver?: boolean;
		supportText?: string;
		onHeaderClick?: () => void;
		onHeaderKeydown?: (event: KeyboardEvent) => void;
		onFileManagementContainerChange?: (container: HTMLDivElement | null) => void;
		readWorkActivityByInputId?: ReadWorkActivityByInputId;
	}

	let {
		isDragOver = false,
		supportText = '',
		onHeaderClick,
		onHeaderKeydown,
		onFileManagementContainerChange,
		readWorkActivityByInputId,
	}: Props = $props();

	let fileManagementContainer: HTMLDivElement | null = null;
	let selectAllEl: HTMLInputElement | null = null;
	let draggedIndex = $state<number | null>(null);
	let hoveredIndex = $state<number | null>(null);

	const files = $derived(readFileListViewFiles());
	const selectedIndices = $derived(readFileListSelectedIndices());
	const sortLabel = $derived(readFileListSortLabel());
	const controls = $derived(readFileListControlsSnapshot());
	const orderLockVisible = $derived(readFileListOrderLockVisible());
	const hasFiles = $derived((getCurrentFileList()?.files.length ?? 0) > 0);
	const selectedCount = $derived(selectedIndices.length);
	const allSelected = $derived(files.length > 0 && selectedCount === files.length);
	const selectAllIndeterminate = $derived(selectedCount > 0 && selectedCount < files.length);

	const dragHandlers = createFileListDragHandlers((state) => {
		draggedIndex = state.draggedIndex;
		hoveredIndex = state.hoveredIndex;
	});

	$effect(() => {
		onFileManagementContainerChange?.(fileManagementContainer);
	});

	$effect(() => {
		if (selectAllEl) {
			selectAllEl.indeterminate = selectAllIndeterminate;
		}
	});

	function selectionIntentForRow(index: number, event: MouseEvent) {
		if (event.shiftKey) {
			const anchorIndex = getSelectedFileIndex();
			return anchorIndex >= 0
				? { type: 'range' as const, anchorIndex, index }
				: { type: 'selectOnly' as const, index };
		}
		if (event.metaKey || event.ctrlKey) return { type: 'toggle' as const, index };
		return { type: 'selectOnly' as const, index };
	}

	function handleRowActivation(index: number, anchor: HTMLElement, event: MouseEvent): void {
		const intent = selectionIntentForRow(index, event);
		void applySelectionIntent(
			intent,
			intent.type === 'selectOnly' ? { openMetadataSurface: true, anchor } : undefined,
		);
	}

	function toggleRow(index: number, event: MouseEvent): void {
		event.stopPropagation();
		void applySelectionIntent(selectionIntentForRow(index, event));
	}

	function toggleSelectAll(checked: boolean): void {
		void applySelectionIntent({ type: checked ? 'selectAll' : 'clear' });
	}

	function handleSortClick(): void {
		void toggleFileSort();
	}

	function handleClearClick(): void {
		clearAllFiles();
	}

	function getFileName(path: string): string {
		return pathBasename(path, { fallback: 'path' });
	}

	function getFileTitle(file: (typeof files)[number]): string {
		return getMetadataForFile(file.path)?.title || getFileName(file.path);
	}

	function getFileAuthor(file: (typeof files)[number]): string {
		return getMetadataForFile(file.path)?.artist || '—';
	}

	function getCoverDataUrl(file: (typeof files)[number]): string | null {
		const coverArt = getMetadataForFile(file.path)?.cover_art;
		return coverArt && coverArt.length > 0 ? coverArtBytesToDataUrl(coverArt) : null;
	}
</script>

<div class="file-list-toolbar">
	<span class="text-xs muted-text italic" id="file-count-display">
		{files.length} {files.length === 1 ? 'file' : 'files'}
	</span>
	<span
		class="text-xs muted-text italic"
		id="file-order-lock"
		style:display={orderLockVisible ? 'inline' : 'none'}
		data-testid="file-order-lock"
	>
		Order locked while processing
	</span>
	<div class="file-list-toolbar-actions">
		<button
			id="sort-toggle-btn"
			class="btn-pill btn-pill-secondary"
			style:display={controls.showSortButton ? 'block' : 'none'}
			disabled={controls.sortDisabled}
			onclick={handleSortClick}
		>
			{sortLabel}
		</button>
		<button
			id="clear-files-btn"
			class="btn-pill btn-pill-secondary"
			style:display={controls.showClearButton ? 'block' : 'none'}
			disabled={controls.clearDisabled}
			onclick={handleClearClick}
		>
			Clear
		</button>
	</div>
</div>

<div
	class="file-management-container mb-3"
	role="region"
	aria-label="File list"
	bind:this={fileManagementContainer}
>
	<div
		class="drop-zone-header"
		class:drag-over={isDragOver}
		data-has-files={hasFiles.toString()}
		role="button"
		aria-label="Add audio files"
		tabindex="0"
		onclick={() => onHeaderClick?.()}
		onkeydown={(event) => onHeaderKeydown?.(event)}
	>
		<p class="text-sm muted-text">Drop files or folders here, click to choose files, or use Add Folder</p>
		<p class="text-xs muted-text mt-1">{supportText}</p>
	</div>

	<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
	<div
		class="file-list-content"
		role="group"
		aria-label="Audio files"
		tabindex="0"
		onkeydown={onFileListKeyDown}
	>
		<table class="file-list-table" data-testid="book-table">
			<thead>
				<tr>
					<th class="file-list-checkbox-cell">
						<input
							bind:this={selectAllEl}
							type="checkbox"
							aria-label="Select all files"
							data-metadata-selection-intent
							checked={allSelected}
							disabled={files.length === 0}
							onchange={(event) => toggleSelectAll(event.currentTarget.checked)}
						/>
					</th>
					<th class="file-list-cover-cell"></th>
					<th>Book</th>
					<th class="file-list-comfortable-only">Author</th>
					<th class="file-list-number">Duration</th>
					<th class="file-list-number file-list-comfortable-only">Size</th>
					<th class="file-list-comfortable-only">Codec</th>
					<th>Status</th>
					<th class="file-list-actions-heading"><span class="sr-only">File actions</span></th>
				</tr>
			</thead>
			<tbody>
				{#each files as file, index (file.inputId ?? file.path)}
					{@const selected = selectedIndices.includes(index)}
					{@const active = index === getSelectedFileIndex()}
					{@const badge = readFileListStatusBadge(file, readWorkActivityByInputId)}
					{@const coverDataUrl = getCoverDataUrl(file)}
					<tr
						data-file-index={index}
						class="file-list-item {file.isValid ? 'valid' : 'invalid'}"
						class:selected
						class:active={active}
						class:dragging={draggedIndex === index}
						class:drag-over={hoveredIndex === index}
						draggable={orderLockVisible ? 'false' : 'true'}
						ondragstart={(event) => dragHandlers.onDragStart(index, event)}
						ondragover={(event) => dragHandlers.onDragOver(index, event)}
						ondrop={(event) => dragHandlers.onDrop(index, event)}
						ondragend={dragHandlers.onDragEnd}
					>
						<td class="file-list-checkbox-cell">
							<input
								type="checkbox"
								aria-label={`Select ${getFileTitle(file)}`}
								data-metadata-selection-intent
								checked={selected}
								onclick={(event) => toggleRow(index, event)}
							/>
						</td>
						<td class="file-list-cover-cell">
							<div class="app-cover-thumb file-list-cover">
								{#if coverDataUrl}
									<img src={coverDataUrl} alt="" />
								{:else}
									<span aria-hidden="true">—</span>
								{/if}
							</div>
						</td>
						<td class="file-list-title-cell">
							<button
								type="button"
								class="file-list-activate"
								aria-pressed={active ?? false}
								aria-label={`Edit metadata for ${getFileTitle(file)}`}
								id={`file-list-row-activate-${index}`}
								data-metadata-selection-intent
								onclick={(event) => handleRowActivation(index, event.currentTarget, event)}
							>
								{getFileTitle(file)}
							</button>
							{#if hasSupplementalAssetsForInputId(file.inputId)}
								<span class="companion-chip" title="Supplemental PDF attached">PDF</span>
							{/if}
						</td>
						<td class="file-list-comfortable-only">{getFileAuthor(file)}</td>
						<td class="file-list-number">{file.duration ? formatDuration(file.duration) : '—'}</td>
						<td class="file-list-number file-list-comfortable-only">
							{file.size ? formatFileSize(file.size) : '—'}
						</td>
						<td class="file-list-comfortable-only">{file.format ?? '—'}</td>
						<td>
							<span title={file.isValid ? undefined : file.error} class:file-work-badge-error={badge.isError} class={`app-badge app-badge-${badge.variant}`}>
								{badge.label}
							</span>
						</td>
						<td class="file-list-actions-cell">
							<button
								class="move-up-btn"
								aria-label={`Move ${getFileTitle(file)} up`}
								disabled={index === 0 || orderLockVisible}
								onclick={(event) => onFileListMoveUp(index, event)}
							>▲</button>
							<button
								class="move-down-btn"
								aria-label={`Move ${getFileTitle(file)} down`}
								disabled={index === files.length - 1 || orderLockVisible}
								onclick={(event) => onFileListMoveDown(index, event)}
							>▼</button>
							<button
								class="remove-file-btn"
								aria-label={`Remove ${getFileTitle(file)}`}
								disabled={orderLockVisible}
								onclick={(event) => onFileListRemove(index, event)}
							>×</button>
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
</div>

<style>
	.file-list-toolbar { display: flex; align-items: center; gap: var(--space-2); margin-bottom: var(--space-2); }
	.file-list-toolbar-actions { display: flex; gap: var(--space-2); margin-left: auto; }
	.file-management-container { display: flex; flex: 1 1 auto; flex-direction: column; min-height: 8rem; overflow: hidden; border: 1px solid var(--border-primary); border-radius: var(--radius-md); background: var(--bg-input); }
	.drop-zone-header { flex-shrink: 0; min-height: 2.5rem; padding: var(--space-2) var(--density-pad); border-bottom: 1px solid var(--border-primary); background: var(--bg-input); cursor: pointer; }
	.drop-zone-header[data-has-files='false'] { display: flex; flex: 1 1 auto; flex-direction: column; align-items: center; justify-content: center; padding: 2rem 1rem; border: 2px dashed var(--border-secondary); background: var(--bg-drag-area); }
	.drop-zone-header:hover, .drop-zone-header.drag-over { background: var(--bg-hover); }
	.drop-zone-header.drag-over { border-color: var(--accent-primary); }
	.file-list-content { flex: 1 1 auto; min-height: 0; overflow-y: auto; overflow-x: auto; }
	.file-list-content:focus-visible { outline: 2px solid var(--border-focus); outline-offset: -2px; }
	.file-list-table { width: 100%; border-collapse: collapse; font-size: var(--density-text); }
	.file-list-table th { position: sticky; top: 0; z-index: 1; padding: var(--space-2) var(--space-3); border-bottom: 1px solid var(--border-primary); background: var(--bg-main); color: var(--text-placeholder); font-size: var(--text-xs); font-weight: 600; letter-spacing: 0.05em; text-align: left; text-transform: uppercase; }
	.file-list-table td { height: var(--density-row-h); padding: 0 var(--space-3); overflow: hidden; border-bottom: 1px solid var(--border-primary); color: var(--text-secondary); text-overflow: ellipsis; white-space: nowrap; }
	.file-list-table tbody tr:hover { background: var(--bg-panel); }
	.file-list-table tbody tr.selected { background: color-mix(in srgb, var(--accent-primary) 12%, transparent); box-shadow: inset 2px 0 0 var(--accent-primary); }
	.file-list-table tbody tr.dragging { opacity: 0.5; }
	.file-list-table tbody tr.drag-over { border-top: 2px solid var(--accent-primary); }
	.file-list-checkbox-cell { width: 1.875rem; }
	.file-list-checkbox-cell input { margin-top: 0; accent-color: var(--accent-primary); }
	.file-list-cover-cell { width: 2.25rem; padding-right: 0 !important; }
	.file-list-cover { --cover-thumb-size: 1.625rem; border: none; }
	.file-list-title-cell { max-width: 0; }
	.file-list-activate { max-width: calc(100% - 2.25rem); margin-top: 0; padding: 0; overflow: hidden; border: 0; background: transparent; color: inherit; cursor: pointer; font: inherit; text-align: left; text-overflow: ellipsis; white-space: nowrap; }
	.file-list-table tbody tr.active .file-list-activate { color: var(--text-primary); font-weight: 600; }
	.file-list-activate:hover { color: var(--text-primary); text-decoration: underline; }
	.companion-chip { margin-left: var(--space-1); padding: 0.0625rem 0.25rem; border: 1px solid var(--accent-primary); border-radius: var(--radius-sm); color: var(--accent-primary); font-size: 0.625rem; font-weight: 600; }
	.file-list-number { color: var(--text-muted) !important; font-family: var(--font-mono); font-size: var(--text-sm); text-align: right !important; }
	.file-list-actions-heading, .file-list-actions-cell { width: 5.5rem; text-align: right; }
	.move-up-btn, .move-down-btn, .remove-file-btn { width: 1.5rem; height: 1.5rem; margin: 0; padding: 0; border: 0; border-radius: var(--radius-sm); background: transparent; color: var(--text-muted); cursor: pointer; }
	.move-up-btn:hover, .move-down-btn:hover { background: var(--bg-hover); color: var(--accent-primary); }
	.remove-file-btn:hover { background: color-mix(in srgb, var(--text-error) 12%, transparent); color: var(--text-error); }
	.move-up-btn:disabled, .move-down-btn:disabled, .remove-file-btn:disabled { opacity: 0.35; cursor: not-allowed; }
	.file-work-badge-error { color: var(--text-error); }
	:global(:root[data-density='compact']) .file-list-comfortable-only { display: none; }
	:global(:root[data-density='compact']) .file-list-cover { --cover-thumb-size: 1.125rem; }
</style>

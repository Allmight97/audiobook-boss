import { displayedTitleForFile, formatFileDetails } from '../../app/inputSession';
import { interpretFileListKeyDown } from '../../app/inputSession/keyboardNavigation';
import { pathBasename } from '../../lib/path/basename';
import { useAppRuntime } from '../../app/runtime';
import { Button } from '../foundation';
import {
	hasSupplementalAssetsForInputId,
	subscribeRemoteSourceSupplementalAssets,
} from '../remoteSource';
import { createSignal, createEffect, For, onCleanup, type JSX } from 'solid-js';
import {
	clearFileListCoverThumbnails,
	getFileListCoverThumbnailState,
	scheduleFileListCoverThumbnails,
	subscribeCoverThumbnails,
} from './coverThumbnails';
import { createFileListPointerReorder, type FileListDragState } from './pointerReorder';
import './fileList.css';

export function FileListView(props: {
	readonly onHeaderClick: () => void;
	readonly fileManagementRef?: (element: HTMLElement | null) => void;
}): JSX.Element {
	const runtime = useAppRuntime();
	const input = runtime.input;
	const metadataView = runtime.metadata.view;
	const view = input.view;
	const selectFile = input.selectFile;
	const selectAll = input.selectAll;
	const clearSelection = input.clearSelection;
	const removeFile = input.removeFile;
	const moveFile = input.moveFile;
	const reorderFiles = input.reorderFiles;
	const toggleSort = input.toggleSort;
	const restoreImportOrder = input.restoreImportOrder;
	const clearAllFiles = input.clearAllFiles;
	const [thumbnailRevision, setThumbnailRevision] = createSignal(0);
	const [assetRevision, setAssetRevision] = createSignal(0);
	const [dragState, setDragState] = createSignal<FileListDragState>({
		draggedIndex: null,
		hoveredIndex: null,
		hoveredEdge: null,
	});
	let fileListContent: HTMLDivElement | null = null;

	const reorderHandlers = createFileListPointerReorder({
		setDragState,
		isBlocked: () => view().orderLocked,
		fileCount: () => view().files.length,
		onReorder: (fromIndex, toIndex) => reorderFiles({ fromIndex, toIndex }),
	});

	onCleanup(() => reorderHandlers.dispose());
	onCleanup(subscribeCoverThumbnails(() => setThumbnailRevision((revision) => revision + 1)));
	onCleanup(
		subscribeRemoteSourceSupplementalAssets(() => setAssetRevision((revision) => revision + 1)),
	);

	createEffect(() => {
		const validPaths = view()
			.files.filter((file) => file.isValid)
			.map((file) => file.path);
		if (validPaths.length === 0) {
			clearFileListCoverThumbnails();
			return;
		}
		scheduleFileListCoverThumbnails(validPaths, async (path) => {
			const thumbnail = await runtime.cover.thumbnail(path);
			return thumbnail?.dataUrl ?? null;
		});
	});

	createEffect(() => {
		const selected = view().selectedIndices;
		const index = selected[selected.length - 1];
		if (typeof index !== 'number') return;
		requestAnimationFrame(() => {
			const selectedItem = fileListContent?.querySelector<HTMLElement>(
				`[data-file-index="${index}"]`,
			);
			selectedItem?.scrollIntoView?.({ block: 'nearest' });
		});
	});

	function isSelected(index: number): boolean {
		return view().selectedIndices.includes(index);
	}

	function hasCompanion(inputId: string | undefined): boolean {
		assetRevision();
		return hasSupplementalAssetsForInputId(inputId);
	}

	function handleFileListClick(index: number, event: MouseEvent): void {
		if (metadataView().saveInProgress) return;
		if (reorderHandlers.consumePostDragClick()) return;
		fileListContent?.focus({ preventScroll: true });
		if (event.shiftKey) window.getSelection()?.removeAllRanges();
		void selectFile({
			index,
			modifiers: { multi: event.metaKey || event.ctrlKey, range: event.shiftKey },
		});
	}

	function handleListKeyDown(event: KeyboardEvent): void {
		const command = interpretFileListKeyDown(event, {
			fileCount: view().fileCount,
			selectedAnchor: view().selectedAnchor,
		});
		if (!command) return;
		if (metadataView().saveInProgress) return;
		event.preventDefault();
		if (command.type === 'navigate') {
			if (view().selectedIndices.length === 1 && view().selectedIndices[0] === command.index) {
				return;
			}
			void selectFile({ index: command.index, modifiers: { multi: false, range: false } });
			return;
		}
		if (command.type === 'selectAll') {
			void selectAll();
			return;
		}
		void clearSelection();
	}

	const drag = () => {
		thumbnailRevision();
		return dragState();
	};

	return (
		<>
			<div class="file-list-toolbar">
				<div class="file-list-toolbar-row">
					<div class="file-list-meta">
						<span class="muted-text file-list-meta-text" id="file-count-display">
							{view().fileCount} {view().fileCount === 1 ? 'file' : 'files'}
						</span>
						<span
							class="muted-text file-list-meta-text"
							id="file-order-lock"
							style={{ display: view().orderLocked ? 'inline' : 'none' }}
							data-testid="file-order-lock"
						>
							Order locked while processing
						</span>
					</div>
					<Button
						id="sort-toggle-btn"
						style={{ display: view().showSortButton ? 'block' : 'none' }}
						disabled={view().orderLocked}
						aria-label={`Sort files ${view().sortDirection === 'ascending' ? 'descending' : 'ascending'}`}
						aria-describedby="file-sort-status"
						onClick={() => toggleSort()}
					>
						{view().sortLabel}
					</Button>
					<span id="file-sort-status" class="sr-only" aria-live="polite">
						{view().sortDirection === 'ascending'
							? 'Files sorted from A to Z.'
							: view().sortDirection === 'descending'
								? 'Files sorted from Z to A.'
								: 'Files are in import order.'}
					</span>
					<Button
						id="restore-import-order-btn"
						style={{ display: view().showRestoreImportOrder ? 'block' : 'none' }}
						disabled={view().orderLocked}
						onClick={() => restoreImportOrder()}
					>
						Restore import order
					</Button>
					<Button
						id="clear-files-btn"
						style={{ display: view().showClearButton ? 'block' : 'none' }}
						disabled={view().orderLocked}
						onClick={() => void clearAllFiles()}
					>
						Clear
					</Button>
				</div>
			</div>
			<section
				class="file-management-container"
				aria-label="File list"
				ref={(element) => props.fileManagementRef?.(element)}
			>
				<button
					type="button"
					class="drop-zone-header"
					classList={{ 'drag-over': view().isDragOver }}
					data-has-files={String(view().hasFiles)}
					aria-label="Add audio files"
					onClick={() => props.onHeaderClick()}
				>
					<span class="muted-text file-list-drop-hint">
						Drop files or folders here, click to choose files, or use Add Folder
					</span>
					<span class="muted-text file-list-support">{view().supportText}</span>
				</button>
				<div
					class="file-list-content"
					role="listbox"
					aria-label="Audio files"
					aria-multiselectable="true"
					tabIndex={0}
					ref={(element) => {
						fileListContent = element;
					}}
					onKeyDown={handleListKeyDown}
				>
					<For each={view().files}>
						{(file, index) => {
							const thumbnail = () => {
								drag();
								return getFileListCoverThumbnailState(file.path);
							};
							return (
								// biome-ignore lint/a11y/useKeyWithClickEvents: listbox owns keyboard; rows are not tab stops
								<div
									data-file-index={index()}
									class="file-list-item"
									classList={{
										valid: file.isValid,
										invalid: !file.isValid,
										selected: isSelected(index()),
										dragging: drag().draggedIndex === index(),
										'drag-over': drag().hoveredIndex === index(),
									}}
									data-drop-edge={
										drag().hoveredIndex === index() ? (drag().hoveredEdge ?? undefined) : undefined
									}
									role="option"
									aria-selected={isSelected(index())}
									aria-label={pathBasename(file.path, { fallback: 'path' })}
									tabIndex={-1}
									onClick={(event) => handleFileListClick(index(), event)}
								>
									<div class="file-item-content">
										<button
											type="button"
											class="file-reorder-grip"
											tabIndex={-1}
											aria-label={`Reorder ${pathBasename(file.path, { fallback: 'path' })}`}
											onPointerDown={(event) => reorderHandlers.onGripPointerDown(index(), event)}
											onClick={(event) => event.stopPropagation()}
										>
											⋮⋮
										</button>
										<div class="file-cover-thumbnail" aria-hidden="true">
											{(() => {
												const thumb = thumbnail();
												return thumb.status === 'ready' ? (
													<img src={thumb.dataUrl} alt="" />
												) : (
													<span>Art</span>
												);
											})()}
										</div>
										<div
											class={`file-status ${file.isValid ? 'file-status-valid' : 'file-status-invalid'}`}
										>
											{file.isValid ? '✓' : '✗'}
										</div>
										<div class="file-info">
											<div class="file-name-row">
												<div class="file-name">{displayedTitleForFile(file)}</div>
												{hasCompanion(file.inputId) ? (
													<span class="companion-chip" title="Supplemental PDF attached">
														PDF
													</span>
												) : null}
											</div>
											<div class="file-details">{formatFileDetails(file)}</div>
										</div>
										<button
											class="move-up-btn"
											type="button"
											disabled={index() === 0 || view().orderLocked}
											onClick={(event) => {
												event.stopPropagation();
												moveFile({ index: index(), direction: 'up' });
											}}
										>
											▲
										</button>
										<button
											class="move-down-btn"
											type="button"
											disabled={index() === view().files.length - 1 || view().orderLocked}
											onClick={(event) => {
												event.stopPropagation();
												moveFile({ index: index(), direction: 'down' });
											}}
										>
											▼
										</button>
										<button
											class="remove-file-btn"
											type="button"
											disabled={view().orderLocked}
											onClick={(event) => {
												event.stopPropagation();
												removeFile(index());
											}}
										>
											×
										</button>
									</div>
								</div>
							);
						}}
					</For>
				</div>
			</section>
		</>
	);
}

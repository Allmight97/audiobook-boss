import {
	clearAllFilesAtom,
	clearSelectionAtom,
	displayedTitleForFile,
	formatFileDetails,
	inputCapabilityAtom,
	inputViewAtom,
	moveFileAtom,
	removeFileAtom,
	reorderFilesAtom,
	restoreImportOrderAtom,
	selectAllAtom,
	selectFileAtom,
	toggleSortAtom,
} from '../../app/inputSession';
import { interpretFileListKeyDown } from '../../app/inputSession/keyboardNavigation';
import { pathBasename } from '../../lib/path/basename';
import { hasSupplementalAssetsForInputId } from '../remoteSource';
import { useAtomSet, useAtomValue } from '../../app/runtime/solid';
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
	const view = useAtomValue(() => inputViewAtom);
	const capability = useAtomValue(() => inputCapabilityAtom);
	const selectFile = useAtomSet(() => selectFileAtom);
	const selectAll = useAtomSet(() => selectAllAtom);
	const clearSelection = useAtomSet(() => clearSelectionAtom);
	const removeFile = useAtomSet(() => removeFileAtom);
	const moveFile = useAtomSet(() => moveFileAtom);
	const reorderFiles = useAtomSet(() => reorderFilesAtom);
	const toggleSort = useAtomSet(() => toggleSortAtom);
	const restoreImportOrder = useAtomSet(() => restoreImportOrderAtom);
	const clearAllFiles = useAtomSet(() => clearAllFilesAtom);
	const [thumbnailRevision, setThumbnailRevision] = createSignal(0);
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

	createEffect(() => {
		const validPaths = view()
			.files.filter((file) => file.isValid)
			.map((file) => file.path);
		if (validPaths.length === 0) {
			clearFileListCoverThumbnails();
			return;
		}
		scheduleFileListCoverThumbnails(validPaths, (path) =>
			capability().readAudioCoverThumbnail(path),
		);
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

	function handleFileListClick(index: number, event: MouseEvent): void {
		if (reorderHandlers.consumePostDragClick()) return;
		fileListContent?.focus({ preventScroll: true });
		if (event.shiftKey) window.getSelection()?.removeAllRanges();
		selectFile({
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
		event.preventDefault();
		if (command.type === 'navigate') {
			if (view().selectedIndices.length === 1 && view().selectedIndices[0] === command.index) {
				return;
			}
			selectFile({ index: command.index, modifiers: { multi: false, range: false } });
			return;
		}
		if (command.type === 'selectAll') {
			selectAll(undefined);
			return;
		}
		clearSelection(undefined);
	}

	const drag = () => {
		thumbnailRevision();
		return dragState();
	};

	return (
		<>
			<div class="flex flex-col gap-2 mb-2">
				<div class="flex items-center justify-end gap-2">
					<div class="flex items-center gap-2 mr-auto self-center pl-1">
						<span class="text-xs muted-text italic" id="file-count-display">
							{view().fileCount} {view().fileCount === 1 ? 'file' : 'files'}
						</span>
						<span
							class="text-xs muted-text italic"
							id="file-order-lock"
							style={{ display: view().orderLocked ? 'inline' : 'none' }}
							data-testid="file-order-lock"
						>
							Order locked while processing
						</span>
					</div>
					<button
						id="sort-toggle-btn"
						class="btn-pill btn-pill-secondary"
						type="button"
						style={{ display: view().showSortButton ? 'block' : 'none' }}
						disabled={view().orderLocked}
						aria-label={`Sort files ${view().sortDirection === 'ascending' ? 'descending' : 'ascending'}`}
						aria-describedby="file-sort-status"
						onClick={() => toggleSort(undefined)}
					>
						{view().sortLabel}
					</button>
					<span id="file-sort-status" class="sr-only" aria-live="polite">
						{view().sortDirection === 'ascending'
							? 'Files sorted from A to Z.'
							: view().sortDirection === 'descending'
								? 'Files sorted from Z to A.'
								: 'Files are in import order.'}
					</span>
					<button
						id="restore-import-order-btn"
						class="btn-pill btn-pill-secondary"
						type="button"
						style={{ display: view().showRestoreImportOrder ? 'block' : 'none' }}
						disabled={view().orderLocked}
						onClick={() => restoreImportOrder(undefined)}
					>
						Restore import order
					</button>
					<button
						id="clear-files-btn"
						class="btn-pill btn-pill-secondary"
						type="button"
						style={{ display: view().showClearButton ? 'block' : 'none' }}
						disabled={view().orderLocked}
						onClick={() => clearAllFiles(undefined)}
					>
						Clear
					</button>
				</div>
			</div>
			<section
				class="file-management-container mb-3"
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
					<span class="text-sm muted-text">
						Drop files or folders here, click to choose files, or use Add Folder
					</span>
					<span class="text-xs muted-text mt-1">{view().supportText}</span>
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
										<div class={`file-status ${file.isValid ? 'text-green-500' : 'text-red-500'}`}>
											{file.isValid ? '✓' : '✗'}
										</div>
										<div class="file-info">
											<div class="file-name-row">
												<div class="file-name">{displayedTitleForFile(file)}</div>
												{hasSupplementalAssetsForInputId(file.inputId) ? (
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

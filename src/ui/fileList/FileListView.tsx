import {
	fileIdentityKey,
	formatAudioProperties,
	interpretFileListKeyDown,
} from '../../app/inputSession';
import { useAppRuntime } from '../../app/runtime';
import { coverSrc } from '../../lib/tauri/coverSrc';
import { Button, CoverImage } from '../foundation';
import { createSignal, createEffect, Show, For, onCleanup } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { createFileListPointerReorder, type FileListDragState } from './pointerReorder';
import { TitleSources } from './TitleSources';
import { AudioHandlingControl } from './AudioHandlingControl';
import './fileList.css';

function isInteractiveListTarget(target: EventTarget | null): boolean {
	return (
		target instanceof HTMLElement && !!target.closest('button, input, select, textarea, a, label')
	);
}

export function FileListView(props: {
	readonly onHeaderClick: () => void;
	readonly fileManagementRef?: (element: HTMLElement | null) => void;
}): JSX.Element {
	const runtime = useAppRuntime();
	const input = runtime.input;
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
	const [dragState, setDragState] = createSignal<FileListDragState>({
		draggedKey: null,
		hoveredKey: null,
		hoveredEdge: null,
	});
	let fileListContent: HTMLDivElement | null = null;

	const reorderHandlers = createFileListPointerReorder({
		setDragState,
		isBlocked: () => view().orderLocked,
		rowKeys: () => view().files.map(fileIdentityKey),
		onReorder: (fromIndex, toIndex) => reorderFiles({ fromIndex, toIndex }),
	});

	onCleanup(() => reorderHandlers.dispose());

	createEffect(
		() => view().selectedIndices,
		(selected) => {
			const index = selected[selected.length - 1];
			if (typeof index !== 'number') return;
			requestAnimationFrame(() => {
				const selectedItem = fileListContent?.querySelector<HTMLElement>(
					`[data-file-index="${index}"]`,
				);
				selectedItem?.scrollIntoView?.({ block: 'nearest' });
			});
		},
	);

	function handleFileListClick(index: number, event: MouseEvent): void {
		if (isInteractiveListTarget(event.target)) return;
		if (reorderHandlers.consumePostDragClick()) return;
		fileListContent?.focus({ preventScroll: true });
		if (event.shiftKey) window.getSelection()?.removeAllRanges();
		void selectFile({
			index,
			modifiers: {
				multi: event.metaKey || event.ctrlKey,
				range: event.shiftKey,
			},
		});
	}

	function handleListKeyDown(event: KeyboardEvent): void {
		if (isInteractiveListTarget(event.target)) return;
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
			void selectFile({
				index: command.index,
				modifiers: { multi: false, range: false },
			});
			return;
		}
		if (command.type === 'selectAll') {
			selectAll();
			return;
		}
		clearSelection();
	}

	const drag = dragState;

	return (
		<>
			<div class="file-list-toolbar">
				<div class="file-list-toolbar-row">
					<div class="file-list-meta">
						<span class="muted-text file-list-meta-text" id="file-count-display">
							{view().fileCount} {view().fileCount === 1 ? 'title' : 'titles'} ·{' '}
							{view().sourceFiles.length} files
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
						style={{
							display: view().showRestoreImportOrder ? 'block' : 'none',
						}}
						onClick={() => restoreImportOrder()}
					>
						Restore import order
					</Button>
					<Button
						id="clear-files-btn"
						style={{ display: view().showClearButton ? 'block' : 'none' }}
						disabled={view().orderLocked}
						onClick={() => clearAllFiles()}
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
					class={['drop-zone-header', { 'drag-over': view().isDragOver }]}
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
					tabindex={0}
					ref={(element) => {
						fileListContent = element;
					}}
					onKeyDown={handleListKeyDown}
				>
					<For each={view().files}>
						{(file, index) => {
							const [expanded, setExpanded] = createSignal(false);
							const sources = () => input.sourcesFor(file);
							const grouped = () => sources().length > 1;
							const failed = () => file.status === 'failed';
							const selected = () => input.isSelected(file);
							return (
								// oxlint-disable-next-line jsx-a11y/click-events-have-key-events -- the listbox owns the keyboard; rows are not tab stops.
								<div
									data-file-index={index()}
									data-file-key={fileIdentityKey(file)}
									class={[
										'file-list-item',
										{
											valid: !failed(),
											invalid: failed(),
											selected: selected(),
											dragging: drag().draggedKey === fileIdentityKey(file),
											'drag-over': drag().hoveredKey === fileIdentityKey(file),
										},
									]}
									data-drop-edge={
										drag().hoveredKey === fileIdentityKey(file)
											? (drag().hoveredEdge ?? undefined)
											: undefined
									}
									role="option"
									data-state={file.status}
									aria-selected={selected() ? 'true' : 'false'}
									aria-label={file.label}
									tabindex={-1}
									onClick={(event) => handleFileListClick(index(), event)}
								>
									<div class={['file-item-content', { expanded: grouped() && expanded() }]}>
										<button
											type="button"
											class="file-reorder-grip"
											tabindex={-1}
											aria-label={`Reorder ${file.label}`}
											onPointerDown={(event) =>
												reorderHandlers.onGripPointerDown(fileIdentityKey(file), event)
											}
											onClick={(event) => event.stopPropagation()}
										>
											⋮⋮
										</button>
										<div class="file-cover-thumbnail" aria-hidden="true">
											<CoverImage
												src={file.cover ? coverSrc(file.cover) : null}
												alt=""
												missing={<span>Art</span>}
											/>
										</div>
										<div
											class={`file-status ${!failed() ? 'file-status-valid' : 'file-status-invalid'}`}
										>
											{!failed() ? '✓' : '✗'}
										</div>
										<div class="file-info">
											<div class="file-name-row">
												<button
													type="button"
													class="file-name title-disclosure"
													aria-expanded={grouped() ? (expanded() ? 'true' : 'false') : undefined}
													aria-controls={grouped() ? `title-sources-${index()}` : undefined}
													onClick={async (event) => {
														event.stopPropagation();
														const accepted = await selectFile({
															index: index(),
															modifiers: {
																multi: event.metaKey || event.ctrlKey,
																range: event.shiftKey,
															},
														});
														if (
															accepted &&
															grouped() &&
															!event.metaKey &&
															!event.ctrlKey &&
															!event.shiftKey
														)
															setExpanded((value) => !value);
													}}
												>
													<Show when={grouped()}>
														<svg
															aria-hidden="true"
															class="stack-indicator"
															viewBox="0 0 20 20"
															width="18"
															height="18"
															fill="none"
															stroke="currentColor"
															stroke-width="1.5"
														>
															<path d="M6 3h8M4 6h12" />
															<rect x="2.5" y="9" width="15" height="8" rx="2" />
														</svg>
														<span aria-hidden="true">{expanded() ? '⌄' : '›'}</span>
													</Show>
													<span class="title-disclosure-label">{file.label}</span>
													<Show when={grouped()}>
														<span class="title-source-count">{sources().length} files</span>
													</Show>
												</button>
												{sources().some((source) => input.hasCompanions(source.inputId)) ? (
													<span class="companion-chip" title="Supplemental PDF attached">
														PDF
													</span>
												) : null}
												<AudioHandlingControl
													file={file}
													index={index()}
													orderLocked={view().orderLocked}
												/>
											</div>
											<div class="file-details">{input.detailsFor(file)}</div>
											<div class="file-audio-row">
												<Show when={file.status === 'ready' && !grouped()}>
													<div class="file-details file-audio-details">
														{formatAudioProperties(file)}
													</div>
												</Show>
											</div>
											<Show when={grouped() && expanded()}>
												<TitleSources file={file} id={`title-sources-${index()}`} />
											</Show>
											<Show when={input.cueNoticeFor(file)}>
												{(cue) => (
													<div class="file-cue-details">
														<Show when={cue().summary}>
															<span>{cue().summary}</span>
														</Show>
														<Show when={cue().message}>
															<p>{cue().message}</p>
														</Show>
														<Show when={cue().note}>
															<p>{cue().note}</p>
														</Show>
														<For each={cue().actions} keyed={(action) => action.choice}>
															{(action) => (
																<button
																	type="button"
																	disabled={view().orderLocked}
																	onClick={(event) => {
																		event.stopPropagation();
																		runtime.input.chooseCue(file.inputId, action().choice);
																	}}
																>
																	{action().label}
																</button>
															)}
														</For>
													</div>
												)}
											</Show>
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
												removeFile(file);
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

import {
	displayedTitleForFile,
	formatFileDetails,
	formatAudioProperties,
} from '../../app/inputSession';
import { interpretFileListKeyDown } from '../../app/inputSession/keyboardNavigation';
import { pathBasename } from '../../lib/path/basename';
import { useAppRuntime } from '../../app/runtime';
import { Button } from '../foundation';
import { createSignal, createEffect, Show, For, onCleanup } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { createFileListCoverThumbnails } from './coverThumbnails';
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
	const remoteSource = runtime.remoteSource;
	const metadataView = runtime.metadata.view;
	const view = input.view;
	const capability = input.capability;
	const selectFile = input.selectFile;
	const selectAll = input.selectAll;
	const clearSelection = input.clearSelection;
	const removeFile = input.removeFile;
	const moveFile = input.moveFile;
	const reorderFiles = input.reorderFiles;
	const toggleSort = input.toggleSort;
	const restoreImportOrder = input.restoreImportOrder;
	const clearAllFiles = input.clearAllFiles;
	const audioHandling = input.audioHandling;
	const setAudioHandling = input.setAudioHandling;
	const thumbnails = createFileListCoverThumbnails((path) =>
		capability().readAudioCoverThumbnail(path),
	);
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
	onCleanup(thumbnails.dispose);

	createEffect(
		() => {
			const validPaths = view()
				.files.filter((file) => file.isValid)
				.map((file) => file.path);
			return validPaths;
		},
		(validPaths) => thumbnails.schedule(validPaths),
	);

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

	function isSelected(index: number): boolean {
		return view().selectedIndices.includes(index);
	}

	function hasCompanion(inputId: string | undefined): boolean {
		return remoteSource.hasCompanions(inputId);
	}

	function handleFileListClick(index: number, event: MouseEvent): void {
		if (isInteractiveListTarget(event.target)) return;
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
		if (isInteractiveListTarget(event.target)) return;
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
							const thumbnail = () => {
								return thumbnails.read(file.path);
							};
							return (
								// biome-ignore lint/a11y/useKeyWithClickEvents: listbox owns keyboard; rows are not tab stops
								// biome-ignore lint/a11y/useFocusableInteractive: Solid 2 JSX types expose tabindex, not tabIndex
								<div
									data-file-index={index()}
									class={[
										'file-list-item',
										{
											valid: file.isValid,
											invalid: !file.isValid,
											selected: isSelected(index()),
											dragging: drag().draggedIndex === index(),
											'drag-over': drag().hoveredIndex === index(),
										},
									]}
									data-drop-edge={
										drag().hoveredIndex === index() ? (drag().hoveredEdge ?? undefined) : undefined
									}
									role="option"
									aria-selected={isSelected(index()) ? 'true' : 'false'}
									aria-label={pathBasename(file.path, { fallback: 'path' })}
									tabindex={-1}
									onClick={(event) => handleFileListClick(index(), event)}
								>
									<div class={['file-item-content', { expanded: grouped() && expanded() }]}>
										<button
											type="button"
											class="file-reorder-grip"
											tabindex={-1}
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
													<span class="title-disclosure-label">{displayedTitleForFile(file)}</span>
													<Show when={grouped()}>
														<span class="title-source-count">{sources().length} files</span>
													</Show>
												</button>
												{hasCompanion(file.inputId) ? (
													<span class="companion-chip" title="Supplemental PDF attached">
														PDF
													</span>
												) : null}
												<Show
													when={
														sources().every((source) => source.preservation?.canPreserve) ||
														input.audioChoiceRequired(file)
													}
												>
													<AudioHandlingControl
														file={file}
														index={index()}
														orderLocked={view().orderLocked}
														setAudioHandling={setAudioHandling}
														handling={audioHandling(file)}
														choiceRequired={input.audioChoiceRequired(file)}
														canPreserve={sources().every(
															(source) => source.preservation?.canPreserve,
														)}
														grouped={grouped()}
														recommended={sources().every(
															(source) => source.preservation?.recommended,
														)}
													/>
												</Show>
											</div>
											<div class="file-details">
												{grouped()
													? formatFileDetails({
															...file,
															duration: sources().reduce((n, f) => n + (f.duration ?? 0), 0),
															size: sources().reduce((n, f) => n + (f.size ?? 0), 0),
															chapters: sources().flatMap((f) => f.chapters ?? []),
														})
													: formatFileDetails(file)}
											</div>
											<div class="file-audio-row">
												<Show when={file.isValid && !grouped()}>
													<div class="file-details file-audio-details">
														{formatAudioProperties(file)}
													</div>
												</Show>
											</div>
											<Show when={grouped() && expanded()}>
												<TitleSources file={file} id={`title-sources-${index()}`} />
											</Show>
											<Show when={file.cueSource}>
												{(cue) => (
													<div class="file-cue-details">
														<span>
															{file.chapterPlan?.chapters.length ?? 0} chapters from{' '}
															{cue().status === 'embeddedPreferred'
																? 'embedded audio'
																: cue().fileName}
														</span>
														<Show when={cue().message}>
															<p>{cue().message}</p>
														</Show>
														<Show when={cue().status === 'needsConfirmation'}>
															<p>
																This CUE uses nonstandard timestamps. Interpret the final field as
																hundredths of a second?
															</p>
															<button
																type="button"
																disabled={view().orderLocked}
																onClick={(event) => {
																	event.stopPropagation();
																	if (file.inputId)
																		runtime.input.chooseCue(file.inputId, 'confirmHundredths');
																}}
															>
																Use hundredths
															</button>
														</Show>
														<Show when={cue().status === 'ignored'}>
															<p>CUE ignored. Converting without its chapters.</p>
														</Show>
														<Show when={cue().status === 'ready'}>
															<p>CUE chapters accepted.</p>
														</Show>
														<Show
															when={
																cue().status !== 'ignored' && cue().status !== 'embeddedPreferred'
															}
														>
															<button
																type="button"
																disabled={view().orderLocked}
																onClick={(event) => {
																	event.stopPropagation();
																	if (file.inputId) runtime.input.chooseCue(file.inputId, 'ignore');
																}}
															>
																Ignore CUE
															</button>
														</Show>
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
												void removeFile(index());
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

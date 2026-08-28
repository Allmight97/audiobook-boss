import { createEffect, createMemo, For, Show } from 'solid-js';
import { pathBasename } from '../../../lib/path/basename';
import {
	fileListNavigationCommandFromKey,
	resolveFileListNavigationTarget,
} from '../keyboardNavigation';
import {
	clearFiles,
	clearSelection,
	removeFile,
	restoreImportOrder,
	selectAll,
	selectFile,
	toggleSort,
} from './session';
import { displayedTitleForFile, readFileListView } from './view';
import {
	clearFileListCoverThumbnails,
	getFileListCoverThumbnailState,
	scheduleFileListCoverThumbnails,
} from './thumbnails';
import './FileListIsland.css';

export type FileListIslandProps = {
	isDragOver?: boolean;
	supportText?: string;
	onHeaderClick?: () => void;
	onHeaderKeydown?: (event: KeyboardEvent) => void;
};

export function FileListIsland(props: FileListIslandProps) {
	const view = createMemo(() => readFileListView());
	createEffect(
		() =>
			view()
				.files.filter((file) => file.isValid)
				.map((file) => file.path),
		(paths) => {
			if (paths.length === 0) clearFileListCoverThumbnails();
			else scheduleFileListCoverThumbnails(paths);
		},
	);

	function onKeyDown(event: KeyboardEvent) {
		const current = view();
		if (!current.files.length) return;
		const command = fileListNavigationCommandFromKey(event);
		if (command) {
			const targetIndex = resolveFileListNavigationTarget({
				command,
				fileCount: current.files.length,
				selectedIndex: current.selectedIndices[current.selectedIndices.length - 1] ?? -1,
			});
			if (targetIndex === null) return;
			event.preventDefault();
			selectFile(targetIndex);
			return;
		}
		const key = event.key.toLowerCase();
		if ((event.metaKey || event.ctrlKey) && key === 'a') {
			event.preventDefault();
			selectAll();
		} else if (key === 'escape') {
			event.preventDefault();
			clearSelection();
		}
	}

	return (
		<>
			<div class="file-list-toolbar">
				<span id="file-count-display">
					{view().files.length} {view().files.length === 1 ? 'file' : 'files'}
				</span>
				<span
					id="file-order-lock"
					data-testid="file-order-lock"
					style={{ display: view().orderLockVisible ? 'inline' : 'none' }}
				>
					Order locked while processing
				</span>
				<button
					type="button"
					id="sort-toggle-btn"
					style={{ display: view().showSortButton ? 'block' : 'none' }}
					disabled={view().sortDisabled}
					aria-label={`Sort files ${view().sortState === 'ascending' ? 'descending' : 'ascending'}`}
					onClick={() => toggleSort()}
				>
					{view().sortLabel}
				</button>
				<button
					type="button"
					id="restore-import-order-btn"
					style={{ display: view().orderDiffersFromImport ? 'block' : 'none' }}
					disabled={view().sortDisabled}
					onClick={() => restoreImportOrder()}
				>
					Restore import order
				</button>
				<button
					type="button"
					id="clear-files-btn"
					style={{ display: view().showClearButton ? 'block' : 'none' }}
					disabled={view().clearDisabled}
					onClick={() => clearFiles()}
				>
					Clear
				</button>
			</div>
			<section class="file-management-container" aria-label="File list">
				{/* biome-ignore lint/a11y: drop zone is a clickable region; Solid 2 JSX uses tabindex */}
				<div
					class={`drop-zone-header${props.isDragOver ? ' drag-over' : ''}`}
					data-has-files={String(view().files.length > 0)}
					role="button"
					aria-label="Add audio files"
					tabindex={0}
					onClick={() => props.onHeaderClick?.()}
					onKeyDown={(event) => props.onHeaderKeydown?.(event)}
				>
					<p>Drop files or folders here, click to choose files, or use Add Folder</p>
					<p>{props.supportText ?? ''}</p>
				</div>
				<div
					class="file-list-content"
					role="listbox"
					aria-label="Audio files"
					aria-multiselectable="true"
					tabindex={0}
					onKeyDown={onKeyDown}
				>
					<For each={view().files} keyed={(file) => file.inputId ?? file.path}>
						{(file, index) => {
							const selected = () => view().selectedIndices.includes(index());
							const readyUrl = () => {
								const state = getFileListCoverThumbnailState(file().path);
								return state.status === 'ready' ? state.dataUrl : undefined;
							};
							return (
								// biome-ignore lint/a11y: listbox option uses Solid 2 tabindex; keyboard stays on the listbox
								<div
									data-file-index={index()}
									class={`file-list-item${selected() ? ' selected' : ''}`}
									role="option"
									aria-selected={selected() ? 'true' : 'false'}
									aria-label={pathBasename(file().path, { fallback: 'path' })}
									tabindex={-1}
									onClick={(event) =>
										selectFile(index(), {
											multi: event.ctrlKey || event.metaKey,
											range: event.shiftKey,
										})
									}
								>
									<div class="file-cover-thumbnail" aria-hidden="true">
										<Show when={readyUrl()} fallback={<span>Art</span>}>
											{(url) => <img src={url()} alt="" />}
										</Show>
									</div>
									<span>{file().isValid ? '✓' : '✗'}</span>
									<span>{displayedTitleForFile(file())}</span>
									<button
										type="button"
										class="remove-file-btn"
										disabled={view().orderLockVisible}
										onClick={(event) => {
											event.stopPropagation();
											removeFile(index());
										}}
									>
										×
									</button>
								</div>
							);
						}}
					</For>
				</div>
			</section>
			<aside data-testid="file-list-inspector" aria-label="Selected file inspector">
				<p data-testid="inspector-context">{view().inspector.contextText}</p>
				<p data-testid="inspector-detail">{view().inspector.contextDetail}</p>
				<p data-testid="inspector-size">{view().inspector.fileSizeText}</p>
				<p data-testid="inspector-combined">{view().combinedSizeText}</p>
			</aside>
		</>
	);
}

import { RegistryContext, useAtomValue } from '@effect/atom-solid';
import { For, Show } from 'solid-js';
import { pathBasename } from '../../../lib/path/basename';
import { formatDuration, formatFileSize, type AudioFile } from '../../../types/audio';
import { AsyncResult } from '../../../lib/effect/appEffect';
import { hasSupplementalAssetsForInputId } from '../../remoteSource';
import {
	fileListNavigationCommandFromKey,
	resolveFileListNavigationTarget,
} from '../keyboardNavigation';
import {
	clearFiles,
	clearSelection,
	fileListRegistry,
	moveFile,
	removeFile,
	restoreImportOrder,
	selectAll,
	selectFile,
	toggleSort,
} from './session';
import { displayedArtistForFile, displayedTitleForFile, fileListViewAtom } from './view';
import { coverThumbnailAtom } from './thumbnails';
import './FileListIsland.css';

export type FileListIslandProps = {
	isDragOver?: boolean;
	supportText?: string;
	onHeaderClick?: () => void;
	onHeaderKeydown?: (event: KeyboardEvent) => void;
};

function fileDetails(file: AudioFile): string {
	const artist = displayedArtistForFile(file);
	const artistPrefix = artist ? `${artist} • ` : '';
	const chapterSuffix = file.chapters?.length
		? ` • ${file.chapters.length} chapter${file.chapters.length === 1 ? '' : 's'}`
		: '';
	if (file.isValid && file.duration && file.size) {
		return `${artistPrefix}${formatDuration(file.duration)} • ${formatFileSize(file.size)} • ${file.format}${chapterSuffix}`;
	}
	return `Error: ${file.error || 'Invalid file'}`;
}

function CoverThumb(props: { path: string }) {
	const thumbnail = useAtomValue(() => coverThumbnailAtom(props.path));
	const dataUrl = () => {
		const result = thumbnail();
		return AsyncResult.isSuccess(result) ? result.value : null;
	};
	return (
		<div class="file-cover-thumbnail" aria-hidden="true">
			<Show when={dataUrl()} fallback={<span>Art</span>}>
				<img src={dataUrl() ?? ''} alt="" />
			</Show>
		</div>
	);
}

function FileRow(props: {
	file: AudioFile;
	index: number;
	selected: boolean;
	fileCount: number;
	orderLocked: boolean;
}) {
	const name = () => pathBasename(props.file.path, { fallback: 'path' });
	return (
		// biome-ignore lint/a11y/useKeyWithClickEvents: listbox owns keyboard navigation for options
		<div
			data-file-index={props.index}
			class="file-list-item"
			classList={{
				valid: props.file.isValid,
				invalid: !props.file.isValid,
				selected: props.selected,
			}}
			role="option"
			aria-selected={props.selected}
			aria-label={name()}
			tabIndex={-1}
			onClick={(event) => {
				if (event.shiftKey) window.getSelection()?.removeAllRanges();
				selectFile(props.index, {
					multi: event.ctrlKey || event.metaKey,
					range: event.shiftKey,
				});
			}}
		>
			<div class="file-item-content">
				<CoverThumb path={props.file.path} />
				<div class={`file-status ${props.file.isValid ? 'text-green-500' : 'text-red-500'}`}>
					{props.file.isValid ? '✓' : '✗'}
				</div>
				<div class="file-info">
					<div class="file-name-row">
						<div class="file-name">{displayedTitleForFile(props.file)}</div>
						<Show when={hasSupplementalAssetsForInputId(props.file.inputId)}>
							<span class="companion-chip" title="Supplemental PDF attached">
								PDF
							</span>
						</Show>
					</div>
					<div class="file-details">{fileDetails(props.file)}</div>
				</div>
				<button
					type="button"
					class="move-up-btn"
					disabled={props.index === 0 || props.orderLocked}
					onClick={(event) => {
						event.stopPropagation();
						moveFile(props.index, -1);
					}}
				>
					▲
				</button>
				<button
					type="button"
					class="move-down-btn"
					disabled={props.index === props.fileCount - 1 || props.orderLocked}
					onClick={(event) => {
						event.stopPropagation();
						moveFile(props.index, 1);
					}}
				>
					▼
				</button>
				<button
					type="button"
					class="remove-file-btn"
					disabled={props.orderLocked}
					onClick={(event) => {
						event.stopPropagation();
						removeFile(props.index);
					}}
				>
					×
				</button>
			</div>
		</div>
	);
}

function FileListView(props: FileListIslandProps) {
	const view = useAtomValue(() => fileListViewAtom);

	function onKeyDown(event: KeyboardEvent) {
		const current = view();
		if (!current.files.length) return;
		const target = event.target;
		if (target instanceof HTMLElement) {
			const tag = target.tagName.toLowerCase();
			if (tag === 'input' || tag === 'textarea') return;
		}
		const command = fileListNavigationCommandFromKey(event);
		if (command) {
			const targetIndex = resolveFileListNavigationTarget({
				command,
				fileCount: current.files.length,
				selectedIndex: current.selectedIndices[current.selectedIndices.length - 1] ?? -1,
			});
			if (targetIndex === null) return;
			event.preventDefault();
			if (current.selectedIndices.length === 1 && current.selectedIndices[0] === targetIndex) {
				return;
			}
			selectFile(targetIndex, { multi: false, range: false });
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
			<div class="flex flex-col gap-2 mb-2">
				<div class="flex items-center justify-end gap-2">
					<div class="flex items-center gap-2 mr-auto self-center pl-1">
						<span class="text-xs muted-text italic" id="file-count-display">
							{view().files.length} {view().files.length === 1 ? 'file' : 'files'}
						</span>
						<span
							class="text-xs muted-text italic"
							id="file-order-lock"
							data-testid="file-order-lock"
							style={{ display: view().orderLockVisible ? 'inline' : 'none' }}
						>
							Order locked while processing
						</span>
					</div>
					<button
						type="button"
						id="sort-toggle-btn"
						class="btn-pill btn-pill-secondary"
						style={{ display: view().showSortButton ? 'block' : 'none' }}
						disabled={view().sortDisabled}
						aria-label={`Sort files ${view().sortState === 'ascending' ? 'descending' : 'ascending'}`}
						aria-describedby="file-sort-status"
						onClick={() => toggleSort()}
					>
						{view().sortLabel}
					</button>
					<span id="file-sort-status" class="sr-only" aria-live="polite">
						{view().sortState === 'ascending'
							? 'Files sorted from A to Z.'
							: view().sortState === 'descending'
								? 'Files sorted from Z to A.'
								: 'Files are in import order.'}
					</span>
					<button
						type="button"
						id="restore-import-order-btn"
						class="btn-pill btn-pill-secondary"
						style={{ display: view().orderDiffersFromImport ? 'block' : 'none' }}
						disabled={view().sortDisabled}
						onClick={() => restoreImportOrder()}
					>
						Restore import order
					</button>
					<button
						type="button"
						id="clear-files-btn"
						class="btn-pill btn-pill-secondary"
						style={{ display: view().showClearButton ? 'block' : 'none' }}
						disabled={view().clearDisabled}
						onClick={() => clearFiles()}
					>
						Clear
					</button>
				</div>
			</div>
			<section class="file-management-container mb-3" aria-label="File list">
				{/* biome-ignore lint/a11y/useSemanticElements: drop zone is a clickable region, not a form button */}
				<div
					class="drop-zone-header"
					classList={{ 'drag-over': Boolean(props.isDragOver) }}
					data-has-files={String(view().files.length > 0)}
					role="button"
					aria-label="Add audio files"
					tabIndex={0}
					onClick={() => props.onHeaderClick?.()}
					onKeyDown={(event) => props.onHeaderKeydown?.(event)}
				>
					<p class="text-sm muted-text">
						Drop files or folders here, click to choose files, or use Add Folder
					</p>
					<p class="text-xs muted-text mt-1">{props.supportText ?? ''}</p>
				</div>
				<div
					class="file-list-content"
					role="listbox"
					aria-label="Audio files"
					aria-multiselectable="true"
					tabIndex={0}
					onKeyDown={onKeyDown}
				>
					<For each={view().files}>
						{(file, index) => (
							<FileRow
								file={file}
								index={index()}
								selected={view().selectedIndices.includes(index())}
								fileCount={view().files.length}
								orderLocked={view().orderLockVisible}
							/>
						)}
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

export function FileListIsland(props: FileListIslandProps) {
	return (
		<RegistryContext.Provider value={fileListRegistry}>
			<FileListView {...props} />
		</RegistryContext.Provider>
	);
}

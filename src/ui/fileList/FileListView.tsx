import { For, type JSX } from 'solid-js';
import {
	displayedTitleForFile,
	formatFileDetails,
	inputViewAtom,
	selectFileAtom,
	type InputViewFile,
} from '../../app/inputSession';
import { useAtomSet, useAtomValue } from '../../app/runtime/solid';
import './fileList.css';

export function FileListView(props: {
	readonly onHeaderClick: () => void;
	readonly fileManagementRef?: (element: HTMLElement | null) => void;
}): JSX.Element {
	const view = useAtomValue(() => inputViewAtom);
	const selectFile = useAtomSet(() => selectFileAtom);

	return (
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
			>
				<For each={view().files}>
					{(file: InputViewFile) => (
						<div
							data-file-index={file.index}
							class="file-list-item"
							classList={{
								valid: file.isValid,
								invalid: !file.isValid,
								selected: file.selected,
							}}
							role="option"
							aria-selected={file.selected}
							aria-label={displayedTitleForFile(file)}
							tabIndex={-1}
							onClick={(event) =>
								selectFile({
									index: file.index,
									modifiers: { multi: event.metaKey || event.ctrlKey, range: event.shiftKey },
								})
							}
							onKeyDown={(event) => {
								if (event.key === 'Enter' || event.key === ' ') {
									event.preventDefault();
									selectFile({
										index: file.index,
										modifiers: { multi: event.metaKey || event.ctrlKey, range: event.shiftKey },
									});
								}
							}}
						>
							<div class="file-item-content">
								<div class="file-cover-thumbnail" aria-hidden="true">
									<span>Art</span>
								</div>
								<div class={`file-status ${file.isValid ? 'text-green-500' : 'text-red-500'}`}>
									{file.isValid ? '✓' : '✗'}
								</div>
								<div class="file-info">
									<div class="file-name-row">
										<div class="file-name">{displayedTitleForFile(file)}</div>
									</div>
									<div class="file-details">{formatFileDetails(file)}</div>
								</div>
							</div>
						</div>
					)}
				</For>
			</div>
		</section>
	);
}

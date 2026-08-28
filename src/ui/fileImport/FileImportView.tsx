import { onCleanup, onMount, Show, type JSX } from 'solid-js';
import {
	hydrateSupportTextAtom,
	importIntentAtom,
	inputViewAtom,
	setDragOverAtom,
} from '../../app/inputSession';
import { useAtomSet, useAtomValue } from '../../app/runtime/solid';
import { FileListView } from '../fileList/FileListView';

export function FileImportView(): JSX.Element {
	const view = useAtomValue(() => inputViewAtom);
	const importIntent = useAtomSet(() => importIntentAtom);
	const hydrateSupportText = useAtomSet(() => hydrateSupportTextAtom);
	const setDragOver = useAtomSet(() => setDragOverAtom);
	let fileManagementContainer: HTMLElement | null = null;

	onMount(() => {
		hydrateSupportText(undefined);
		const onDragOver = (event: DragEvent) => {
			event.preventDefault();
			setDragOver(true);
		};
		const onDragLeave = () => setDragOver(false);
		const onDrop = (event: DragEvent) => {
			event.preventDefault();
			setDragOver(false);
			const files = Array.from(event.dataTransfer?.files ?? [])
				.map((file) => (file as File & { path?: string }).path)
				.filter((path): path is string => Boolean(path));
			if (files.length > 0) {
				importIntent({ type: 'importPaths', paths: files });
			}
		};
		const target = fileManagementContainer;
		target?.addEventListener('dragover', onDragOver);
		target?.addEventListener('dragleave', onDragLeave);
		target?.addEventListener('drop', onDrop);
		onCleanup(() => {
			target?.removeEventListener('dragover', onDragOver);
			target?.removeEventListener('dragleave', onDragLeave);
			target?.removeEventListener('drop', onDrop);
		});
	});

	function handleHeaderClick(): void {
		importIntent({ type: 'pickFiles' });
	}

	function handleFolderClick(): void {
		importIntent({ type: 'pickFolder' });
	}

	return (
		<>
			<div class="flex items-center justify-end gap-2 mb-2">
				<button
					id="add-folder-btn"
					class="btn-pill btn-pill-secondary"
					type="button"
					onClick={handleFolderClick}
				>
					Add Folder
				</button>
			</div>
			<Show when={view().errorMessage}>
				{(message) => (
					<div id="file-import-error" class="error-message mb-3" style={{ display: 'block' }}>
						{message()}
					</div>
				)}
			</Show>
			<FileListView
				onHeaderClick={handleHeaderClick}
				fileManagementRef={(element) => {
					fileManagementContainer = element;
				}}
			/>
		</>
	);
}

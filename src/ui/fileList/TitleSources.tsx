import { createSignal, For, onCleanup } from 'solid-js';
import type { JSX } from '@solidjs/web';
import { useAppRuntime } from '../../app/runtime';
import { fileIdentityKey, formatFileDetails, formatAudioProperties } from '../../app/inputSession';
import { pathBasename } from '../../lib/path/basename';
import type { AudioFile } from '../../lib/generated/tauri';
import { createFileListPointerReorder, type FileListDragState } from './pointerReorder';

export function TitleSources(props: {
	readonly file: AudioFile;
	readonly id: string;
}): JSX.Element {
	const input = useAppRuntime().input;
	let list: HTMLOListElement | undefined;
	const sources = () => input.sourcesFor(props.file);
	const [drag, setDragState] = createSignal<FileListDragState>({
		draggedKey: null,
		hoveredKey: null,
		hoveredEdge: null,
	});
	const reorder = createFileListPointerReorder({
		setDragState,
		isBlocked: () => input.view().orderLocked,
		rowKeys: () => sources().map(fileIdentityKey),
		onReorder: (from, to) => input.reorderSources(props.file, from, to),
		hitTest: (x, y) => {
			const row = document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-source-key]');
			if (!row?.dataset.sourceKey || !list?.contains(row)) return null;
			const rect = row.getBoundingClientRect();
			return {
				key: row.dataset.sourceKey,
				edge: y < rect.top + rect.height / 2 ? 'top' : 'bottom',
			};
		},
	});
	onCleanup(reorder.dispose);
	return (
		<div class="title-sources">
			<ol
				id={props.id}
				ref={(element) => {
					list = element;
				}}
				aria-label="Files in output order"
			>
				<For each={sources()}>
					{(source, index) => (
						<li
							data-source-key={fileIdentityKey(source)}
							class={[{ dragging: drag().draggedKey === fileIdentityKey(source) }]}
							data-drop-edge={
								drag().hoveredKey === fileIdentityKey(source)
									? (drag().hoveredEdge ?? undefined)
									: undefined
							}
						>
							<button
								type="button"
								class="file-reorder-grip"
								aria-label={`Reorder source ${index() + 1}`}
								disabled={input.view().orderLocked}
								onPointerDown={(event) => {
									event.stopPropagation();
									reorder.onGripPointerDown(fileIdentityKey(source), event);
								}}
							>
								⋮⋮
							</button>
							<span class="source-ordinal">{index() + 1}</span>
							<div class="source-facts">
								<strong>{pathBasename(source.path, { fallback: 'path' })}</strong>
								<span>
									{formatFileDetails(source)} · {formatAudioProperties(source)}
								</span>
							</div>
							<button
								type="button"
								aria-label={`Move source ${index() + 1} up`}
								disabled={input.view().orderLocked || index() === 0}
								onClick={() => input.reorderSources(props.file, index(), index() - 1)}
							>
								↑
							</button>
							<button
								type="button"
								aria-label={`Move source ${index() + 1} down`}
								disabled={input.view().orderLocked || index() === sources().length - 1}
								onClick={() => input.reorderSources(props.file, index(), index() + 1)}
							>
								↓
							</button>
						</li>
					)}
				</For>
			</ol>
			<button
				type="button"
				class="ungroup-title"
				disabled={input.view().orderLocked}
				title={`Title edits stay with ${pathBasename(props.file.path, { fallback: 'path' })}. Other files retain their own drafts.`}
				onClick={() => input.ungroup(props.file)}
			>
				Separate into individual titles
			</button>
		</div>
	);
}

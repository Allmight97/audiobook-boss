import { createEffect, createMemo, Show, untrack } from 'solid-js';
import type { JSX } from '@solidjs/web';

import type { AudioFile } from '../../types/audio';

import { useAppRuntime } from '../../app/runtime';
import { CoverArtView } from '../coverArt';
import { MetadataFormView } from '../metadataForm';
import './metadataManager.css';

export function MetadataManagerView(): JSX.Element {
	const runtime = useAppRuntime();
	const inputView = runtime.input.view;
	const view = runtime.metadata.view;

	// Rehydrate only when what hydration reads changes: file identities, validity,
	// and selection. Unrelated input updates (drag-over, order lock) would otherwise
	// re-resolve covers each time. The memo's equality check stops those reruns.
	const hydrationKey = createMemo(() => {
		const input = inputView();
		const id = (file: AudioFile) => `${file.path}\u0001${file.inputId}\u0001${file.isValid}`;
		return [
			input.selectedIndices.join(','),
			input.files.map(id).join('\u0000'),
			input.sourceFiles.map((file) => file.path).join('\u0000'),
		].join('\u0002');
	});
	createEffect(hydrationKey, () => {
		untrack(() => {
			void runtime.metadata.hydrateSelection(document.activeElement);
		});
	});

	const snapshot = () => view().form;

	return (
		<section class="metadata-manager" data-testid="metadata-manager" aria-label="Metadata Manager">
			<div class="section-header">
				<h3>Metadata Manager</h3>
			</div>
			<div
				id="metadata-selection-count"
				class="muted-text metadata-selection-count"
				hidden={snapshot().mode !== 'multi' || snapshot().selectionCount <= 1}
			>
				{snapshot().selectionCount} titles selected
			</div>
			<Show when={view().statusMessage}>
				<p
					class="muted-text metadata-status-message"
					data-testid="metadata-status-message"
					role="status"
				>
					{view().statusMessage}
				</p>
			</Show>
			<div id="metadata-form" data-multi-select={snapshot().mode === 'multi'}>
				<div class="metadata-manager-layout">
					<div class="metadata-cover-cell">
						<CoverArtView />
					</div>
					<div class="metadata-fields-cell">
						<MetadataFormView />
					</div>
				</div>
			</div>
		</section>
	);
}

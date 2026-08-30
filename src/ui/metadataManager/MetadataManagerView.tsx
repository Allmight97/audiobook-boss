import { createEffect, Show, untrack, type JSX } from 'solid-js';
import { useAppRuntime } from '../../app/runtime';
import { CoverArtView } from '../coverArt';
import { MetadataFormView } from '../metadataForm';
import './metadataManager.css';

export function MetadataManagerView(): JSX.Element {
	const runtime = useAppRuntime();
	const inputView = runtime.input.view;
	const jobType = runtime.input.jobType;
	const view = runtime.metadata.view;

	createEffect(() => {
		inputView();
		jobType();
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
				class="text-xs muted-text mb-2"
				hidden={snapshot().mode !== 'multi' || snapshot().selectionCount <= 1}
			>
				{snapshot().selectionCount} files selected
			</div>
			<Show when={view().statusMessage}>
				<p class="text-xs muted-text mb-2" data-testid="metadata-status-message" role="status">
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

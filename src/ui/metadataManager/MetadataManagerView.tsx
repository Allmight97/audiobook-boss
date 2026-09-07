import { createEffect, Show, untrack } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { useAppRuntime } from '../../app/runtime';
import { CoverArtView } from '../coverArt';
import { MetadataFormView } from '../metadataForm';
import './metadataManager.css';

export function MetadataManagerView(): JSX.Element {
	const runtime = useAppRuntime();
	const inputView = runtime.input.view;
	const jobType = runtime.input.jobType;
	const view = runtime.metadata.view;

	createEffect(
		() => ({
			input: inputView(),
			jobType: jobType(),
		}),
		() => {
			untrack(() => {
				void runtime.metadata.hydrateSelection(document.activeElement);
			});
		},
	);

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
				{snapshot().selectionCount} files selected
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

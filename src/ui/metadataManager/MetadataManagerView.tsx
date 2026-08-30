import { createEffect, type JSX } from 'solid-js';
import { hydrateMetadataSelectionAtom, metadataViewAtom } from '../../app/metadataSession';
import { useAppRuntime } from '../../app/runtime';
import { useAtomSet, useAtomValue } from '../../app/runtime/solid';
import { CoverArtView } from '../coverArt';
import { MetadataFormView } from '../metadataForm';
import './metadataManager.css';

export function MetadataManagerView(): JSX.Element {
	const input = useAppRuntime().input;
	const view = useAtomValue(() => metadataViewAtom);
	const inputView = input.view;
	const jobType = input.jobType;
	const hydrate = useAtomSet(() => hydrateMetadataSelectionAtom);

	createEffect(() => {
		inputView();
		jobType();
		void hydrate(document.activeElement);
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

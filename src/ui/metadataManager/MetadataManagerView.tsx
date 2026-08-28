import { createEffect, type JSX } from 'solid-js';
import { hydrateMetadataSelectionAtom, metadataViewAtom } from '../../app/metadataSession';
import { inputViewAtom, jobTypeAtom } from '../../app/inputSession';
import { useAtomSet, useAtomValue } from '../../app/runtime/solid';
import { CoverArtView } from '../coverArt/CoverArtView';
import { MetadataFormView } from '../metadataForm/MetadataFormView';
import './metadataManager.css';

export function MetadataManagerView(): JSX.Element {
	const view = useAtomValue(() => metadataViewAtom);
	const inputView = useAtomValue(() => inputViewAtom);
	const jobType = useAtomValue(() => jobTypeAtom);
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

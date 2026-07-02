<script lang="ts">
	import {
		onClearCoverArt,
		onLoadCoverArtFromFilePicker,
		onLoadCoverArtFromInput,
	} from '../coverArt';
	import CoverArtIsland from '../coverArt/CoverArtIsland.svelte';
	import { saveMetadataFromUI } from '../metadataSession';
	import {
		onMetadataFormActionSelectChange,
		onMetadataFormFieldInput,
	} from '../metadataForm';
	import MetadataFormFieldsIsland from '../metadataForm/MetadataFormFieldsIsland.svelte';
	import { metadataFormState } from '../metadataForm/state.svelte';
	import { MetadataArtifactsIsland } from '../metadataArtifacts';
</script>

<section class="metadata-manager" data-testid="metadata-manager" aria-label="Metadata Manager">
	<div class="section-header">
		<h3>Metadata Manager</h3>
	</div>
	<div
		id="metadata-selection-count"
		class="text-xs muted-text mb-2"
		hidden={metadataFormState.mode !== 'multi' || metadataFormState.selectionCount <= 1}
	>
		{metadataFormState.selectionCount} files selected
	</div>
	<div id="metadata-form" data-multi-select={metadataFormState.mode === 'multi'}>
		<div class="metadata-manager-layout">
			<div class="metadata-cover-cell">
				<CoverArtIsland
					onLoadFromFile={onLoadCoverArtFromFilePicker}
					onLoadFromInput={onLoadCoverArtFromInput}
					onClearCoverArt={onClearCoverArt}
				/>
			</div>
			<div class="metadata-fields-cell">
				<MetadataFormFieldsIsland
					onFieldInput={onMetadataFormFieldInput}
					onActionChange={onMetadataFormActionSelectChange}
					onSaveMetadata={() => {
						void saveMetadataFromUI();
					}}
				/>
				<MetadataArtifactsIsland />
			</div>
		</div>
	</div>
</section>

<style>
	.metadata-manager {
		min-width: 0;
	}

	.metadata-manager-layout {
		display: grid;
		grid-template-columns: minmax(11rem, 0.95fr) minmax(0, 3fr);
		gap: 0.75rem;
		align-items: start;
	}

	.metadata-cover-cell,
	.metadata-fields-cell {
		min-width: 0;
	}

	.metadata-cover-cell {
		display: flex;
		justify-content: center;
	}

	@media (max-width: 900px) {
		.metadata-manager-layout {
			grid-template-columns: 1fr;
		}

		.metadata-cover-cell {
			justify-content: flex-start;
		}
	}
</style>

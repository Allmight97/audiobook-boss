<script lang="ts">
	import EncoderWorkbenchIsland from '../encoderPanel/EncoderWorkbenchIsland.svelte';
	import { OutputPanelIsland } from '../outputPanel';
	import PreviewAudioControls from '../previewAudio/PreviewAudioControls.svelte';
	import { TagPreviewIsland } from '../tagPreview';
</script>

<div class="encoding-workbench-frame">
	<section
		class="encoding-workbench"
		aria-label="Encoding, output, and tags"
		data-testid="encoding-workbench"
	>
		<div class="workbench-block workbench-block-encoder" data-testid="encoding-workbench-encoder">
			<EncoderWorkbenchIsland />
		</div>

		<div class="workbench-block workbench-block-output" data-testid="encoding-workbench-output">
			<OutputPanelIsland variant="workbench" />
		</div>

		<div class="workbench-block workbench-block-tags" data-testid="encoding-workbench-tags">
			<div class="workbench-block-header tags-header">
				<h3>Tags Preview</h3>
				<PreviewAudioControls variant="compact" />
			</div>
			<TagPreviewIsland variant="workbench" />
		</div>
	</section>
</div>

<style>
	.encoding-workbench-frame {
		container-type: inline-size;
		min-width: 0;
	}

	.encoding-workbench {
		display: grid;
		grid-template-columns:
			minmax(0, 0.95fr)
			minmax(0, 1.15fr)
			minmax(0, 1fr);
		gap: 0.375rem;
	}

	.workbench-block {
		min-width: 0;
		min-height: 0;
		padding: 0.45rem;
		border: 1px solid var(--border-primary);
		border-radius: 0.375rem;
		background:
			linear-gradient(180deg, rgba(255, 255, 255, 0.025), rgba(255, 255, 255, 0)),
			var(--bg-panel);
	}

	.workbench-block-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		margin-bottom: 0.375rem;
	}

	.workbench-block-header h3 {
		min-width: 0;
		margin: 0;
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--text-secondary);
		line-height: 1.2;
		overflow-wrap: anywhere;
	}

	.tags-header {
		align-items: center;
		flex-wrap: wrap;
	}

	.workbench-block-tags {
		overflow: visible;
	}

	.workbench-block-tags :global([data-testid="preview-audio-controls"]) {
		margin-left: auto;
	}

	@container (max-width: 760px) {
		.encoding-workbench {
			grid-template-columns: minmax(0, 0.95fr) minmax(0, 1.1fr);
		}

		.workbench-block-tags {
			grid-column: 1 / -1;
		}

		.tags-header {
			flex-wrap: nowrap;
		}
	}

	@container (max-width: 560px) {
		.encoding-workbench {
			grid-template-columns: 1fr;
		}

		.workbench-block-tags {
			grid-column: auto;
		}
	}
</style>

<script lang="ts">
	import { readFileListCount, readCombinedDurationText, readCombinedSizeText } from '../fileList';
	import { PreviewAudioControls } from '../previewAudio';
	import { StatusTransportIsland } from '../statusPanel';
	import { deriveWorkOperationCounts, workCenterState, WorkCenterIsland } from '../workCenter';
	import { toggleOpsDisclosure, toggleOpsPin, type OpsMode } from './mode';

	let mode = $state<OpsMode>('collapsed');
	const counts = $derived(deriveWorkOperationCounts(workCenterState.operations));
	const fileCount = $derived(readFileListCount());
	const durationText = $derived(readCombinedDurationText());
	const sizeText = $derived(readCombinedSizeText());

	function toggleDisclosure(): void {
		mode = toggleOpsDisclosure(mode);
	}

	function togglePin(): void {
		mode = toggleOpsPin(mode);
	}
</script>

<section class:open={mode !== 'collapsed'} class="operations-bar" aria-label="Operations">
	<div class="operations-bar-row">
		<div class="operations-bar-transport">
			<PreviewAudioControls variant="compact" />
			<StatusTransportIsland />
		</div>

		<div class="operations-bar-info" aria-label="File totals">
			{fileCount} {fileCount === 1 ? 'file' : 'files'} · {durationText} · {sizeText}
		</div>

		<div class="operations-bar-meta">
			<span class="app-badge app-badge-info">{counts.running} running</span>
			<span class="app-badge app-badge-muted">{counts.queued} queued</span>
			<span class="app-badge app-badge-ok">{counts.done} done</span>
			<button
				type="button"
				class:on={mode === 'pinned'}
				class="operations-bar-icon"
				aria-label={mode === 'pinned' ? 'Unpin operations' : 'Pin operations open'}
				aria-pressed={mode === 'pinned'}
				onclick={togglePin}
			>
				⚲
			</button>
			<button
				type="button"
				class="operations-bar-icon"
				aria-controls="operations-bar-body"
				aria-expanded={mode !== 'collapsed'}
				aria-label={mode === 'collapsed' ? 'Expand operations' : 'Collapse operations'}
				onclick={toggleDisclosure}
			>
				▾
			</button>
		</div>
	</div>

	<div id="operations-bar-body" class="operations-bar-body">
		<WorkCenterIsland />
	</div>
</section>

<style>
	.operations-bar {
		flex-shrink: 0;
		border-top: 1px solid var(--border-primary);
		background: var(--bg-panel);
	}

	.operations-bar-row,
	.operations-bar-transport,
	.operations-bar-meta {
		display: flex;
		align-items: center;
	}

	.operations-bar-row {
		gap: var(--space-3);
		padding: var(--space-2) var(--density-pad);
	}

	.operations-bar-transport {
		min-width: 0;
		flex: 1 1 32rem;
		gap: var(--space-3);
	}

	.operations-bar-info {
		color: var(--text-muted);
		font-family: var(--font-mono);
		font-size: var(--text-sm);
		white-space: nowrap;
	}

	.operations-bar-meta {
		margin-left: auto;
		gap: var(--space-2);
		flex-wrap: wrap;
		justify-content: flex-end;
	}

	.operations-bar-icon {
		margin-top: 0;
		border: none;
		background: transparent;
		color: var(--text-placeholder);
		font-size: var(--text-sm);
		cursor: pointer;
	}

	.operations-bar-icon.on {
		color: var(--accent-primary-hover);
	}

	.operations-bar-body {
		display: none;
		max-height: 13.75rem;
		overflow: auto;
		padding: 0 var(--density-pad) var(--space-3);
	}

	.operations-bar.open .operations-bar-body {
		display: block;
	}
</style>

<script lang="ts">
	import { onMount } from 'svelte';
	import { initStatusPanel, triggerCancelAllFromStatusPanel } from './controller';
	import { statusPanelViewState } from './viewState.svelte';

	const activeJob = $derived(
		statusPanelViewState.jobItems.find((item) => item.status === 'processing') ??
			statusPanelViewState.jobItems[0],
	);
	const previewLabel = $derived(activeJob?.label ?? 'Preview ready');
	const hasCancellableForegroundJob = $derived(
		statusPanelViewState.jobItems.some((item) => item.canCancel && item.cancelId),
	);

	onMount(() => {
		initStatusPanel();
	});
</script>

<div class="status-transport" aria-label="Preview transport">
	<div class="status-transport-track">
		<div class="app-progress-track" aria-label="Preview progress">
			<div
				class="app-progress-fill status-transport-fill"
				data-testid="status-transport-progress"
				style={`width: ${statusPanelViewState.progressPercentage}%`}
			></div>
		</div>
	</div>
	<div class="status-transport-copy">
		<span class="status-transport-label">{previewLabel} · {statusPanelViewState.progressPercentage.toFixed(0)}%</span>
		<span class="status-transport-status">{statusPanelViewState.statusText}</span>
	</div>
	<button
		type="button"
		class="btn-pill btn-pill-secondary btn-pill-xs"
		disabled={
			statusPanelViewState.cancelAllPending ||
			!statusPanelViewState.isProcessing ||
			!hasCancellableForegroundJob
		}
		onclick={triggerCancelAllFromStatusPanel}
	>
		Cancel All
	</button>
</div>

<style>
	.status-transport {
		display: flex;
		min-width: 0;
		flex: 1;
		align-items: center;
		gap: var(--space-2);
	}

	.status-transport-track {
		min-width: 7rem;
		max-width: 23.75rem;
		flex: 1 1 15rem;
	}

	.status-transport-fill {
		position: relative;
	}

	.status-transport-copy {
		display: flex;
		min-width: 0;
		flex: 1 1 13rem;
		flex-direction: column;
		font-size: var(--text-sm);
	}

	.status-transport-label,
	.status-transport-status {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.status-transport-label {
		color: var(--text-secondary);
	}

	.status-transport-status {
		color: var(--text-muted);
		font-size: var(--text-xs);
	}
</style>

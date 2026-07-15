<script lang="ts">
	import { onMount } from 'svelte';
	import { initStatusPanel, triggerCancelAllFromStatusPanel } from './controller';
	import { STATUS_PANEL_DEFAULT_STEP_COLOR, statusPanelViewState } from './viewState.svelte';

	const activeJob = $derived(
		statusPanelViewState.jobItems.find((item) => item.status === 'processing') ??
			statusPanelViewState.jobItems[0],
	);
	const hasCancellableForegroundJob = $derived(
		statusPanelViewState.jobItems.some((item) => item.canCancel && item.cancelId),
	);

	// One mono transport line. Precedence: showError/showSuccess feedback
	// (never demoted to tooltip) → active processing summary → idle status.
	// The informational step message surfaces as the line's tooltip.
	const feedbackActive = $derived(
		statusPanelViewState.stepColor !== STATUS_PANEL_DEFAULT_STEP_COLOR &&
			statusPanelViewState.stepText.length > 0,
	);
	const transportLine = $derived(
		feedbackActive
			? statusPanelViewState.stepText
			: statusPanelViewState.isProcessing
				? `${activeJob?.label ?? 'Preview'} · ${statusPanelViewState.progressPercentage.toFixed(0)}% · ${statusPanelViewState.statusText}`
				: statusPanelViewState.statusText,
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
	<div class="status-transport-copy" title={statusPanelViewState.stepText || undefined}>
		<span
			class="status-transport-line"
			data-testid="status-transport-line"
			style:color={feedbackActive ? statusPanelViewState.stepColor : undefined}
		>
			{transportLine}
		</span>
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
		align-items: center;
	}

	.status-transport-line {
		overflow: hidden;
		color: var(--text-secondary);
		font-family: var(--font-mono);
		font-size: var(--text-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>

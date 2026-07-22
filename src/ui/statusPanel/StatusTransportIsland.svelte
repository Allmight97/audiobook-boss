<script lang="ts">
	import { onMount } from 'svelte';
	import { initStatusPanel, triggerCancelAllFromStatusPanel } from './controller';
	import { formatEtaRemaining } from './formatting';
	import { STATUS_PANEL_DEFAULT_STEP_COLOR, statusPanelViewState } from './viewState.svelte';

	// One mono transport line. Precedence: active foreground progress (a live
	// preview must never be hidden behind a retained verdict) → showError/
	// showSuccess feedback (never demoted to tooltip) → idle status. The
	// informational step message surfaces as the line's tooltip.
	const feedbackActive = $derived(
		!statusPanelViewState.isProcessing &&
			statusPanelViewState.stepColor !== STATUS_PANEL_DEFAULT_STEP_COLOR &&
			statusPanelViewState.stepText.length > 0,
	);
	const transportTail = $derived(
		statusPanelViewState.etaSeconds !== null
			? formatEtaRemaining(statusPanelViewState.etaSeconds)
			: statusPanelViewState.statusText,
	);
	const transportLine = $derived(
		statusPanelViewState.isProcessing
			? `${statusPanelViewState.foregroundJobLabel ?? 'Preview'} · ${statusPanelViewState.progressPercentage.toFixed(0)}% · ${transportTail}`
			: feedbackActive
				? statusPanelViewState.stepText
				: statusPanelViewState.statusText,
	);

	onMount(() => {
		initStatusPanel();
	});
</script>

<div class="status-transport" aria-label="Preview transport">
	<div class="status-transport-track">
		<div
			class="app-progress-track"
			role="progressbar"
			aria-label="Preview progress"
			aria-valuemin="0"
			aria-valuemax="100"
			aria-valuenow={Math.round(statusPanelViewState.progressPercentage)}
		>
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
	{#if statusPanelViewState.isProcessing && statusPanelViewState.hasCancellableForegroundJob}
		<button
			type="button"
			class="pill pill-ghost pill-xs"
			onclick={triggerCancelAllFromStatusPanel}
		>
			Cancel All
		</button>
	{/if}
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

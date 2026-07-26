<script lang="ts">
	import {
		readFileListCount,
		readCombinedDurationText,
		readCombinedSizeText,
		readFileListOrderLockVisible,
	} from '../fileList';
	import {
		clearStatusPanelRetainedFeedback,
		readStatusTransportProcessing,
		StatusTransportIsland,
	} from '../statusPanel';
	import { formatEtaRemaining } from '../../lib/format/eta';
	import { deriveWorkOperationCounts, workCenterState, WorkCenterIsland } from '../workCenter';
	import { toggleOpsDisclosure, toggleOpsPin, type OpsMode } from './mode';

	let mode = $state<OpsMode>('collapsed');
	const counts = $derived(deriveWorkOperationCounts(workCenterState.operations));
	const fileCount = $derived(readFileListCount());
	const durationText = $derived(readCombinedDurationText());
	const sizeText = $derived(readCombinedSizeText());
	const orderLockVisible = $derived(readFileListOrderLockVisible());
	const statusTransportProcessing = $derived(readStatusTransportProcessing());
	const runningOperation = $derived(
		workCenterState.operations.find(
			(operation) => operation.status === 'running' || operation.status === 'cancelling',
		),
	);

	// Clear retained preview verdicts only on a true background takeover of an
	// idle transport row (F11): a background operation APPEARING while the
	// foreground is not processing. Any verdict retained at that moment
	// predates the takeover and would be stale when the row reverts. A verdict
	// written while an operation is already running (preview finishing during
	// background work) is fresh — it must survive and win precedence once the
	// background row yields.
	let hadRunningOperation = false;
	$effect(() => {
		const hasRunningOperation = runningOperation !== undefined;
		if (hasRunningOperation && !hadRunningOperation && !statusTransportProcessing) {
			clearStatusPanelRetainedFeedback();
		}
		hadRunningOperation = hasRunningOperation;
	});

	function toggleDisclosure(): void {
		mode = toggleOpsDisclosure(mode);
	}

	function togglePin(): void {
		mode = toggleOpsPin(mode);
	}

	function stopPropagation(event: MouseEvent): void {
		event.stopPropagation();
	}

	function stopInteractiveChildPropagation(event: MouseEvent): void {
		const target = event.target;
		if (
			target instanceof Element &&
			target.closest('button, a, input, select, textarea, [role="button"]')
		) {
			event.stopPropagation();
		}
	}

	function isNestedInteractiveTarget(event: KeyboardEvent): boolean {
		const target = event.target;
		if (!(target instanceof Element)) return false;
		const interactive = target.closest('button, a, input, select, textarea, [role="button"]');
		return interactive !== null && interactive !== event.currentTarget;
	}

	function handleRowKeydown(event: KeyboardEvent): void {
		if (event.key !== 'Enter' && event.key !== ' ') return;
		if (isNestedInteractiveTarget(event)) return;
		event.preventDefault();
		toggleDisclosure();
	}
</script>

<section class:open={mode !== 'collapsed'} class="operations-bar" aria-label="Operations">
	<div
		class="operations-bar-row"
		role="button"
		tabindex="0"
		aria-controls="operations-bar-body"
		aria-expanded={mode !== 'collapsed'}
		aria-label="Toggle operations"
		onclick={toggleDisclosure}
		onkeydown={handleRowKeydown}
	>
		<div
			class="operations-bar-transport"
			role="presentation"
			onclick={stopInteractiveChildPropagation}
		>
			{#if !statusTransportProcessing && runningOperation}
				<div class="operations-bar-background-transport" aria-label="Background operation transport">
					<div
						class="app-progress-track operations-bar-background-track"
						role="progressbar"
						aria-label="Background operation progress"
						aria-valuemin="0"
						aria-valuemax="100"
						aria-valuenow={Math.round(runningOperation.progress.percentage)}
					>
						<div
							class="app-progress-fill"
							style={`width: ${runningOperation.progress.percentage}%`}
						></div>
					</div>
					<span class="mono operations-bar-background-line">
						{runningOperation.title} · {runningOperation.progress.percentage.toFixed(0)}%
						{#if runningOperation.progress.etaSeconds != null}
							· {formatEtaRemaining(runningOperation.progress.etaSeconds)}
						{/if}
					</span>
				</div>
			{:else}
				<StatusTransportIsland />
				{#if !statusTransportProcessing && orderLockVisible}
					<span class="mono operations-bar-order-lock">· order locked (submitting)</span>
				{/if}
			{/if}
		</div>

		<div class="operations-bar-meta">
			{#if counts.running > 0 || counts.queued > 0 || counts.done > 0}
				<span class="app-badge app-badge-info">{counts.running} running</span>
				<span class="app-badge app-badge-muted">{counts.queued} queued</span>
				<span class="app-badge app-badge-ok">{counts.done} done</span>
			{/if}
			<span class="operations-bar-info" aria-label="File totals">
				{fileCount}
				{fileCount === 1 ? 'book' : 'books'} · <span class="mono">{durationText} · {sizeText}</span>
			</span>
			<button
				type="button"
				class:on={mode === 'pinned'}
				class="operations-bar-icon"
				aria-label={mode === 'pinned' ? 'Unpin operations' : 'Pin operations open'}
				aria-pressed={mode === 'pinned'}
				onclick={(event) => {
					stopPropagation(event);
					togglePin();
				}}
			>
				⚲ {mode === 'pinned' ? 'pinned' : 'pin'}
			</button>
			<span class="operations-bar-icon" aria-hidden="true">▾</span>
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
		cursor: pointer;
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

	.operations-bar-background-transport {
		display: flex;
		min-width: 0;
		flex: 1;
		align-items: center;
		gap: var(--space-2);
	}

	.operations-bar-background-track {
		height: 5px;
		max-width: 23.75rem;
		flex: 1 1 15rem;
	}

	.operations-bar-background-line,
	.operations-bar-order-lock {
		min-width: 0;
		overflow: hidden;
		color: var(--text-secondary);
		font-size: var(--text-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
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

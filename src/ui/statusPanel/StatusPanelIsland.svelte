<script lang="ts">
	import { onMount } from 'svelte';
	import type { EventStage } from '../../types/events';
	import {
		initStatusPanel,
		triggerCancelAllFromStatusPanel,
		triggerProcessFromStatusPanel,
	} from './controller';
	import { statusPanelViewState } from './viewState.svelte';

	let isQueueExpanded = false;

	onMount(() => {
		initStatusPanel();
	});

	$: if (statusPanelViewState.jobItems.length === 0) {
		isQueueExpanded = false;
	}

	function handleProcessClick(): void {
		triggerProcessFromStatusPanel();
	}

	function handleCancelAllClick(): void {
		triggerCancelAllFromStatusPanel();
	}

	function toggleQueue(): void {
		isQueueExpanded = !isQueueExpanded;
	}

	function getJobPercentageText(item: (typeof statusPanelViewState.jobItems)[number]): string {
		return typeof item.percentage === 'number' ? ` (${item.percentage.toFixed(1)}%)` : '';
	}

	function getJobStatusLabel(item: (typeof statusPanelViewState.jobItems)[number]): string {
		return `${item.statusText}${getJobPercentageText(item)}`;
	}

	function getJobStatusClass(item: (typeof statusPanelViewState.jobItems)[number]): string {
		if (item.status === 'processing') return 'is-active';
		if (item.status === 'queued') return 'is-queued';
		if (item.status === 'completed' || item.status === 'skipped') return 'is-complete';
		if (item.status === 'failed') return 'is-failed';
		if (item.status === 'cancelled') return 'is-cancelled';
		return '';
	}

	function getCount(
		items: typeof statusPanelViewState.jobItems,
		statuses: Array<(typeof statusPanelViewState.jobItems)[number]['status']>,
	): number {
		return items.filter((item) => statuses.includes(item.status)).length;
	}

	function getActiveStageLabel(stage?: EventStage): string {
		if (stage === 'writing') return 'writing';
		if (stage === 'converting') return 'converting';
		if (stage === 'analyzing') return 'analyzing';
		return 'running';
	}

	function getActiveChipLabel(items: typeof statusPanelViewState.jobItems): string | null {
		const activeItems = items.filter((item) => item.status === 'processing');
		if (activeItems.length === 0) return null;
		const labels = new Set(activeItems.map((item) => getActiveStageLabel(item.stage)));
		const label = labels.size === 1 ? Array.from(labels)[0] : 'running';
		return `${activeItems.length} ${label}`;
	}

    function handleCancelJob(item: (typeof statusPanelViewState.jobItems)[number]): void {
      if (!item.canCancel || !item.cancelId || !item.onCancel) return;
      item.onCancel(item.cancelId);
    }

    function hasCancellableForegroundJob(): boolean {
      return statusPanelViewState.jobItems.some((item) => item.canCancel && item.cancelId);
    }
</script>

<div class="panel status-panel">
  <div class="status-panel-content">
    <div class="art-thumbnail">
      {#if statusPanelViewState.coverArtDataUrl}
        <img
          src={statusPanelViewState.coverArtDataUrl}
          alt="Cover Art"
          style="width: 100%; height: 100%; object-fit: cover; border-radius: 0.25rem;"
        />
      {:else}
        <span>Art</span>
      {/if}
    </div>
    <div class="progress-details">
      <div class="flex justify-between mb-0.5">
        <span class="text-xs"
          >Progress:
          <span class="property-value" id="percentage-processed"
            >{statusPanelViewState.progressPercentage.toFixed(1)}%</span
          ></span
        >
        <span id="status-text" class="text-xs font-semibold">{statusPanelViewState.statusText}</span>
      </div>
      <div class="progress-bar-bg">
        <div
          id="progress-bar"
          class="progress-bar-fg"
          style={`width: ${statusPanelViewState.progressPercentage}%`}
        ></div>
      </div>
      <div
        id="step-text"
        class="text-xs muted-text mt-0.5"
        style={`color: ${statusPanelViewState.stepColor}`}
      >
        {statusPanelViewState.stepText}
      </div>
      <div id="concurrency-status" class="text-xs muted-text mt-1">
        {statusPanelViewState.concurrencyText}
      </div>
      {#if statusPanelViewState.jobItems.length > 0}
        <div class="queue-summary-row" id="queue-summary">
          <div class="queue-chip-group" aria-label="Queue status summary">
            {#if getActiveChipLabel(statusPanelViewState.jobItems)}
              <span class="queue-chip is-active" data-testid="queue-chip-active">{getActiveChipLabel(statusPanelViewState.jobItems)}</span>
            {/if}
            {#if getCount(statusPanelViewState.jobItems, ['queued']) > 0}
              <span class="queue-chip is-queued" data-testid="queue-chip-queued">{getCount(statusPanelViewState.jobItems, ['queued'])} queued</span>
            {/if}
            {#if getCount(statusPanelViewState.jobItems, ['completed', 'skipped']) > 0}
              <span class="queue-chip is-complete" data-testid="queue-chip-complete">{getCount(statusPanelViewState.jobItems, ['completed', 'skipped'])} complete</span>
            {/if}
            {#if getCount(statusPanelViewState.jobItems, ['failed']) > 0}
              <span class="queue-chip is-failed" data-testid="queue-chip-failed">{getCount(statusPanelViewState.jobItems, ['failed'])} failed</span>
            {/if}
            {#if getCount(statusPanelViewState.jobItems, ['cancelled']) > 0}
              <span class="queue-chip is-cancelled" data-testid="queue-chip-cancelled">{getCount(statusPanelViewState.jobItems, ['cancelled'])} cancelled</span>
            {/if}
          </div>
          <button
            id="queue-toggle-button"
            class="queue-toggle-button"
            aria-expanded={isQueueExpanded}
            aria-controls="job-list"
            onclick={toggleQueue}
          >
            {isQueueExpanded ? 'Hide queue' : 'View queue'}
          </button>
        </div>
      {/if}
      <div
        id="job-list"
        class="queue-job-list"
        hidden={!isQueueExpanded || statusPanelViewState.jobItems.length === 0}
      >
        {#each statusPanelViewState.jobItems as item (item.key)}
          <div class={`queue-job-row ${getJobStatusClass(item)}`}>
            <span class="queue-job-label">{item.label}</span>
            <span class="queue-job-status">{getJobStatusLabel(item)}</span>
            <button
              id={"cancel-" + item.key}
              class="job-cancel-button"
              disabled={
                statusPanelViewState.cancelAllPending || !item.canCancel || !item.cancelId || !item.onCancel
              }
              onclick={() => handleCancelJob(item)}
            >
              Cancel
            </button>
          </div>
        {/each}
      </div>
    </div>
    <div class="status-actions">
      <button
        id="process-button"
        class="btn-pill btn-pill-primary"
        onclick={handleProcessClick}
      >
        Process Audiobook
      </button>
      <button
        id="cancel-all-button"
        class="btn-pill btn-pill-secondary"
        disabled={
          statusPanelViewState.cancelAllPending ||
          !statusPanelViewState.isProcessing ||
          !hasCancellableForegroundJob()
        }
        onclick={handleCancelAllClick}
      >
        Cancel
      </button>
    </div>
  </div>
</div>

<style>
	.status-panel {
		flex-shrink: 0;
		padding: 0.5rem 0.75rem;
	}

	.status-panel-content {
		display: flex;
		align-items: flex-start;
		gap: 1rem;
	}

	.art-thumbnail {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 42px;
		height: 42px;
		flex-shrink: 0;
		overflow: hidden;
		border: 2px dashed var(--border-secondary);
		border-radius: 0.375rem;
		background-color: var(--bg-drag-area);
		color: var(--text-placeholder);
		font-size: 0.75rem;
		font-weight: 500;
	}

	.art-thumbnail span {
		font-size: 0.75rem;
	}

	.progress-details {
		flex: 1;
		min-width: 0;
	}

	.status-actions {
		display: flex;
		flex-shrink: 0;
		gap: 0.5rem;
	}

	.progress-bar-bg {
		position: relative;
		overflow: hidden;
		height: 6px;
		border-radius: 9999px;
		background-color: var(--progress-bg);
	}

	.progress-bar-fg {
		position: relative;
		height: 100%;
		border-radius: 9999px;
		background: linear-gradient(
			90deg,
			var(--progress-fg),
			color-mix(in srgb, var(--progress-fg) 70%, white 30%)
		);
		transition: width 0.3s ease-in-out;
	}

	.progress-bar-fg::after {
		content: "";
		position: absolute;
		inset: 0;
		background: linear-gradient(
			90deg,
			transparent 0%,
			rgba(255, 255, 255, 0.25) 35%,
			transparent 70%
		);
		animation: progress-shimmer 1.6s linear infinite;
	}

	@keyframes progress-shimmer {
		from {
			transform: translateX(-100%);
		}

		to {
			transform: translateX(100%);
		}
	}

	.job-cancel-button {
		padding: 0.35rem 0.75rem;
		border: none;
		border-radius: 0.5rem;
		background-color: var(--accent-secondary);
		color: var(--text-inverse);
		font-weight: 500;
		cursor: pointer;
		white-space: nowrap;
		transition: all 0.2s ease-in-out;
	}

	.job-cancel-button:hover {
		background-color: var(--accent-secondary-hover);
	}

	.job-cancel-button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.queue-summary-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		margin-top: 0.5rem;
	}

	.queue-chip-group {
		display: flex;
		flex: 1;
		flex-wrap: wrap;
		gap: 0.375rem;
		min-width: 0;
	}

	.queue-chip {
		display: inline-flex;
		align-items: center;
		min-height: 1.35rem;
		padding: 0.15rem 0.55rem;
		border-radius: 9999px;
		font-size: 0.75rem;
		font-weight: 700;
		line-height: 1;
		white-space: nowrap;
	}

	.queue-chip.is-active {
		background: rgba(59, 130, 246, 0.22);
		color: #bfdbfe;
	}

	.queue-chip.is-queued {
		background: rgba(245, 158, 11, 0.18);
		color: #fcd34d;
	}

	.queue-chip.is-complete {
		background: rgba(16, 185, 129, 0.16);
		color: #86efac;
	}

	.queue-chip.is-failed {
		background: rgba(239, 68, 68, 0.18);
		color: #fca5a5;
	}

	.queue-chip.is-cancelled {
		background: rgba(156, 163, 175, 0.2);
		color: var(--text-muted);
	}

	.queue-toggle-button {
		min-height: 2rem;
		padding: 0.25rem 0.75rem;
		border: 1px solid var(--border-secondary);
		border-radius: 0.5rem;
		background: var(--bg-input);
		color: var(--text-primary);
		cursor: pointer;
		font-size: 0.8rem;
		font-weight: 700;
		white-space: nowrap;
		transition: all 0.2s ease-in-out;
	}

	.queue-toggle-button:hover,
	.queue-toggle-button:focus-visible {
		border-color: var(--accent-primary);
		color: var(--accent-primary-hover);
		outline: none;
	}

	.queue-job-list {
		max-height: 16rem;
		margin-top: 0.5rem;
		overflow-y: auto;
		border: 1px solid var(--border-primary);
		border-radius: 0.5rem;
		background: color-mix(in srgb, var(--bg-panel) 78%, var(--bg-main) 22%);
	}

	.queue-job-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto auto;
		align-items: center;
		gap: 0.75rem;
		min-height: 2.5rem;
		padding: 0.45rem 0.6rem;
		border-bottom: 1px solid var(--border-primary);
	}

	.queue-job-row:last-child {
		border-bottom: none;
	}

	.queue-job-row.is-active {
		background: rgba(59, 130, 246, 0.12);
	}

	.queue-job-label {
		min-width: 0;
		overflow: hidden;
		color: var(--text-secondary);
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.queue-job-status {
		color: var(--text-muted);
		font-weight: 600;
		white-space: nowrap;
	}

	.queue-job-row.is-active .queue-job-status {
		color: #bfdbfe;
	}

	.queue-job-row.is-queued .queue-job-status {
		color: #fcd34d;
	}

	.queue-job-row.is-failed .queue-job-status {
		color: #fca5a5;
	}

	@media (max-width: 900px) {
		.status-panel-content,
		.queue-summary-row,
		.status-actions {
			flex-direction: column;
			align-items: stretch;
		}

		.queue-job-row {
			grid-template-columns: minmax(0, 1fr);
		}

		.queue-job-status {
			white-space: normal;
		}
	}
</style>

<script lang="ts">
	import { onMount } from 'svelte';
	import {
		initStatusPanel,
		triggerCancelAllFromStatusPanel,
		triggerProcessFromStatusPanel,
	} from './controller';
	import { statusPanelViewState } from './viewState.svelte';

	onMount(() => {
		initStatusPanel();
	});

	function handleProcessClick(): void {
		triggerProcessFromStatusPanel();
	}

	function handleCancelAllClick(): void {
		triggerCancelAllFromStatusPanel();
	}

	function getJobLabel(item: (typeof statusPanelViewState.jobItems)[number]): string {
		const percentage = typeof item.percentage === 'number' ? ` (${item.percentage.toFixed(1)}%)` : '';
		return `${item.label} • ${item.statusText}${percentage}`;
	}

	function handleCancelJob(item: (typeof statusPanelViewState.jobItems)[number]): void {
		if (!item.canCancel || !item.cancelId || !item.onCancel) return;
		item.onCancel(item.cancelId);
	}

	function getProgressText(): string {
		return `${statusPanelViewState.progressPercentage.toFixed(1)}%`;
	}
</script>

<div class="panel status-panel">
  <div class="status-panel-content">
    <div class="art-thumbnail" data-svelte-owned="true">
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
          <span class="property-value" id="percentage-processed">{getProgressText()}</span></span
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
      <div id="job-list" class="text-xs muted-text mt-1" data-svelte-owned="true">
        {#each statusPanelViewState.jobItems as item (item.key)}
          <div class="flex items-center justify-between gap-2 mb-1">
            <span class="flex-1">{getJobLabel(item)}</span>
            <button
              id={"cancel-" + item.key}
              class="job-cancel-button"
              style="padding: 0.1rem 0.4rem;"
              disabled={
                statusPanelViewState.cancelAllPending || !item.canCancel || !item.cancelId || !item.onCancel
              }
              on:click={() => handleCancelJob(item)}
            >
              Cancel
            </button>
          </div>
        {/each}
      </div>
      <div class="text-xs muted-text mt-1 flex items-center gap-2" style="display: none">
        <!-- Moved Max Concurrent to Header -->
      </div>
    </div>
    <div class="flex gap-2">
      <button
        id="process-button"
        class="btn-pill btn-pill-primary"
        on:click={handleProcessClick}
      >
        Process Audiobook
      </button>
      <button
        id="cancel-all-button"
        class="btn-pill btn-pill-secondary"
        disabled={statusPanelViewState.cancelAllPending}
        on:click={handleCancelAllClick}
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
		align-items: center;
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
		padding: 0.5rem 1rem;
		border: none;
		border-radius: 0.375rem;
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
</style>

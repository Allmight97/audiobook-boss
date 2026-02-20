<script lang="ts">
	import { triggerCancelAllFromStatusPanel, triggerProcessFromStatusPanel } from './logic';
	import { statusPanelViewState } from './viewState.svelte';

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
          <span class="property-value" id="hours-processed">--.-- / --.-- hours</span>
          →
          <span class="property-value" id="percentage-processed">--%</span></span
        >
        <span id="status-text" class="text-xs font-semibold">Idle</span>
      </div>
      <div class="progress-bar-bg">
        <div id="progress-bar" class="progress-bar-fg" style="width: 0%"></div>
      </div>
      <div id="step-text" class="text-xs muted-text mt-0.5">
        Current Step: Waiting for files...
      </div>
      <div id="concurrency-status" class="text-xs muted-text mt-1"></div>
      <div id="job-list" class="text-xs muted-text mt-1" data-svelte-owned="true">
        {#each statusPanelViewState.jobItems as item (item.key)}
          <div class="flex items-center justify-between gap-2 mb-1">
            <span class="flex-1">{getJobLabel(item)}</span>
            <button
              id={"cancel-" + item.key}
              class="button-secondary"
              style="padding: 0.1rem 0.4rem;"
              disabled={!item.canCancel || !item.cancelId || !item.onCancel}
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
        on:click={handleCancelAllClick}
      >
        Cancel
      </button>
    </div>
  </div>
</div>

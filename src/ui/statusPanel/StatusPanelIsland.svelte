<div class="panel status-panel">
  <div class="status-panel-content">
    <div class="art-thumbnail">
      <span>Art</span>
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
      <div id="job-list" class="text-xs muted-text mt-1"></div>
      <div class="text-xs muted-text mt-1 flex items-center gap-2" style="display: none">
        <!-- Moved Max Concurrent to Header -->
      </div>
    </div>
    <div class="flex gap-2">
      <button id="process-button" class="btn-pill btn-pill-primary">Process Audiobook</button>
      <button id="cancel-all-button" class="btn-pill btn-pill-secondary">Cancel</button>
    </div>
  </div>
</div>

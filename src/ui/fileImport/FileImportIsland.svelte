<div class="flex flex-col gap-2 mb-2">
  <div class="flex items-center justify-end gap-2">
    <div class="flex items-center gap-2 mr-auto self-center pl-1">
      <span class="text-xs muted-text italic" id="file-count-display">0 files</span>
      <span
        class="text-xs muted-text italic"
        id="file-order-lock"
        style="display: none"
        data-testid="file-order-lock"
      >
        Order locked while processing
      </span>
    </div>
    <button id="sort-toggle-btn" class="btn-pill btn-pill-secondary">Sort: A-Z</button>
    <button id="clear-files-btn" class="btn-pill btn-pill-secondary" style="display: none">Clear</button>
  </div>
</div>

<div id="file-import-error" class="error-message mb-3" style="display: none"></div>

<div class="file-management-container mb-3" role="region" aria-label="File list">
  <div
    class="drop-zone-header"
    data-has-files="false"
    role="button"
    aria-label="Add audio files"
    tabindex="0"
  >
    <p class="text-sm muted-text">Drag & Drop files here or Click to Select</p>
    <p class="text-xs muted-text mt-1">Supports: mp3, m4a, m4b, aac</p>
  </div>

  <div class="file-list-content" role="list" aria-label="Audio files">
    <!-- File items rendered here -->
  </div>
</div>

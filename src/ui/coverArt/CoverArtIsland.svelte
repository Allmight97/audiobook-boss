<div class="col-span-1 flex flex-col items-center">
  <span class="text-xs font-medium mb-1">Cover Art</span>
  <div id="cover-art-area" class="cover-art-area mb-1" data-testid="cover-art-area">
    <div class="placeholder-text">Click or Drag Image</div>
    <img src="" alt="Book Cover Art" id="cover-art-img" class="hidden" />
    <div class="cover-art-loading" id="cover-art-loading">Loading...</div>
    <button class="cover-art-clear-btn" id="cover-art-clear-btn">✕</button>
  </div>
  <div class="cover-art-url-row">
    <input
      type="text"
      id="cover-art-url-input"
      data-testid="cover-art-url-input"
      class="cover-art-url-input"
      placeholder="Paste image URL (https://...)"
    />
    <button id="cover-art-url-load-btn" data-testid="cover-art-url-load-btn" class="cover-art-url-load-btn">
      Load
    </button>
  </div>
  <div id="cover-art-url-message" class="cover-art-url-message" role="status" aria-live="polite"></div>
</div>

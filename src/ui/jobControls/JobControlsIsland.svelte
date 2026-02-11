<div class="flex items-center gap-2">
  <div class="flex items-center gap-1.5">
    <div class="relative group">
      <div class="info-icon">i</div>
      <div class="info-popover group-hover:block">
        Combines all files in the list into a single audiobook in the order they appear. Each file will be treated as a chapter.
      </div>
    </div>
    <label class="checkbox-label text-xs mb-0" data-testid="merge-toggle">
      <input type="checkbox" id="merge-mode-toggle" />
      <span class="option-label">Merge files into one audiobook</span>
    </label>
  </div>

  <div class="flex items-center gap-1" title="Concurrent Jobs">
    <span class="text-xs muted-text whitespace-nowrap">Number of Jobs:</span>
    <select id="max-concurrent-select" class="text-xs w-14 px-1 py-0.5" style="height: 24px">
      <option value="auto" selected>Auto</option>
      <option value="1">1</option>
      <option value="2">2</option>
      <option value="3">3</option>
      <option value="4">4</option>
    </select>
    <span
      id="max-concurrent-effective"
      class="text-xs muted-text"
      aria-live="polite"
      data-testid="max-concurrent-effective"
    ></span>
  </div>
</div>

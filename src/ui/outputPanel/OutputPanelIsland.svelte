<script lang="ts">
	import {
		handleAbsCompatibleChange,
		handleAbsIncludeYearChange,
		handleDirectoryBrowse,
	} from './handlers';

	function handleBrowseClick(): void {
		void handleDirectoryBrowse();
	}
</script>

<div>
  <div class="section-header">
    <h3>Output Directory</h3>
  </div>
  <div class="mb-2">
    <div class="output-preview-box">
      <div
        id="output-preview-text"
        class="output-path-text"
        title="Full output path preview"
      >
        Select output directory...
      </div>
      <button
        id="output-dir-browse"
        class="btn-pill btn-pill-primary-soft"
        on:click={handleBrowseClick}
      >
        Browse…
      </button>
    </div>

    <input
      type="text"
      id="output-dir-text"
      class="hidden"
      readonly
    />

    <div class="output-options-panel">
      <div class="path-option-row flex items-center gap-2">
        <label class="checkbox-label text-xs mt-0">
          <input
            type="checkbox"
            id="output-abs-structure"
            checked
            on:change={handleAbsCompatibleChange}
          />
          ABS-compatible structure
          ([Author]/[Series]/[Book # - Title]/)
        </label>
        <div class="relative group">
          <div class="info-icon">i</div>
          <div class="info-popover group-hover:block">
            Creates folder structure Audiobookshelf auto-parses.
          </div>
        </div>
      </div>
      <div class="path-option-row" id="output-abs-options">
        <label class="checkbox-label text-xs mt-0">
          <input
            type="checkbox"
            id="output-abs-include-year"
            on:change={handleAbsIncludeYearChange}
          />
          Include year segment (YYYY)
        </label>
        <span id="output-abs-hint" class="text-xs muted-text">
          Creates Author / Series / Book # - Title
        </span>
      </div>
    </div>
  </div>
</div>

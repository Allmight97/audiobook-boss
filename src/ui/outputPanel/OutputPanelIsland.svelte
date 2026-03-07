<script lang="ts">
	import {
		handleAbsIncludeYearChange,
		handleDirectoryBrowse,
		handleNamingPresetChange,
		handleNamingTemplateInput,
	} from './handlers';
	import { outputPanelState } from './state.svelte';

	const customTemplatePlaceholder = '{author}/{series}/Book {seriesPart} - {title}';

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
        title={outputPanelState.previewTitle}
      >
        {outputPanelState.previewText}
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
      value={outputPanelState.outputDirectory}
    />

    <div class="output-options-panel">
      <div class="path-option-row flex items-center gap-2">
        <label for="output-naming-preset" class="input-label text-xs mt-0">Naming preset</label>
        <select
          id="output-naming-preset"
          class="input-text"
          value={outputPanelState.namingPreset}
          on:change={handleNamingPresetChange}
        >
          <option value="absDefault">ABS Default</option>
          <option value="customTemplate">Custom Template</option>
        </select>
        <div class="relative group">
          <div class="info-icon">i</div>
          <div class="info-popover group-hover:block">
            ABS Default keeps Audiobookshelf-compatible paths. Custom Template stores your draft template.
          </div>
        </div>
      </div>
      <div
        class="path-option-row"
        id="output-template-row"
        hidden={outputPanelState.templateRowHidden}
      >
        <label for="output-template-input" class="input-label text-xs mt-0">Template</label>
	        <input
	          id="output-template-input"
	          type="text"
	          class="input-text w-full"
	          value={outputPanelState.namingTemplate}
	          placeholder={customTemplatePlaceholder}
	          on:input={handleNamingTemplateInput}
	          autocomplete="off"
	          spellcheck="false"
	        />
      </div>
      <div class="path-option-row" id="output-abs-options">
        <label class="checkbox-label text-xs mt-0">
          <input
            type="checkbox"
            id="output-abs-include-year"
            checked={outputPanelState.absIncludeYear}
            on:change={handleAbsIncludeYearChange}
          />
          Include year segment (YYYY)
        </label>
        <span
          id="output-abs-hint"
          class="text-xs muted-text"
          hidden={outputPanelState.absHintHidden}
        >
          {outputPanelState.absHintText}
        </span>
      </div>
    </div>
  </div>
</div>

<script lang="ts">
	import { onMount } from 'svelte';
	import {
		handleAbsIncludeYearChange,
		handleDirectoryBrowse,
		handleNamingPresetChange,
		handleNamingTemplateInput,
	} from './handlers';
	import { initOutputPanel } from './index';
	import { outputPanelState } from './state.svelte';

	const customTemplatePlaceholder = '{author}/{series}/Book {seriesPart} - {title}';

	function handleBrowseClick(): void {
		void handleDirectoryBrowse();
	}

	onMount(() => {
		initOutputPanel();
	});
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

    <div class="output-options-panel">
      <div class="path-option-row flex items-center gap-2">
        <label for="output-naming-preset" class="text-xs mt-0">Naming preset</label>
        <select
          id="output-naming-preset"
          class="w-auto min-w-40"
          value={outputPanelState.namingPreset}
          on:change={handleNamingPresetChange}
        >
          <option value="absDefault">ABS Default</option>
          <option value="customTemplate">Custom Template</option>
        </select>
        <div class="relative group">
          <div class="info-icon">i</div>
          <div class="info-popover">
            ABS Default keeps Audiobookshelf-compatible paths. Custom Template stores your draft template.
          </div>
        </div>
      </div>
      <div
        class="path-option-row"
        id="output-template-row"
        hidden={outputPanelState.templateRowHidden}
      >
        <label for="output-template-input" class="text-xs mt-0">Template</label>
	        <input
	          id="output-template-input"
	          type="text"
	          class="w-full"
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

<style>
	.output-preview-box {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 0.5rem 0.5rem 0.75rem;
		border: 1px solid var(--border-primary);
		border-radius: 0.375rem;
		background-color: var(--bg-input);
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: var(--text-primary);
	}

	.output-path-text {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--text-muted);
	}

	.btn-pill-primary-soft {
		background-color: #6ba3f7;
		color: #ffffff;
	}

	.btn-pill-primary-soft:hover {
		background-color: #5b93e7;
	}

	.output-options-panel {
		margin-top: 0.25rem;
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--border-primary);
		border-radius: 0.375rem;
		background-color: var(--bg-input);
	}

	.path-option-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.25rem 0;
	}
</style>

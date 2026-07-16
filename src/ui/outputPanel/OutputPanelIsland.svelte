<script lang="ts">
	import { onMount } from 'svelte';
	import {
		browseOutputDirectory,
		editNamingTemplate,
		selectNamingPreset,
		setAbsIncludeYear,
	} from './actions';
	import { initOutputPanel } from './index';
	import { outputPanelState } from './state.svelte';

	let { variant = 'default' }: { variant?: 'default' | 'workbench' } = $props();

	const customTemplatePlaceholder = '{author}/{series}/Book {seriesPart} - {title}';

	function handleBrowseClick(): void {
		void browseOutputDirectory();
	}

	function handleNamingPresetChange(event: Event): void {
		const select = event.currentTarget as HTMLSelectElement | null;
		selectNamingPreset(select?.value ?? 'absDefault');
	}

	function handleNamingTemplateInput(event: Event): void {
		const input = event.currentTarget as HTMLInputElement | null;
		editNamingTemplate(input?.value ?? '');
	}

	function handleAbsIncludeYearChange(event: Event): void {
		const input = event.currentTarget as HTMLInputElement | null;
		setAbsIncludeYear(Boolean(input?.checked));
	}

	onMount(() => {
		initOutputPanel();
	});
</script>

<div
	class="output-panel"
	class:output-panel-workbench={variant === 'workbench'}
	data-testid="output-panel"
>
  <div class="section-header">
    <h3>{variant === 'workbench' ? 'Output' : 'Output Directory'}</h3>
  </div>
  <div class="output-panel-body">
    {#if variant === 'workbench'}
      <label for="output-dir-browse" class="output-subtitle">Output Directory</label>
    {/if}
    <div class="output-preview-box">
      <div
        id={variant === 'workbench' ? 'output-dir-text' : 'output-preview-text'}
        class="output-path-text"
        title={variant === 'workbench'
          ? outputPanelState.outputDirectory || outputPanelState.previewTitle
          : outputPanelState.previewTitle}
        data-testid={variant === 'workbench' ? 'output-directory-value' : 'output-preview-value'}
      >
        {variant === 'workbench'
          ? outputPanelState.outputDirectory || 'Select output directory...'
          : outputPanelState.previewText}
      </div>
      <button
        id="output-dir-browse"
        class="pill pill-primary pill-sm output-browse-button"
        onclick={handleBrowseClick}
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
          onchange={handleNamingPresetChange}
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
	          oninput={handleNamingTemplateInput}
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
            onchange={handleAbsIncludeYearChange}
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
      {#if variant === 'workbench'}
        <div class="output-example" data-testid="output-example">
          <span class="output-example-label">Example:</span>
          <span class="output-example-path" title={outputPanelState.previewTitle}>
            {outputPanelState.previewText}
          </span>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
	.output-panel {
		min-width: 0;
	}

	.output-panel-body {
		margin-bottom: 0.5rem;
	}

	.output-panel-workbench .section-header {
		margin-bottom: 0.5rem;
	}

	.output-subtitle {
		margin-top: 0;
		margin-bottom: 0.25rem;
	}

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

	.output-panel-workbench .output-preview-box {
		padding: 0.425rem 0.5rem 0.425rem 0.625rem;
	}

	.output-path-text {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--text-muted);
	}

	.output-panel-workbench .output-path-text {
		color: var(--text-primary);
	}

	.output-browse-button {
		padding: 0.375rem 0.75rem;
	}

	.output-options-panel {
		margin-top: 0.25rem;
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--border-primary);
		border-radius: 0.375rem;
		background-color: var(--bg-input);
	}

	.output-panel-workbench .output-options-panel {
		padding: 0.5rem 0.625rem;
	}

	.path-option-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.25rem 0;
	}

	.output-panel-workbench .path-option-row {
		flex-wrap: wrap;
	}

	.output-example {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		gap: 0.375rem;
		margin-top: 0.5rem;
		color: var(--text-muted);
		font-size: 0.75rem;
		line-height: 1.35;
	}

	.output-example-label {
		color: var(--text-secondary);
		font-weight: 500;
	}

	.output-example-path {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>

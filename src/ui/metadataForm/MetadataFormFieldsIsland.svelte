<script lang="ts">
	import { openMetadataLookup } from '../metadataLookup';
	import { metadataSaveInProgressStore } from '../metadataSaveState';

	type MetadataFieldHandler = (id: string) => void;
	type MetadataSaveHandler = () => void;

	export let onFieldInput: MetadataFieldHandler;
	export let onActionChange: MetadataFieldHandler;
	export let onSaveMetadata: MetadataSaveHandler;

	function handleMetadataFieldInput(event: Event): void {
		const target = event.target;
		if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
			if (!target.id.startsWith('meta-')) return;
			onFieldInput?.(target.id);
		}
	}

	function handleMetadataFieldChange(event: Event): void {
		const target = event.target;
		if (target instanceof HTMLSelectElement) {
			if (!target.classList.contains('meta-apply-select')) return;
			onActionChange?.(target.id);
			return;
		}
		if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
			if (!target.id.startsWith('meta-')) return;
			onFieldInput?.(target.id);
		}
	}
</script>

<div
	class="grid grid-cols-4 gap-x-3 gap-y-1.5"
	on:input={handleMetadataFieldInput}
	on:change={handleMetadataFieldChange}
>
  <div class="col-span-3">
    <div class="meta-field-header">
      <label for="meta-title">Book Title</label>
      <select id="meta-title-action" class="meta-apply-select" data-testid="meta-title-action">
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input type="text" id="meta-title" placeholder="Book title" />
  </div>
  <div class="col-span-1">
    <div class="meta-field-header">
      <label for="meta-year">Publication Date</label>
      <select id="meta-year-action" class="meta-apply-select" data-testid="meta-year-action">
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input type="text" id="meta-year" placeholder="YYYY or YYYY-MM" />
  </div>
  <div class="col-span-2">
    <div class="meta-field-header">
      <label for="meta-author">Author</label>
      <select id="meta-author-action" class="meta-apply-select" data-testid="meta-author-action">
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input type="text" id="meta-author" placeholder="Author" />
  </div>
  <div class="col-span-2">
    <div class="meta-field-header">
      <label for="meta-narrator">Narrator</label>
      <select
        id="meta-narrator-action"
        class="meta-apply-select"
        data-testid="meta-narrator-action"
      >
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input type="text" id="meta-narrator" placeholder="Narrator" />
  </div>
  <div class="col-span-2">
    <div class="meta-field-header">
      <label for="meta-series">Series</label>
      <select id="meta-series-action" class="meta-apply-select" data-testid="meta-series-action">
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input type="text" id="meta-series" placeholder="Series name" />
  </div>
  <div class="col-span-1">
    <div class="meta-field-header">
      <label for="meta-series-part">Book #</label>
      <select
        id="meta-series-part-action"
        class="meta-apply-select"
        data-testid="meta-series-part-action"
      >
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input type="text" id="meta-series-part" placeholder="#" />
    <div id="meta-series-part-warning" class="text-xs warning-text" hidden>
      Series detected - add Book # (series sequence) for ABS ordering.
    </div>
  </div>
  <div class="col-span-2">
    <div class="meta-field-header">
      <label for="meta-subseries">Sub-series</label>
      <select
        id="meta-subseries-action"
        class="meta-apply-select"
        data-testid="meta-subseries-action"
      >
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input type="text" id="meta-subseries" placeholder="Sub-series name" />
  </div>
  <div class="col-span-1">
    <div class="meta-field-header">
      <label for="meta-subseries-part">Sub-series #</label>
      <select
        id="meta-subseries-part-action"
        class="meta-apply-select"
        data-testid="meta-subseries-part-action"
      >
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input type="text" id="meta-subseries-part" placeholder="#" />
    <div id="meta-subseries-part-warning" class="text-xs warning-text" hidden>
      Sub-series detected - add sub-series # (series sequence) for ABS ordering.
    </div>
  </div>
  <div class="col-span-1">
    <div class="meta-field-header">
      <label for="meta-genre">Genre</label>
      <select id="meta-genre-action" class="meta-apply-select" data-testid="meta-genre-action">
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input type="text" id="meta-genre" placeholder="Genre" />
  </div>
  <div class="col-span-4">
    <div class="meta-field-header">
      <label for="meta-description">Description</label>
      <select
        id="meta-description-action"
        class="meta-apply-select"
        data-testid="meta-description-action"
      >
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <textarea id="meta-description" rows="2" placeholder="Description"></textarea>
  </div>
</div>
<div class="metadata-apply-row">
  <button
    id="metadata-lookup-btn"
    class="btn-pill btn-pill-secondary"
    data-testid="metadata-lookup-btn"
    type="button"
    on:click={openMetadataLookup}
  >
    Find Metadata
  </button>
  <button
    id="metadata-save-btn"
    class="btn-pill btn-pill-primary"
    data-testid="metadata-save-btn"
    disabled={$metadataSaveInProgressStore}
    on:click={onSaveMetadata}
  >
    Save All Changes
  </button>
</div>

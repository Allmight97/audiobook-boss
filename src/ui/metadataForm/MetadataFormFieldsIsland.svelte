<script lang="ts">
	import { openMetadataLookup } from '../metadataLookup';
	import { metadataSaveInProgressStore } from '../metadataSaveState';
	import { metadataFormState } from './state.svelte';

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
      <select
        id="meta-title-action"
        class="meta-apply-select"
        data-testid="meta-title-action"
        bind:value={metadataFormState.fields['meta-title'].action}
      >
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input
      type="text"
      id="meta-title"
      bind:value={metadataFormState.fields['meta-title'].value}
      placeholder={metadataFormState.fields['meta-title'].mixed
        ? 'Mixed values'
        : metadataFormState.fields['meta-title'].placeholder}
      data-dirty={metadataFormState.fields['meta-title'].dirty ? 'true' : undefined}
      class:dirty-field={metadataFormState.fields['meta-title'].dirty}
      data-mixed={metadataFormState.fields['meta-title'].mixed ? 'true' : undefined}
    />
  </div>
  <div class="col-span-1">
    <div class="meta-field-header">
      <label for="meta-year">Publication Date</label>
      <select
        id="meta-year-action"
        class="meta-apply-select"
        data-testid="meta-year-action"
        bind:value={metadataFormState.fields['meta-year'].action}
      >
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input
      type="text"
      id="meta-year"
      bind:value={metadataFormState.fields['meta-year'].value}
      placeholder={metadataFormState.fields['meta-year'].mixed
        ? 'Mixed values'
        : metadataFormState.fields['meta-year'].placeholder}
      data-dirty={metadataFormState.fields['meta-year'].dirty ? 'true' : undefined}
      class:dirty-field={metadataFormState.fields['meta-year'].dirty}
      data-mixed={metadataFormState.fields['meta-year'].mixed ? 'true' : undefined}
    />
  </div>
  <div class="col-span-2">
    <div class="meta-field-header">
      <label for="meta-author">Author</label>
      <select
        id="meta-author-action"
        class="meta-apply-select"
        data-testid="meta-author-action"
        bind:value={metadataFormState.fields['meta-author'].action}
      >
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input
      type="text"
      id="meta-author"
      bind:value={metadataFormState.fields['meta-author'].value}
      placeholder={metadataFormState.fields['meta-author'].mixed
        ? 'Mixed values'
        : metadataFormState.fields['meta-author'].placeholder}
      data-dirty={metadataFormState.fields['meta-author'].dirty ? 'true' : undefined}
      class:dirty-field={metadataFormState.fields['meta-author'].dirty}
      data-mixed={metadataFormState.fields['meta-author'].mixed ? 'true' : undefined}
    />
  </div>
  <div class="col-span-2">
    <div class="meta-field-header">
      <label for="meta-narrator">Narrator</label>
      <select
        id="meta-narrator-action"
        class="meta-apply-select"
        data-testid="meta-narrator-action"
        bind:value={metadataFormState.fields['meta-narrator'].action}
      >
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input
      type="text"
      id="meta-narrator"
      bind:value={metadataFormState.fields['meta-narrator'].value}
      placeholder={metadataFormState.fields['meta-narrator'].mixed
        ? 'Mixed values'
        : metadataFormState.fields['meta-narrator'].placeholder}
      data-dirty={metadataFormState.fields['meta-narrator'].dirty ? 'true' : undefined}
      class:dirty-field={metadataFormState.fields['meta-narrator'].dirty}
      data-mixed={metadataFormState.fields['meta-narrator'].mixed ? 'true' : undefined}
    />
  </div>
  <div class="col-span-2">
    <div class="meta-field-header">
      <label for="meta-series">Series</label>
      <select
        id="meta-series-action"
        class="meta-apply-select"
        data-testid="meta-series-action"
        bind:value={metadataFormState.fields['meta-series'].action}
      >
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input
      type="text"
      id="meta-series"
      bind:value={metadataFormState.fields['meta-series'].value}
      placeholder={metadataFormState.fields['meta-series'].mixed
        ? 'Mixed values'
        : metadataFormState.fields['meta-series'].placeholder}
      data-dirty={metadataFormState.fields['meta-series'].dirty ? 'true' : undefined}
      class:dirty-field={metadataFormState.fields['meta-series'].dirty}
      data-mixed={metadataFormState.fields['meta-series'].mixed ? 'true' : undefined}
    />
  </div>
  <div class="col-span-1">
    <div class="meta-field-header">
      <label for="meta-series-part">Book #</label>
      <select
        id="meta-series-part-action"
        class="meta-apply-select"
        data-testid="meta-series-part-action"
        bind:value={metadataFormState.fields['meta-series-part'].action}
      >
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input
      type="text"
      id="meta-series-part"
      bind:value={metadataFormState.fields['meta-series-part'].value}
      placeholder={metadataFormState.fields['meta-series-part'].mixed
        ? 'Mixed values'
        : metadataFormState.fields['meta-series-part'].placeholder}
      data-dirty={metadataFormState.fields['meta-series-part'].dirty ? 'true' : undefined}
      class:dirty-field={metadataFormState.fields['meta-series-part'].dirty}
      data-mixed={metadataFormState.fields['meta-series-part'].mixed ? 'true' : undefined}
    />
    <div
      id="meta-series-part-warning"
      class="text-xs warning-text"
      hidden={!metadataFormState.seriesPartWarning.visible}
    >
      {metadataFormState.seriesPartWarning.message}
    </div>
  </div>
  <div class="col-span-2">
    <div class="meta-field-header">
      <label for="meta-subseries">Sub-series</label>
      <select
        id="meta-subseries-action"
        class="meta-apply-select"
        data-testid="meta-subseries-action"
        bind:value={metadataFormState.fields['meta-subseries'].action}
      >
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input
      type="text"
      id="meta-subseries"
      bind:value={metadataFormState.fields['meta-subseries'].value}
      placeholder={metadataFormState.fields['meta-subseries'].mixed
        ? 'Mixed values'
        : metadataFormState.fields['meta-subseries'].placeholder}
      data-dirty={metadataFormState.fields['meta-subseries'].dirty ? 'true' : undefined}
      class:dirty-field={metadataFormState.fields['meta-subseries'].dirty}
      data-mixed={metadataFormState.fields['meta-subseries'].mixed ? 'true' : undefined}
    />
  </div>
  <div class="col-span-1">
    <div class="meta-field-header">
      <label for="meta-subseries-part">Sub-series #</label>
      <select
        id="meta-subseries-part-action"
        class="meta-apply-select"
        data-testid="meta-subseries-part-action"
        bind:value={metadataFormState.fields['meta-subseries-part'].action}
      >
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input
      type="text"
      id="meta-subseries-part"
      bind:value={metadataFormState.fields['meta-subseries-part'].value}
      placeholder={metadataFormState.fields['meta-subseries-part'].mixed
        ? 'Mixed values'
        : metadataFormState.fields['meta-subseries-part'].placeholder}
      data-dirty={metadataFormState.fields['meta-subseries-part'].dirty ? 'true' : undefined}
      class:dirty-field={metadataFormState.fields['meta-subseries-part'].dirty}
      data-mixed={metadataFormState.fields['meta-subseries-part'].mixed ? 'true' : undefined}
    />
    <div
      id="meta-subseries-part-warning"
      class="text-xs warning-text"
      hidden={!metadataFormState.subseriesPartWarning.visible}
    >
      {metadataFormState.subseriesPartWarning.message}
    </div>
  </div>
  <div class="col-span-1">
    <div class="meta-field-header">
      <label for="meta-genre">Genre</label>
      <select
        id="meta-genre-action"
        class="meta-apply-select"
        data-testid="meta-genre-action"
        bind:value={metadataFormState.fields['meta-genre'].action}
      >
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <input
      type="text"
      id="meta-genre"
      bind:value={metadataFormState.fields['meta-genre'].value}
      placeholder={metadataFormState.fields['meta-genre'].mixed
        ? 'Mixed values'
        : metadataFormState.fields['meta-genre'].placeholder}
      data-dirty={metadataFormState.fields['meta-genre'].dirty ? 'true' : undefined}
      class:dirty-field={metadataFormState.fields['meta-genre'].dirty}
      data-mixed={metadataFormState.fields['meta-genre'].mixed ? 'true' : undefined}
    />
  </div>
  <div class="col-span-4">
    <div class="meta-field-header">
      <label for="meta-description">Description</label>
      <select
        id="meta-description-action"
        class="meta-apply-select"
        data-testid="meta-description-action"
        bind:value={metadataFormState.fields['meta-description'].action}
      >
        <option value="keep">Keep</option>
        <option value="blank">Blank</option>
      </select>
    </div>
    <textarea
      id="meta-description"
      rows="2"
      bind:value={metadataFormState.fields['meta-description'].value}
      placeholder={metadataFormState.fields['meta-description'].mixed
        ? 'Mixed values'
        : metadataFormState.fields['meta-description'].placeholder}
      data-dirty={metadataFormState.fields['meta-description'].dirty ? 'true' : undefined}
      class:dirty-field={metadataFormState.fields['meta-description'].dirty}
      data-mixed={metadataFormState.fields['meta-description'].mixed ? 'true' : undefined}
    ></textarea>
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

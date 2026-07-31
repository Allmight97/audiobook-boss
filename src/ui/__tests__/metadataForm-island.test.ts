import { beforeEach, describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import MetadataFormFieldsIsland from '../metadataForm/MetadataFormFieldsIsland.svelte';
import { metadataFormState } from '../metadataForm/state.svelte';
import {
	initMetadataFormEvents,
	onMetadataFormActionSelectChange,
	onMetadataFormFieldInput,
	setMetadataFormMode,
} from '../metadataForm';

describe('MetadataForm island mount + multi-select action sync', () => {
	beforeEach(() => {
		document.body.innerHTML = `
      <div id="metadata-selection-count"></div>
      <div id="metadata-form" data-multi-select="false">
        <div class="grid grid-cols-4 gap-3 mb-3"></div>
      </div>
    `;
		render(MetadataFormFieldsIsland, {
			onFieldInput: onMetadataFormFieldInput,
			onActionChange: onMetadataFormActionSelectChange,
			onSaveMetadata: () => {},
		});
	});

	it('hides bulk action selects in single-select mode', async () => {
		initMetadataFormEvents();
		setMetadataFormMode('single');
		await tick();

		expect(document.getElementById('meta-title-action')).toBeNull();
		expect(document.getElementById('meta-author-action')).toBeNull();
	});

	it('mounts fields into root and syncs action select on multi-select input', async () => {
		initMetadataFormEvents();
		setMetadataFormMode('multi', 2);
		await tick();

		const titleInput = document.getElementById('meta-title') as HTMLInputElement | null;
		const titleAction = document.getElementById('meta-title-action') as HTMLSelectElement | null;

		expect(titleInput).toBeTruthy();
		expect(titleAction).toBeTruthy();

		if (!titleInput || !titleAction) return;

		titleInput.value = 'Dune';
		titleInput.dispatchEvent(new Event('input', { bubbles: true }));

		expect(metadataFormState.fields['meta-title'].dirty).toBe(true);
		expect(titleAction.value).toBe('keep');
	});

	it('updates selection count text for multi-select mode', () => {
		initMetadataFormEvents();
		setMetadataFormMode('multi', 3);

		expect(metadataFormState.mode).toBe('multi');
		expect(metadataFormState.selectionCount).toBe(3);
	});
});

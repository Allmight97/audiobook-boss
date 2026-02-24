import { beforeEach, describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import MetadataFormFieldsIsland from '../metadataForm/MetadataFormFieldsIsland.svelte';
import {
	initMetadataFormEvents,
	onMetadataFormActionSelectChange,
	onMetadataFormFieldInput,
	setMetadataFormMode,
	triggerMetadataFormSave,
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
			onSaveMetadata: triggerMetadataFormSave,
		});
	});

	it('mounts fields into root and syncs action select on multi-select input', () => {
		initMetadataFormEvents();
		setMetadataFormMode('multi', 2);

		const titleInput = document.getElementById('meta-title') as HTMLInputElement | null;
		const titleAction = document.getElementById('meta-title-action') as HTMLSelectElement | null;

		expect(titleInput).toBeTruthy();
		expect(titleAction).toBeTruthy();

		if (!titleInput || !titleAction) return;

		titleInput.value = 'Dune';
		titleInput.dispatchEvent(new Event('input', { bubbles: true }));

		expect(titleInput.dataset.dirty).toBe('true');
		expect(titleAction.value).toBe('keep');
	});

	it('updates selection count text for multi-select mode', () => {
		initMetadataFormEvents();
		setMetadataFormMode('multi', 3);

		const count = document.getElementById('metadata-selection-count');
		expect(count?.textContent).toBe('3 files selected');
	});
});

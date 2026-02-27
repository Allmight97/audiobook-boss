import { beforeEach, describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import OutputPanelIsland from '../outputPanel/OutputPanelIsland.svelte';
import { initOutputPanel } from '../outputPanel';

describe('OutputPanel island mount', () => {
	beforeEach(() => {
		document.body.innerHTML = `
      <input id="meta-title" value="" />
      <input id="meta-author" value="" />
      <input id="meta-narrator" value="" />
      <input id="meta-year" value="" />
      <input id="meta-genre" value="" />
      <textarea id="meta-description"></textarea>
      <input id="meta-series" value="" />
      <input id="meta-series-part" value="" />
      <input id="meta-subseries" value="" />
      <input id="meta-subseries-part" value="" />
      <div id="meta-series-part-warning" hidden></div>
      <div id="meta-subseries-part-warning" hidden></div>
    `;
	});

	it('mounts output directory controls and renders default preview text', () => {
		render(OutputPanelIsland);
		initOutputPanel();

		const preview = document.getElementById('output-preview-text');
		expect(preview).toBeTruthy();
		expect(preview?.textContent).toBe('Select output directory...');
		expect(document.getElementById('output-dir-browse')).toBeTruthy();

		const hiddenDirInput = document.getElementById('output-dir-text') as HTMLInputElement | null;
		expect(hiddenDirInput).toBeTruthy();
		expect(hiddenDirInput?.readOnly).toBe(true);
		expect(hiddenDirInput?.classList.contains('hidden')).toBe(true);
		expect(document.querySelector('label[for="output-dir-text"]')).toBeNull();

		expect(document.getElementById('output-naming-preset')).toBeTruthy();
		expect(document.getElementById('output-template-row')).toBeTruthy();
		expect(document.getElementById('output-abs-options')).toBeTruthy();
	});
});

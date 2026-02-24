import { beforeEach, describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import { calculateTSOA, initTagPreview, updateTagPreview } from '../tagPreview';
import {
	resetMetadataFormPreviewState,
	setMetadataFormPreviewValueByInputId,
} from '../metadataForm/previewState.svelte';
import TagPreviewIsland from '../tagPreview/TagPreviewIsland.svelte';

function setupDom(initialValues: Record<string, string> = {}): void {
	const value = (id: string): string => initialValues[id] ?? '';

	document.body.innerHTML = `
    <input id="meta-title" value="${value('meta-title')}" />
    <input id="meta-author" value="${value('meta-author')}" />
    <input id="meta-narrator" value="${value('meta-narrator')}" />
    <input id="meta-series" value="${value('meta-series')}" />
    <input id="meta-series-part" value="${value('meta-series-part')}" />
    <input id="meta-subseries" value="${value('meta-subseries')}" />
    <input id="meta-subseries-part" value="${value('meta-subseries-part')}" />
    <input id="meta-year" value="${value('meta-year')}" />
    <input id="meta-genre" value="${value('meta-genre')}" />
    <div id="tag-preview-root"></div>
  `;

	resetMetadataFormPreviewState();
	const previewInputIds = [
		'meta-title',
		'meta-author',
		'meta-narrator',
		'meta-series',
		'meta-series-part',
		'meta-subseries',
		'meta-subseries-part',
		'meta-year',
		'meta-genre',
	];
	for (const inputId of previewInputIds) {
		const element = document.getElementById(inputId) as HTMLInputElement | null;
		if (!element) continue;
		setMetadataFormPreviewValueByInputId(inputId, element.value);
	}

	render(TagPreviewIsland);
}

function getFieldValue(field: string): string {
	const element = document.querySelector(`[data-field="${field}"]`);
	if (!element) {
		throw new Error(`Missing tag preview field: ${field}`);
	}

	return element.textContent ?? '';
}

function input(id: string): HTMLInputElement {
	const element = document.getElementById(id) as HTMLInputElement | null;
	if (!element) {
		throw new Error(`Missing metadata input: ${id}`);
	}

	return element;
}

async function flushRender(): Promise<void> {
	await Promise.resolve();
	await Promise.resolve();
}

describe('tagPreview', () => {
	beforeEach(() => {
		setupDom();
	});

	it('calculates TSOA with zero-padded part numbers', () => {
		expect(calculateTSOA('The Stormlight Archive', '3', 'Oathbringer')).toBe(
			'The Stormlight Archive 03 - Oathbringer',
		);
		expect(calculateTSOA('Series', '12', 'Finale')).toBe('Series 12 - Finale');
	});

	it('returns empty TSOA when series or title are missing', () => {
		expect(calculateTSOA('', '1', 'Book')).toBe('');
		expect(calculateTSOA('Series', '1', '')).toBe('');
		expect(calculateTSOA('Series', '0', 'Book')).toBe('Series 00 - Book');
	});

	it('renders preview values from metadata fields with derived mappings', async () => {
		setupDom({
			'meta-title': 'Mistborn',
			'meta-author': 'Brandon Sanderson',
			'meta-narrator': 'Michael Kramer',
			'meta-series': 'The Mistborn Saga',
			'meta-series-part': '1',
			'meta-subseries': 'Era 1',
			'meta-subseries-part': '2',
			'meta-year': '2006',
			'meta-genre': 'Fantasy',
		});

		initTagPreview();
		await flushRender();

		expect(getFieldValue('title')).toBe('Mistborn');
		expect(getFieldValue('album')).toBe('Mistborn');
		expect(getFieldValue('artist')).toBe('Brandon Sanderson');
		expect(getFieldValue('albumArtist')).toBe('Brandon Sanderson');
		expect(getFieldValue('composer')).toBe('Michael Kramer');
		expect(getFieldValue('series')).toBe('The Mistborn Saga');
		expect(getFieldValue('part')).toBe('1');
		expect(getFieldValue('subseries')).toBe('Era 1');
		expect(getFieldValue('subpart')).toBe('2');
		expect(getFieldValue('year')).toBe('2006');
		expect(getFieldValue('genre')).toBe('Fantasy');
		expect(getFieldValue('tsoa')).toBe('The Mistborn Saga 01 - Mistborn');
	});

	it('updates rendered values after metadata preview state changes', async () => {
		initTagPreview();
		await flushRender();

		expect(getFieldValue('title')).toBe('—');
		expect(getFieldValue('tsoa')).toBe('—');

		input('meta-title').value = '  Way of Kings  ';
		input('meta-series').value = '  Stormlight  ';
		input('meta-series-part').value = '4';
		setMetadataFormPreviewValueByInputId('meta-title', input('meta-title').value);
		setMetadataFormPreviewValueByInputId('meta-series', input('meta-series').value);
		setMetadataFormPreviewValueByInputId('meta-series-part', input('meta-series-part').value);
		updateTagPreview();
		await flushRender();

		expect(getFieldValue('title')).toBe('Way of Kings');
		expect(getFieldValue('album')).toBe('Way of Kings');
		expect(getFieldValue('series')).toBe('Stormlight');
		expect(getFieldValue('tsoa')).toBe('Stormlight 04 - Way of Kings');

		input('meta-series').value = '';
		setMetadataFormPreviewValueByInputId('meta-series', input('meta-series').value);
		updateTagPreview();
		await flushRender();

		expect(getFieldValue('series')).toBe('—');
		expect(getFieldValue('tsoa')).toBe('—');
	});

	it('supports direct updateTagPreview calls for existing callsites', async () => {
		initTagPreview();
		await flushRender();

		input('meta-author').value = 'Robin Hobb';
		setMetadataFormPreviewValueByInputId('meta-author', input('meta-author').value);
		updateTagPreview();
		await flushRender();

		expect(getFieldValue('artist')).toBe('Robin Hobb');
		expect(getFieldValue('albumArtist')).toBe('Robin Hobb');
	});

	it('remounts the island cleanly when preview root is cleared', async () => {
		initTagPreview();
		await flushRender();

		expect(document.querySelectorAll('[data-field="title"]').length).toBe(1);

		setupDom();
		initTagPreview();
		await flushRender();

		expect(document.querySelectorAll('[data-field="title"]').length).toBe(1);

		input('meta-title').value = 'Dune';
		setMetadataFormPreviewValueByInputId('meta-title', input('meta-title').value);
		updateTagPreview();
		await flushRender();

		expect(getFieldValue('title')).toBe('Dune');
	});
});

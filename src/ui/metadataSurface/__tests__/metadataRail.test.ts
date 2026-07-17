import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { AudioFile, FileListInfo } from '../../../types/audio';
import { setMetadataSurfacePresentation } from '../../fileList';
import { coordinateMetadataSurfaceSelectionTransition } from '../../fileList/metadataPanel';
import {
	setCurrentFileList,
	setSelectedFileIndices,
	setSelectedIndex,
} from '../../fileList/state.svelte';
import { populateMetadataFormMulti, populateMetadataFormSingle } from '../../metadataForm';
import { clearMetadataSession, cacheMetadataForFile } from '../../metadataSession';
import MetadataRailIsland from '../MetadataRailIsland.svelte';
import MetadataSurfaceIsland from '../MetadataSurfaceIsland.svelte';
import { applyEditSurfacePreference } from '../editSurface.svelte';

function file(path: string, duration = 65): AudioFile {
	return { path, duration, size: 1024, format: 'm4b', isValid: true };
}

function fileList(...files: AudioFile[]): FileListInfo {
	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: files.reduce((total, item) => total + (item.duration ?? 0), 0),
		totalSize: files.reduce((total, item) => total + (item.size ?? 0), 0),
		validCount: files.length,
		invalidCount: 0,
	};
}

describe('MetadataRailIsland', () => {
	beforeEach(() => {
		applyEditSurfacePreference('rail');
		clearMetadataSession();
		setCurrentFileList(null);
		setSelectedFileIndices([]);
		setSelectedIndex(-1);
		populateMetadataFormSingle({});
	});

	afterEach(() => {
		setMetadataSurfacePresentation(null);
	});

	it('renders a quiet empty placeholder without a selection', () => {
		render(MetadataRailIsland);

		expect(screen.getByTestId('metadata-rail')).toHaveTextContent(
			'Select a book to edit its details.',
		);
	});

	it('renders the active single selection head and switches tabs', async () => {
		const active = file('/books/way-of-kings.m4b', 66_161);
		setCurrentFileList(fileList(active));
		setSelectedFileIndices([0]);
		setSelectedIndex(0);
		cacheMetadataForFile(active.path, { title: 'The Way of Kings', artist: 'Brandon Sanderson' });
		populateMetadataFormSingle({ title: 'The Way of Kings', artist: 'Brandon Sanderson' });
		render(MetadataRailIsland);

		expect(screen.getByRole('heading', { name: 'The Way of Kings' })).toBeInTheDocument();
		expect(screen.getByText('Brandon Sanderson · 18:22:41 · 0 chapters')).toBeInTheDocument();

		await fireEvent.click(screen.getByRole('tab', { name: 'Facts' }));
		expect(screen.getByRole('tabpanel')).toHaveAttribute('id', 'metadata-rail-panel-facts');
	});

	it('shows the multi-select pane header', () => {
		const files = [file('/books/one.m4b'), file('/books/two.m4b'), file('/books/three.m4b')];
		setCurrentFileList(fileList(...files));
		setSelectedFileIndices([0, 1, 2]);
		setSelectedIndex(0);
		populateMetadataFormMulti(
			[{ artist: 'Author' }, { artist: 'Author' }, { artist: 'Author' }],
			3,
		);
		render(MetadataRailIsland);

		expect(screen.getByText('3 files selected')).toBeInTheDocument();
		expect(screen.getByTestId('meta-author-action')).toBeInTheDocument();
		expect(screen.queryByText(/— · 1:05 · 0 chapters/)).toBeNull();
	});

	it.each([
		{
			name: 'single',
			prepare: () => {
				const active = file('/books/words-of-radiance.m4b');
				setCurrentFileList(fileList(active));
				setSelectedFileIndices([0]);
				setSelectedIndex(0);
				cacheMetadataForFile(active.path, { title: 'Words of Radiance' });
				populateMetadataFormSingle({ title: 'Words of Radiance' });
			},
		},
		{
			name: 'multi',
			prepare: () => {
				const files = [file('/books/one.m4b'), file('/books/two.m4b')];
				setCurrentFileList(fileList(...files));
				setSelectedFileIndices([0, 1]);
				setSelectedIndex(0);
				populateMetadataFormMulti([{ title: 'One' }, { title: 'Two' }], 2);
			},
		},
	])('renders the same $name summary in rail and popover presentations', async ({ prepare }) => {
		prepare();
		const rail = render(MetadataRailIsland);
		const railHeading = screen.getByRole('heading').textContent;
		rail.unmount();

		applyEditSurfacePreference('popover');
		const onPresentationReady = vi.fn();
		render(MetadataSurfaceIsland, { onPresentationReady });
		const presentation = onPresentationReady.mock.calls[
			onPresentationReady.mock.calls.length - 1
		]?.[0] as { open(anchor: HTMLElement): void };
		const anchor = document.createElement('button');
		document.body.append(anchor);
		presentation.open(anchor);
		await tick();

		expect(
			screen.getByRole('dialog', { name: 'Metadata editor' }).querySelector('h2'),
		).toHaveTextContent(railHeading ?? '');
	});
});

describe('metadata rail presentation', () => {
	beforeEach(() => {
		applyEditSurfacePreference('rail');
		clearMetadataSession();
		const active = file('/books/active.m4b');
		setCurrentFileList(fileList(active));
		setSelectedFileIndices([]);
		setSelectedIndex(-1);
		cacheMetadataForFile(active.path, { title: 'Active' });
		populateMetadataFormSingle({});
	});

	afterEach(() => {
		setMetadataSurfacePresentation(null);
	});

	it('keeps row focus through an open-after-populate selection transition', async () => {
		const onPresentationReady = vi.fn();
		render(MetadataSurfaceIsland, { onPresentationReady });
		const railPresentation =
			onPresentationReady.mock.calls[onPresentationReady.mock.calls.length - 1]?.[0];
		setMetadataSurfacePresentation(railPresentation);
		const row = document.createElement('button');
		document.body.append(row);
		row.focus();

		await coordinateMetadataSurfaceSelectionTransition(
			{ type: 'selectOnly', index: 0 },
			() => {
				setSelectedFileIndices([0]);
				setSelectedIndex(0);
				return { changed: true };
			},
			{ anchor: row, openAfterPopulate: true },
		);

		expect(document.activeElement).toBe(row);
	});

	it('re-registers the popover presentation when the preference changes', async () => {
		const onPresentationReady = vi.fn();
		render(MetadataSurfaceIsland, { onPresentationReady });
		const railPresentation = onPresentationReady.mock.calls[
			onPresentationReady.mock.calls.length - 1
		]?.[0] as { isOpen(): boolean };
		expect(railPresentation.isOpen()).toBe(false);

		applyEditSurfacePreference('popover');
		await tick();
		const popoverPresentation = onPresentationReady.mock.calls[
			onPresentationReady.mock.calls.length - 1
		]?.[0] as {
			open(anchor: HTMLElement): void;
		};
		const row = document.createElement('button');
		document.body.append(row);
		popoverPresentation.open(row);
		await tick();

		expect(screen.getByRole('dialog', { name: 'Metadata editor' })).toBeInTheDocument();
	});
});

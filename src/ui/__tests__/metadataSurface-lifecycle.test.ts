import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import MetadataSurfaceIsland from '../metadataSurface/MetadataSurfaceIsland.svelte';
import metadataSurfaceSource from '../metadataSurface/MetadataSurfaceIsland.svelte?raw';
import { applyEditSurfacePreference } from '../metadataSurface/editSurface.svelte';

vi.mock('../coverArt', () => ({
	coverArtBytesToDataUrl: () => 'data:image/jpeg;base64,cover',
	onClearCoverArt: vi.fn(),
	onLoadCoverArtFromFilePicker: vi.fn(),
	onLoadCoverArtFromInput: vi.fn(),
}));
vi.mock('../metadataSession', () => ({
	getMetadataForFile: () => ({
		title: 'The Way of Kings',
		cover_art: new Uint8Array([1]),
	}),
	metadataSaveInProgress: { subscribe: vi.fn(() => () => {}) },
	saveMetadataFromUI: vi.fn(),
}));
vi.mock('../metadataLookup', () => ({ openMetadataLookup: vi.fn() }));
vi.mock('../fileList', () => ({
	getCurrentFileList: () => ({
		files: [{ path: '/books/the-way-of-kings.m4b', duration: 65_000 }],
	}),
	getSelectedFileIndex: () => 0,
	getSelectedFiles: () => [{ path: '/books/the-way-of-kings.m4b' }],
	readInspectorFacts: () => [
		{ label: 'Codec', value: 'AAC-LC', title: 'AAC-LC' },
		{ label: 'Position', value: 'Unsupported container', title: 'Unsupported container' },
	],
	readActiveFileChapters: () => [{ title: 'Opening', startMs: 0, endMs: 65_000 }],
}));
vi.mock('../outputPanel', () => ({
	readOutputDisplaySnapshot: () => ({
		previewText: 'Example.m4b',
		previewTitle: '/tmp/Example.m4b',
	}),
}));

describe('MetadataSurfaceIsland lifecycle', () => {
	beforeEach(() => {
		applyEditSurfacePreference('popover');
	});

	it('opens on its row anchor, delegates Escape and X dismissal, and restores row focus', async () => {
		const onDismiss = vi.fn(async () => true);
		const onPresentationReady = vi.fn();
		render(MetadataSurfaceIsland, { onDismiss, onPresentationReady });

		const runtime = onPresentationReady.mock.calls[0]?.[0] as {
			open(anchor: HTMLElement): void;
			closeWithoutStaging(): void;
		};
		const rowControl = document.createElement('button');
		rowControl.textContent = 'Edit metadata for Alpha';
		document.body.append(rowControl);
		runtime.open(rowControl);
		await tick();

		const dialog = screen.getByRole('dialog', { name: 'Metadata editor' });
		expect(dialog).toBeInTheDocument();
		await fireEvent.keyDown(dialog, { key: 'Escape' });
		expect(onDismiss).toHaveBeenCalledTimes(1);

		runtime.closeWithoutStaging();
		await tick();
		expect(screen.queryByRole('dialog', { name: 'Metadata editor' })).toBeNull();
		expect(document.activeElement).toBe(rowControl);

		runtime.open(rowControl);
		await tick();
		await fireEvent.click(screen.getByRole('button', { name: 'Close metadata editor' }));
		expect(onDismiss).toHaveBeenCalledTimes(2);
	});

	it('uses the 330px pop-head with the active title, cover, close pill, and stacked panes', async () => {
		const onPresentationReady = vi.fn();
		render(MetadataSurfaceIsland, { onPresentationReady });
		const runtime = onPresentationReady.mock.calls[0]?.[0] as {
			open(anchor: HTMLElement): void;
		};
		const rowControl = document.createElement('button');
		document.body.append(rowControl);
		runtime.open(rowControl);
		await tick();

		const dialog = screen.getByTestId('metadata-surface');
		expect(screen.getByRole('heading', { name: 'The Way of Kings' })).toBeInTheDocument();
		expect(dialog.querySelector('img')).toHaveAttribute('src', 'data:image/jpeg;base64,cover');
		expect(screen.getByRole('button', { name: 'Close metadata editor' })).toHaveClass(
			'pill',
			'pill-ghost',
			'pill-xs',
		);

		expect(metadataSurfaceSource).toContain('width: 330px');
		expect(metadataSurfaceSource).toContain('<MetadataSurfacePanes idPrefix="metadata-surface" layout="stacked"');
	});

	it('renders inspector facts, embedded chapters, and the public output preview in tabs', async () => {
		const onPresentationReady = vi.fn();
		render(MetadataSurfaceIsland, { onPresentationReady });
		const runtime = onPresentationReady.mock.calls[0]?.[0] as {
			open(anchor: HTMLElement): void;
			closeWithoutStaging(options?: { restoreFocus?: boolean }): void;
		};
		const rowControl = document.createElement('button');
		document.body.append(rowControl);
		runtime.open(rowControl);
		await tick();

		await fireEvent.click(screen.getByRole('tab', { name: 'Facts' }));
		expect(screen.getByText('AAC-LC')).toBeInTheDocument();
		expect(screen.getByText('Unsupported container')).toBeInTheDocument();
		await fireEvent.click(screen.getByRole('tab', { name: 'Chapters' }));
		expect(screen.getByText('Opening')).toBeInTheDocument();
		expect(screen.getByText('0:00 – 1:05')).toBeInTheDocument();
		await fireEvent.click(screen.getByRole('tab', { name: 'Output' }));
		expect(screen.getByText('Example.m4b')).toBeInTheDocument();

		runtime.closeWithoutStaging({ restoreFocus: false });
		runtime.open(rowControl);
		await tick();
		expect(screen.getByRole('tab', { name: 'Metadata' })).toHaveAttribute(
			'aria-selected',
			'true',
		);
	});
});

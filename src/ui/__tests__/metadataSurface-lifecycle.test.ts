import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';

import MetadataSurfaceIsland from '../metadataSurface/MetadataSurfaceIsland.svelte';

vi.mock('../coverArt', () => ({
	onClearCoverArt: vi.fn(),
	onLoadCoverArtFromFilePicker: vi.fn(),
	onLoadCoverArtFromInput: vi.fn(),
}));
vi.mock('../metadataSession', () => ({
	metadataSaveInProgress: { subscribe: vi.fn(() => () => {}) },
	saveMetadataFromUI: vi.fn(),
}));
vi.mock('../metadataLookup', () => ({ openMetadataLookup: vi.fn() }));
vi.mock('../fileList', () => ({
	readInspectorFacts: () => [{ label: 'Codec', value: 'AAC-LC', title: 'AAC-LC' }],
	readActiveFileChapters: () => [{ title: 'Opening', startMs: 0, endMs: 65_000 }],
}));
vi.mock('../outputPanel', () => ({
	readOutputDisplaySnapshot: () => ({ previewText: 'Example.m4b', previewTitle: '/tmp/Example.m4b' }),
}));

describe('MetadataSurfaceIsland lifecycle', () => {
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

	it('renders inspector facts, embedded chapters, and the public output preview in tabs', async () => {
		const onPresentationReady = vi.fn();
		render(MetadataSurfaceIsland, { onPresentationReady });
		const runtime = onPresentationReady.mock.calls[0]?.[0] as { open(anchor: HTMLElement): void };
		const rowControl = document.createElement('button');
		document.body.append(rowControl);
		runtime.open(rowControl);
		await tick();

		await fireEvent.click(screen.getByRole('tab', { name: 'Facts' }));
		expect(screen.getByText('AAC-LC')).toBeInTheDocument();
		await fireEvent.click(screen.getByRole('tab', { name: 'Chapters' }));
		expect(screen.getByText('Opening')).toBeInTheDocument();
		expect(screen.getByText('0:00 – 1:05')).toBeInTheDocument();
		await fireEvent.click(screen.getByRole('tab', { name: 'Output' }));
		expect(screen.getByText('Example.m4b')).toBeInTheDocument();
	});
});

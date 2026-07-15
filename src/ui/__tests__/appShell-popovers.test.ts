import { fireEvent, render, screen } from '@testing-library/svelte';
import { createRawSnippet, tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import AppShellIsland from '../appShell/AppShellIsland.svelte';
import { handleClickToSelectFolder } from '../fileImport';

const selectionHolder = vi.hoisted(() => ({ indices: new Set<number>() }));

vi.mock('../appSettings', () => ({ openAppSettingsDialog: vi.fn() }));
vi.mock('../fileImport', () => ({
	handleClickToSelect: vi.fn(),
	handleClickToSelectFolder: vi.fn(),
}));
vi.mock('../fileList', () => ({
	getSelectedFileIndices: () => selectionHolder.indices,
	openMetadataSurfaceForCurrentSelection: vi.fn(),
	readFileListCount: () => 5,
	removeSelectedFiles: vi.fn(),
}));
vi.mock('../jobControls', () => ({
	handleMergeModeChange: vi.fn(),
	JobControlsIsland: vi.fn(),
}));
vi.mock('../remoteSource', () => ({ openRemoteSourceAcquire: vi.fn() }));
vi.mock('../statusPanel', () => ({ triggerProcessFromStatusPanel: vi.fn() }));
vi.mock('../metadataLookup', () => ({ openMetadataLookup: vi.fn() }));
vi.mock('../operationsBar', () => ({ OperationsBarIsland: vi.fn() }));
vi.mock('../encoderPanel', () => ({ readEncoderSummaryLabel: () => 'FDK HE-AAC · VBR 3' }));
vi.mock('../encoderPanel/EncoderWorkbenchIsland.svelte', () => ({ default: vi.fn() }));
vi.mock('../outputPanel', () => ({
	OutputPanelIsland: vi.fn(),
	readOutputNamingSummaryLabel: () => 'ABS Default',
}));
vi.mock('../tagPreview', () => ({ TagPreviewIsland: vi.fn() }));

const emptyChildren = createRawSnippet(() => ({ render: () => '<span></span>' }));

describe('import split-button menu', () => {
	it('opens from the caret, runs Add Folder, and closes after selection', async () => {
		selectionHolder.indices = new Set<number>();
		render(AppShellIsland, { children: emptyChildren });
		const caret = document.getElementById('import-menu-toggle') as HTMLButtonElement;
		const addFolder = document.getElementById('add-folder-btn') as HTMLButtonElement;

		expect(caret.getAttribute('aria-expanded')).toBe('false');
		await fireEvent.click(caret);
		expect(caret.getAttribute('aria-expanded')).toBe('true');

		await fireEvent.click(addFolder);
		expect(vi.mocked(handleClickToSelectFolder)).toHaveBeenCalledTimes(1);
		expect(caret.getAttribute('aria-expanded')).toBe('false');
	});

	it('closes on Escape and restores focus to the caret', async () => {
		selectionHolder.indices = new Set<number>();
		render(AppShellIsland, { children: emptyChildren });
		const caret = document.getElementById('import-menu-toggle') as HTMLButtonElement;

		await fireEvent.click(caret);
		expect(caret.getAttribute('aria-expanded')).toBe('true');

		const menu = document.getElementById('add-folder-btn') as HTMLButtonElement;
		await fireEvent.keyDown(menu, { key: 'Escape' });
		await tick();
		expect(caret.getAttribute('aria-expanded')).toBe('false');
		expect(document.activeElement).toBe(caret);
	});

	it('closes on click-away', async () => {
		selectionHolder.indices = new Set<number>();
		render(AppShellIsland, { children: emptyChildren });
		const caret = document.getElementById('import-menu-toggle') as HTMLButtonElement;
		const clickAwayTarget = document.createElement('button');
		document.body.append(clickAwayTarget);

		await fireEvent.click(caret);
		expect(caret.getAttribute('aria-expanded')).toBe('true');
		await fireEvent.click(clickAwayTarget);
		await tick();
		expect(caret.getAttribute('aria-expanded')).toBe('false');
	});
});

describe('contextual selection cluster', () => {
	it('hides selection actions and shows the merge chip zone with no selection', () => {
		selectionHolder.indices = new Set<number>();
		render(AppShellIsland, { children: emptyChildren });

		expect(screen.queryByLabelText('Selected files actions')).toBeNull();
		const mergeZone = document.querySelector('.app-shell-merge') as HTMLElement;
		expect(mergeZone.hidden).toBe(false);
	});

	it('shows selection actions and hides the merge chip zone when files are selected', () => {
		selectionHolder.indices = new Set<number>([0, 1, 2]);
		render(AppShellIsland, { children: emptyChildren });

		const cluster = screen.getByLabelText('Selected files actions');
		expect(cluster.textContent).toContain('3 selected');
		expect(cluster.textContent).toContain('Find metadata (3)');
		expect(cluster.textContent).toContain('Edit shared fields (3)');
		expect(cluster.textContent).toContain('Remove');
		const mergeZone = document.querySelector('.app-shell-merge') as HTMLElement;
		expect(mergeZone.hidden).toBe(true);
	});
});

describe.each([
	['encoder', 'Encoder settings'],
	['naming', 'Output naming and tag preview'],
] as const)('%s toolbar popover', (kind, dialogName) => {
	it('opens, closes on Escape, and restores focus to its pill', async () => {
		render(AppShellIsland, { children: emptyChildren });
		const trigger = screen.getByTestId(`${kind}-popover-trigger`);

		await fireEvent.click(trigger);
		const dialog = screen.getByRole('dialog', { name: dialogName });
		expect(trigger.getAttribute('aria-expanded')).toBe('true');

		await fireEvent.keyDown(dialog, { key: 'Escape' });
		await tick();
		expect(screen.queryByRole('dialog', { name: dialogName })).toBeNull();
		expect(document.activeElement).toBe(trigger);
	});

	it('closes on click-away without stealing focus from the clicked target', async () => {
		render(AppShellIsland, { children: emptyChildren });
		const trigger = screen.getByTestId(`${kind}-popover-trigger`);
		const clickAwayTarget = document.createElement('button');
		document.body.append(clickAwayTarget);

		await fireEvent.click(trigger);
		expect(screen.getByRole('dialog', { name: dialogName })).toBeTruthy();
		clickAwayTarget.focus();
		await fireEvent.click(clickAwayTarget);
		await tick();

		expect(screen.queryByRole('dialog', { name: dialogName })).toBeNull();
		expect(document.activeElement).toBe(clickAwayTarget);
	});
});

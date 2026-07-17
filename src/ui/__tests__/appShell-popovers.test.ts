import { fireEvent, render, screen } from '@testing-library/svelte';
import { createRawSnippet, tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import AppShellIsland from '../appShell/AppShellIsland.svelte';
import { handleClickToSelect, handleClickToSelectFolder } from '../fileImport';
import { triggerProcessFromStatusPanel } from '../statusPanel';

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

describe('import menu', () => {
	it('opens from the Import pill with Files/Folder options and runs Folder', async () => {
		selectionHolder.indices = new Set<number>();
		render(AppShellIsland, { children: emptyChildren });
		const pill = document.getElementById('import-files-btn') as HTMLButtonElement;

		expect(pill.getAttribute('aria-expanded')).toBe('false');
		await fireEvent.click(pill);
		expect(pill.getAttribute('aria-expanded')).toBe('true');

		const filesOption = document.getElementById('import-files-option') as HTMLButtonElement;
		const folderOption = document.getElementById('add-folder-btn') as HTMLButtonElement;
		expect(filesOption.textContent).toContain('Files…');
		expect(folderOption.textContent).toContain('Folder…');

		await fireEvent.click(folderOption);
		expect(vi.mocked(handleClickToSelectFolder)).toHaveBeenCalledTimes(1);
		expect(pill.getAttribute('aria-expanded')).toBe('false');
	});

	it('runs the Files option and closes', async () => {
		selectionHolder.indices = new Set<number>();
		render(AppShellIsland, { children: emptyChildren });
		const pill = document.getElementById('import-files-btn') as HTMLButtonElement;

		await fireEvent.click(pill);
		await fireEvent.click(document.getElementById('import-files-option') as HTMLButtonElement);

		expect(vi.mocked(handleClickToSelect)).toHaveBeenCalledTimes(1);
		expect(pill.getAttribute('aria-expanded')).toBe('false');
	});

	it('closes on Escape and restores focus to the pill', async () => {
		selectionHolder.indices = new Set<number>();
		render(AppShellIsland, { children: emptyChildren });
		const pill = document.getElementById('import-files-btn') as HTMLButtonElement;

		await fireEvent.click(pill);
		expect(pill.getAttribute('aria-expanded')).toBe('true');

		const menu = document.getElementById('add-folder-btn') as HTMLButtonElement;
		await fireEvent.keyDown(menu, { key: 'Escape' });
		await tick();
		expect(pill.getAttribute('aria-expanded')).toBe('false');
		expect(document.activeElement).toBe(pill);
	});

	it('closes on click-away', async () => {
		selectionHolder.indices = new Set<number>();
		render(AppShellIsland, { children: emptyChildren });
		const pill = document.getElementById('import-files-btn') as HTMLButtonElement;
		const clickAwayTarget = document.createElement('button');
		document.body.append(clickAwayTarget);

		await fireEvent.click(pill);
		expect(pill.getAttribute('aria-expanded')).toBe('true');
		await fireEvent.click(clickAwayTarget);
		await tick();
		expect(pill.getAttribute('aria-expanded')).toBe('false');
	});

	it('preserves pointer focus but moves keyboard-open focus into the menu', async () => {
		selectionHolder.indices = new Set<number>();
		render(AppShellIsland, { children: emptyChildren });
		const pill = document.getElementById('import-files-btn') as HTMLButtonElement;

		pill.focus();
		await fireEvent.click(pill);
		await tick();
		expect(document.activeElement).toBe(pill);

		await fireEvent.click(pill);
		await fireEvent.keyDown(pill, { key: 'ArrowDown' });
		await tick();
		expect(document.activeElement).toBe(document.getElementById('import-files-option'));
	});
});

describe('process split-button', () => {
	it('runs a full process from the main button', async () => {
		selectionHolder.indices = new Set<number>();
		render(AppShellIsland, { children: emptyChildren });

		await fireEvent.click(document.getElementById('process-button') as HTMLButtonElement);

		expect(vi.mocked(triggerProcessFromStatusPanel)).toHaveBeenCalledTimes(1);
		expect(vi.mocked(triggerProcessFromStatusPanel)).toHaveBeenCalledWith();
	});

	it('offers preview durations from the caret and closes after select', async () => {
		selectionHolder.indices = new Set<number>();
		render(AppShellIsland, { children: emptyChildren });
		const caret = document.getElementById('process-menu-toggle') as HTMLButtonElement;

		await fireEvent.click(caret);
		expect(caret.getAttribute('aria-expanded')).toBe('true');

		const preview15 = screen.getByRole('menuitem', { name: 'Preview 15s' });
		await fireEvent.click(preview15);

		expect(vi.mocked(triggerProcessFromStatusPanel)).toHaveBeenCalledWith({ previewSeconds: 15 });
		expect(caret.getAttribute('aria-expanded')).toBe('false');
	});

	it('closes preview choices on Escape and restores focus to the caret', async () => {
		selectionHolder.indices = new Set<number>();
		render(AppShellIsland, { children: emptyChildren });
		const caret = document.getElementById('process-menu-toggle') as HTMLButtonElement;

		await fireEvent.click(caret);
		const preview15 = screen.getByRole('menuitem', { name: 'Preview 15s' });
		await fireEvent.keyDown(preview15, { key: 'Escape' });
		await tick();

		expect(caret.getAttribute('aria-expanded')).toBe('false');
		expect(document.activeElement).toBe(caret);
	});

	it('closes preview choices on click-away without stealing click target focus', async () => {
		selectionHolder.indices = new Set<number>();
		render(AppShellIsland, { children: emptyChildren });
		const caret = document.getElementById('process-menu-toggle') as HTMLButtonElement;
		const clickAwayTarget = document.createElement('button');
		document.body.append(clickAwayTarget);

		await fireEvent.click(caret);
		clickAwayTarget.focus();
		await fireEvent.click(clickAwayTarget);
		await tick();

		expect(caret.getAttribute('aria-expanded')).toBe('false');
		expect(document.activeElement).toBe(clickAwayTarget);
	});

	it('does not dismiss preview choices when the split main action is clicked', async () => {
		selectionHolder.indices = new Set<number>();
		render(AppShellIsland, { children: emptyChildren });
		const caret = document.getElementById('process-menu-toggle') as HTMLButtonElement;

		await fireEvent.click(caret);
		await fireEvent.click(document.getElementById('process-button') as HTMLButtonElement);

		expect(vi.mocked(triggerProcessFromStatusPanel)).toHaveBeenCalledWith();
		expect(caret.getAttribute('aria-expanded')).toBe('true');
	});

	it('preserves pointer focus but moves keyboard-open focus into preview choices', async () => {
		selectionHolder.indices = new Set<number>();
		render(AppShellIsland, { children: emptyChildren });
		const caret = document.getElementById('process-menu-toggle') as HTMLButtonElement;

		caret.focus();
		await fireEvent.click(caret);
		await tick();
		expect(document.activeElement).toBe(caret);

		await fireEvent.click(caret);
		await fireEvent.keyDown(caret, { key: 'ArrowDown' });
		await tick();
		expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Preview 15s' }));
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

	it('shows selection actions while keeping the merge chip zone visible when files are selected', () => {
		selectionHolder.indices = new Set<number>([0, 1, 2]);
		render(AppShellIsland, { children: emptyChildren });

		const cluster = screen.getByLabelText('Selected files actions');
		expect(cluster.textContent).toContain('3 selected');
		expect(cluster.textContent).toContain('Find metadata (3)');
		expect(cluster.textContent).toContain('Edit shared fields (3)');
		expect(cluster.textContent).toContain('Remove');
		const mergeZone = document.querySelector('.app-shell-merge') as HTMLElement;
		expect(mergeZone.hidden).toBe(false);
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

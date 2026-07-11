import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import AppShellIsland from '../appShell/AppShellIsland.svelte';

vi.mock('../appSettings', () => ({ openAppSettingsDialog: vi.fn() }));
vi.mock('../fileImport', () => ({
	handleClickToSelect: vi.fn(),
	handleClickToSelectFolder: vi.fn(),
}));
vi.mock('../fileList', () => ({
	getSelectedFileIndices: () => new Set<number>(),
	openMetadataSurfaceForCurrentSelection: vi.fn(),
	removeSelectedFiles: vi.fn(),
}));
vi.mock('../jobControls', () => ({
	handleMaxConcurrentSelectionChange: vi.fn(),
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

describe.each([
	['encoder', 'Encoder settings'],
	['naming', 'Output naming and tag preview'],
] as const)('%s toolbar popover', (kind, dialogName) => {
	it('opens, closes on Escape, and restores focus to its pill', async () => {
		render(AppShellIsland, { children: () => undefined });
		const trigger = screen.getByTestId(`${kind}-popover-trigger`);

		await fireEvent.click(trigger);
		const dialog = screen.getByRole('dialog', { name: dialogName });
		expect(trigger.getAttribute('aria-expanded')).toBe('true');

		await fireEvent.keyDown(dialog, { key: 'Escape' });
		await tick();
		expect(screen.queryByRole('dialog', { name: dialogName })).toBeNull();
		expect(document.activeElement).toBe(trigger);
	});

	it('closes on click-away and restores focus to its pill', async () => {
		render(AppShellIsland, { children: () => undefined });
		const trigger = screen.getByTestId(`${kind}-popover-trigger`);

		await fireEvent.click(trigger);
		expect(screen.getByRole('dialog', { name: dialogName })).toBeTruthy();
		await fireEvent.click(document.body);
		await tick();

		expect(screen.queryByRole('dialog', { name: dialogName })).toBeNull();
		expect(document.activeElement).toBe(trigger);
	});
});

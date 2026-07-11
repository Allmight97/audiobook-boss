import { describe, expect, it } from 'vitest';
import appSource from '../../App.svelte?raw';
import appShellSource from '../appShell/AppShellIsland.svelte?raw';
import fileImportSource from '../fileImport/FileImportIsland.svelte?raw';
import inputWorkflowSource from '../leftColumn/InputWorkflowPanel.svelte?raw';
import statusPanelSource from '../statusPanel/StatusPanelIsland.svelte?raw';

describe('app shell composition', () => {
	it('wraps the unchanged application grid in app chrome', () => {
		const shellIndex = appSource.indexOf('<AppShellIsland>');
		const mainIndex = appSource.indexOf('<div class="main-container">');

		expect(shellIndex).toBeGreaterThan(-1);
		expect(mainIndex).toBeGreaterThan(shellIndex);
		expect(appSource).toContain('<LeftColumnIsland {readWorkActivityByInputId} />');
		expect(appSource).toContain('<MetadataManagerIsland />');
		expect(appSource).toContain('<EncodingWorkbenchIsland />');
		expect(appSource).toContain('<StatusPanelIsland />');
		expect(appSource).toContain('<WorkCenterIsland />');
	});

	it('arranges appbar, unified toolbar, and main region in that order', () => {
		const appbarIndex = appShellSource.indexOf('data-testid="app-shell-appbar"');
		const toolbarIndex = appShellSource.indexOf('data-testid="app-shell-toolbar"');
		const mainIndex = appShellSource.indexOf('data-testid="app-shell-main"');

		expect(appbarIndex).toBeGreaterThan(-1);
		expect(toolbarIndex).toBeGreaterThan(appbarIndex);
		expect(mainIndex).toBeGreaterThan(toolbarIndex);
		expect(appShellSource).toContain('Audiobook Boss');
		expect(appShellSource).toContain('aria-label="Density"');
		expect(appShellSource).toContain('openAppSettingsDialog');
	});

	it('relocates each toolbar control through its owner strip', () => {
		for (const ownerSurface of [
			'handleClickToSelect',
			'handleClickToSelectFolder',
			'openRemoteSourceAcquire',
			'<JobControlsIsland',
			'triggerProcessFromStatusPanel',
		]) {
			expect(appShellSource).toContain(ownerSurface);
		}

		for (const ownerSurface of ['getSelectedFileIndices', 'removeSelectedFiles', 'openMetadataLookup']) {
			expect(appShellSource).toContain(ownerSurface);
		}

		expect(inputWorkflowSource).not.toContain('<JobControlsIsland');
		expect(fileImportSource).not.toContain('add-folder-btn');
		expect(fileImportSource).not.toContain('acquire-audiobooks-btn');
		expect(statusPanelSource).not.toContain('process-button');
	});
});

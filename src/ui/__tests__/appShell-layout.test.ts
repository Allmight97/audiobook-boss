import { describe, expect, it } from 'vitest';
import appSource from '../../App.svelte?raw';
import appShellSource from '../appShell/AppShellIsland.svelte?raw';
import fileImportSource from '../fileImport/FileImportIsland.svelte?raw';
import operationsBarSource from '../operationsBar/OperationsBarIsland.svelte?raw';

describe('app shell composition', () => {
	it('uses the full-width file area as final v3 geometry', () => {
		const shellIndex = appSource.indexOf('<AppShellIsland>');
		const fileAreaIndex = appSource.indexOf('data-testid="file-area"');

		expect(shellIndex).toBeGreaterThan(-1);
		expect(fileAreaIndex).toBeGreaterThan(shellIndex);
		expect(appSource).toContain('<FileImportIsland>');
		expect(appSource).toContain('<FileListIsland {...dropTarget} {readWorkActivityByInputId} />');
		expect(appSource).toContain('<MetadataSurfaceIsland');
		expect(appSource).not.toContain('LeftColumnIsland');
		expect(appSource).not.toContain('MetadataManagerIsland');
		expect(appSource).not.toContain('main-container');
		expect(appSource).not.toContain('StatusPanelIsland');
		expect(appSource).not.toContain('WorkCenterIsland');
	});

	it('arranges appbar, unified toolbar, main region, and operations bar in that order', () => {
		const appbarIndex = appShellSource.indexOf('data-testid="app-shell-appbar"');
		const toolbarIndex = appShellSource.indexOf('data-testid="app-shell-toolbar"');
		const mainIndex = appShellSource.indexOf('data-testid="app-shell-main"');
		const operationsIndex = appShellSource.indexOf('data-testid="app-shell-operations"');

		expect(appbarIndex).toBeGreaterThan(-1);
		expect(toolbarIndex).toBeGreaterThan(appbarIndex);
		expect(mainIndex).toBeGreaterThan(toolbarIndex);
		expect(operationsIndex).toBeGreaterThan(mainIndex);
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

		for (const ownerSurface of [
			'getSelectedFileIndices',
			'removeSelectedFiles',
			'openMetadataLookup',
			'openMetadataSurfaceForCurrentSelection',
		]) {
			expect(appShellSource).toContain(ownerSurface);
		}

		expect(fileImportSource).not.toContain('add-folder-btn');
		expect(fileImportSource).not.toContain('acquire-audiobooks-btn');
		expect(operationsBarSource).toContain('<StatusTransportIsland');
		expect(operationsBarSource).toContain('<WorkCenterIsland');
	});

	it('composes encoder and naming popovers before Process through owner strips', () => {
		const encoderIndex = appShellSource.indexOf('data-testid="encoder-popover-trigger"');
		const namingIndex = appShellSource.indexOf('data-testid="naming-popover-trigger"');
		const processIndex = appShellSource.indexOf('id="process-button"');

		expect(encoderIndex).toBeGreaterThan(-1);
		expect(namingIndex).toBeGreaterThan(encoderIndex);
		expect(processIndex).toBeGreaterThan(namingIndex);
		expect(appShellSource).toContain('readEncoderSummaryLabel');
		expect(appShellSource).toContain('readOutputNamingSummaryLabel');
		expect(appShellSource).toContain('<EncoderWorkbenchIsland');
		expect(appShellSource).toContain('<OutputPanelIsland variant="workbench"');
		expect(appShellSource).toContain('<TagPreviewIsland variant="workbench"');
		expect(appShellSource.match(/class="app-popover/g)).toHaveLength(2);
		expect(appShellSource).toContain('{@render overlay?.()}');
	});

	it('renders the selection cluster contextually and feeds the merge chip its file count', () => {
		expect(appShellSource).toContain('{#if selectedFileCount > 0}');
		expect(appShellSource).toContain('<JobControlsIsland {fileCount}');
		expect(appShellSource).toContain('hidden={selectedFileCount > 0}');
		expect(appShellSource).toContain('readFileListCount');
	});

	it('marks every selection-mutating toolbar control for metadata click-away coordination', () => {
		const removeButtonIndex = appShellSource.indexOf('onclick={() => void removeSelectedFiles()}');
		const markerIndex = appShellSource.lastIndexOf(
			'data-metadata-selection-intent',
			removeButtonIndex,
		);

		expect(markerIndex).toBeGreaterThan(-1);
		expect(markerIndex).toBeLessThan(removeButtonIndex);
	});
});

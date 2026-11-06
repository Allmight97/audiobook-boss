import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { replayInvoke } from '../../test/fixtures/recordedInvoke';
import { interpretFileListKeyDown } from '../inputSession';
import { createAppRuntime, type AppRuntime } from './index';

const ALPHA = '/fixture/media/alpha.wav';
const BETA = '/fixture/media/beta.wav';
const BROKEN = '/fixture/media/broken.wav';
const GAMMA = '/fixture/media/gamma.wav';
const SINGLE = { multi: false, range: false };

describe('workflows recorded from the real engine', () => {
	let app: AppRuntime | undefined;
	const original = vi.mocked(invoke).getMockImplementation();

	beforeEach(() => {
		app = undefined;
	});
	afterEach(() => {
		app?.dispose();
		vi.mocked(invoke).mockImplementation(original ?? (() => Promise.resolve(undefined)));
	});

	async function open(): Promise<AppRuntime> {
		app = createAppRuntime();
		await app.initialize();
		return app;
	}

	async function importTitles(runtime: AppRuntime, paths: string[]): Promise<void> {
		await runtime.input.importIntent({ type: 'importPaths', paths });
		await vi.waitFor(() => expect(runtime.input.view().files.length).toBeGreaterThanOrEqual(1));
	}

	function form(runtime: AppRuntime) {
		return runtime.metadata.view().form;
	}

	it('several selected titles show shared values and mark mixed ones', async () => {
		const replay = replayInvoke('session', 'selection', { through: 3 });
		const runtime = await open();

		await importTitles(runtime, [ALPHA, BETA, BROKEN]);
		await runtime.input.selectFile({ index: 0, modifiers: SINGLE });
		expect(form(runtime)).toMatchObject({ mode: 'single', selectionCount: 0 });
		await runtime.input.selectFile({ index: 1, modifiers: { multi: false, range: true } });

		expect(form(runtime)).toMatchObject({ mode: 'multi', selectionCount: 2 });
		expect(form(runtime).fields['meta-title']).toMatchObject({ mixed: true, value: '' });
		expect(form(runtime).fields['meta-author']).toMatchObject({
			mixed: false,
			value: 'Ann Author',
		});
		expect(form(runtime).fields['meta-genre']).toMatchObject({ mixed: false, value: 'Fantasy' });
		replay.finish();
	});

	it('an invalid title counts as empty metadata in a multi selection, and Blank is sent for a mixed field', async () => {
		const replay = replayInvoke('session', 'selection', { through: 6 });
		const runtime = await open();
		await importTitles(runtime, [ALPHA, BETA, BROKEN]);
		await runtime.input.selectFile({ index: 0, modifiers: SINGLE });
		await runtime.input.selectFile({ index: 1, modifiers: { multi: false, range: true } });
		expect(form(runtime).fields['meta-genre'].mixed).toBe(false);

		runtime.input.selectAll();
		await vi.waitFor(() => expect(form(runtime).selectionCount).toBe(3));
		expect(form(runtime).fields['meta-genre']).toMatchObject({ mixed: true, value: '' });

		runtime.metadata.setFieldValue({ field: 'genre', value: '' });
		await vi.waitFor(() => expect(form(runtime).fields['meta-genre'].action).toBe('blank'));
		runtime.metadata.setFieldAction({ field: 'author', action: 'blank' });
		await vi.waitFor(() => expect(form(runtime).fields['meta-author'].action).toBe('blank'));
		replay.finish();
	});

	it('a new selection drops typing for the old form', async () => {
		const replay = replayInvoke('session', 'selection', { through: 7, hold: 5 });
		const runtime = await open();
		await importTitles(runtime, [ALPHA, BETA, BROKEN]);
		await runtime.input.selectFile({ index: 0, modifiers: SINGLE });
		await runtime.input.selectFile({ index: 1, modifiers: { multi: false, range: true } });
		runtime.input.selectAll();
		await vi.waitFor(() => expect(form(runtime).selectionCount).toBe(3));

		runtime.metadata.setFieldValue({ field: 'genre', value: '' });
		expect(form(runtime).fields['meta-genre'].dirty).toBe(true);
		runtime.metadata.setFieldAction({ field: 'author', action: 'blank' });
		runtime.input.clearSelection();

		await vi.waitFor(() => expect(form(runtime).mode).toBe('single'));
		expect(form(runtime).fields['meta-genre'].dirty).toBe(false);
		replay.release();
		await vi.waitFor(() => replay.finish());
	});

	it('select all anchors keyboard moves on the first title', async () => {
		const replay = replayInvoke('session', 'selection', { through: 4 });
		const runtime = await open();
		await importTitles(runtime, [ALPHA, BETA, BROKEN]);
		await runtime.input.selectFile({ index: 0, modifiers: SINGLE });
		await runtime.input.selectFile({ index: 1, modifiers: { multi: false, range: true } });
		expect(runtime.input.view().selectedAnchor).toBe(1);

		runtime.input.selectAll();
		await vi.waitFor(() => expect(runtime.input.view().selectedIndices).toEqual([0, 1, 2]));

		const view = runtime.input.view();
		expect(view.selectedAnchor).toBe(0);
		const next = interpretFileListKeyDown(new KeyboardEvent('keydown', { key: 'ArrowDown' }), view);
		expect(next).toEqual({ type: 'navigate', index: 1 });
		replay.finish();
	});

	it('removing a title keeps the selection on the same book', async () => {
		const replay = replayInvoke('session', 'removal');
		const runtime = await open();
		await importTitles(runtime, [ALPHA, BETA, GAMMA]);
		await runtime.input.selectFile({ index: 2, modifiers: SINGLE });
		const [first] = runtime.input.view().files;

		runtime.input.removeFile(first);
		await vi.waitFor(() => expect(runtime.input.view().files).toHaveLength(2));

		const view = runtime.input.view();
		expect(view.files.map((file) => file.path)).toEqual([BETA, GAMMA]);
		expect(view.selectedIndices).toEqual([1]);
		expect(form(runtime).selectionCount).toBe(0);

		runtime.input.clearAllFiles();
		await vi.waitFor(() => expect(runtime.input.view().files).toHaveLength(0));
		expect(runtime.input.view().selectedIndices).toEqual([]);
		replay.finish();
	});

	it('importing only duplicates tells the user', async () => {
		const replay = replayInvoke('session', 'duplicates');
		const runtime = await open();

		await importTitles(runtime, [ALPHA]);
		await runtime.input.importIntent({ type: 'importPaths', paths: [ALPHA] });
		await vi.waitFor(() =>
			expect(runtime.input.view().errorMessage).toBe(
				'No new files added. All analyzed files were already in the list.',
			),
		);
		expect(runtime.input.view().files).toHaveLength(1);

		await runtime.input.importIntent({ type: 'importPaths', paths: [BETA, BETA] });
		await vi.waitFor(() => expect(runtime.input.view().errorMessage).toBe(''));
		expect(runtime.input.view().files.map((file) => file.path)).toEqual([ALPHA, BETA]);
		replay.finish();
	});

	it('pinned defaults show in the dialog and reset returns to fresh', async () => {
		const replay = replayInvoke('settings', 'pin-reset');
		const runtime = await open();
		runtime.settings.openDialog();

		await runtime.settings.setStartupBehavior('pinnedDefaults');
		expect(runtime.settings.dialog().settings?.startupBehavior).toBe('pinnedDefaults');
		expect(runtime.settings.dialog().settings?.pinnedDefaults).toBeNull();
		await runtime.settings.saveCurrentSettingsAsPinnedDefaults();
		expect(runtime.settings.dialog().settings?.pinnedDefaults).toBeDefined();
		await runtime.settings.setDefaultAcquisitionLane('indexer');
		expect(runtime.settings.defaultAcquisitionLane()).toBe('indexer');

		await runtime.settings.resetAllAppSettings();
		expect(runtime.settings.dialog().settings).toMatchObject({
			startupBehavior: 'rememberLastState',
			defaultAcquisitionLane: 'audible',
		});
		expect(runtime.settings.dialog().settings?.pinnedDefaults).toBeNull();
		replay.finish();
	});

	it('a failed save shows the error, retry clears it, the choice stays', async () => {
		const replay = replayInvoke('settings', 'save-retry');
		const runtime = await open();
		runtime.settings.openDialog();

		await runtime.settings.setKeepAwakeWhileWorking(false);
		expect(runtime.settings.durability().state).toBe('error');
		expect(runtime.settings.durability().message).toMatch(/^IO operation failed/);
		expect(runtime.settings.dialog().settings?.keepAwakeWhileWorking).toBe(false);

		await runtime.settings.retryPersistence();
		expect(runtime.settings.durability().state).toBe('error');
		await runtime.settings.retryPersistence();
		expect(runtime.settings.durability()).toEqual({ state: 'saved', message: '' });
		expect(runtime.settings.dialog().settings?.keepAwakeWhileWorking).toBe(false);
		replay.finish();
	});

	it('a recovery notice survives retry and clears on the next change', async () => {
		const replay = replayInvoke('settings', 'recovery');
		const runtime = await open();
		expect(runtime.settings.recoveryNotice()).toContain('number of jobs');

		await runtime.settings.retryPersistence();
		expect(runtime.settings.recoveryNotice()).toContain('number of jobs');

		await runtime.settings.setKeepAwakeWhileWorking(false);
		expect(runtime.settings.recoveryNotice()).toBe('');
		replay.finish();
	});

	it("a refused concurrency keeps the accepted value and shows the engine's reason", async () => {
		const replay = replayInvoke('settings', 'concurrency');
		const runtime = await open();

		await runtime.settings.setConcurrencySelection('2');
		expect(runtime.settings.concurrency()).toMatchObject({ selection: '2', effective: 2 });

		await runtime.settings.setConcurrencySelection('99');
		expect(runtime.settings.concurrency()).toMatchObject({ selection: '2', effective: 2 });
		expect(runtime.settings.concurrency().errorMessage).toContain('from 1 to 8');
		replay.finish();
	});
});

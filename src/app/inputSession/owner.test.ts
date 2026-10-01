import { afterEach, describe, expect, it, vi } from 'vitest';
import { liveInputCapability } from '../../lib/tauri/capabilities/input';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import type { SessionOutcome } from '../../types/session';
import { createAppRuntime, type AppRuntime } from '../runtime';

// The titles, their order, and the selection live in the engine. These tests
// cover what this owner adds: the intents it sends, the wording of what the
// engine reports, and state that belongs to the view.

describe('input owner', () => {
	let runtime: AppRuntime | undefined;
	let engine: FakeEngine;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	async function open(input = liveInputCapability): Promise<AppRuntime> {
		engine = createFakeEngine();
		runtime = createAppRuntime({ engine, input });
		await runtime.initialize();
		return runtime;
	}

	it('imports the chosen paths and shows the engine titles', async () => {
		const app = await open();

		await app.input.importIntent({ type: 'importPaths', paths: ['/books/a.m4b'] });

		expect(engine.sessionIntents).toContainEqual({ kind: 'import', paths: ['/books/a.m4b'] });
		const view = app.input.view();
		expect(view.files.map((file) => file.path)).toEqual(['/books/a.m4b']);
		expect(view.selectedIndices).toEqual([0]);
		expect(view.selectedAnchor).toBe(0);
		expect(view.totalDurationSeconds).toBe(60);
	});

	it('words the reason an import added nothing', async () => {
		const app = await open();
		await app.input.importIntent({ type: 'importPaths', paths: ['/books/a.m4b'] });

		await app.input.importIntent({ type: 'importPaths', paths: ['/books/a.m4b'] });
		expect(app.input.view().errorMessage).toBe(
			'No new files added. All analyzed files were already in the list.',
		);

		engine.change((state) => {
			state.titles.notice = { kind: 'noSupportedFiles', formatsText: 'MP3 and M4B' };
		});
		expect(app.input.view().errorMessage).toBe(
			'No supported audio files found. Please use MP3 and M4B files.',
		);

		engine.change((state) => {
			state.titles.notice = {
				kind: 'analysisFailed',
				error: { code: 'io_error', category: 'io', message: 'Disk unreadable', detail: null },
			};
		});
		expect(app.input.view().errorMessage).toBe('Disk unreadable');
	});

	it('explains a picker that could not open and sends no import', async () => {
		const app = await open({
			...liveInputCapability,
			openDirectory: vi.fn(async () => {
				throw new Error('boom');
			}),
		});

		await app.input.importIntent({ type: 'pickFolder' });

		expect(app.input.view().errorMessage).toBe('Failed to open folder dialog. Please try again.');
		expect(engine.sessionIntents.some((intent) => intent.kind === 'import')).toBe(false);
	});

	it('reports whether the engine accepted a selection change', async () => {
		const app = await open();
		engine.loadTitles([audioFile('/books/a.m4b'), audioFile('/books/b.m4b')]);
		const select = () =>
			app.input.selectFile({ index: 1, modifiers: { multi: false, range: false } });

		expect(await select()).toBe(true);
		engine.respond = (): SessionOutcome => ({ kind: 'draftRejected', message: 'Bad date' });
		expect(await select()).toBe(false);
	});

	it('keeps drag-over state in the view and out of the engine', async () => {
		const app = await open();
		const sent = engine.sessionIntents.length;

		app.input.setDragOver(true);

		expect(app.input.view().isDragOver).toBe(true);
		expect(engine.sessionIntents).toHaveLength(sent);
	});
});

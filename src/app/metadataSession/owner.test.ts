import { afterEach, describe, expect, it, vi } from 'vitest';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import type { SessionOutcome } from '../../types/session';
import { createAppRuntime, type AppRuntime } from '../runtime';

// The form's rules live in the engine. These tests cover what this owner
// adds: text that shows before the engine confirms it, the wording of what
// the engine reports, and the lifetime of a cover message.

describe('metadata owner', () => {
	let runtime: AppRuntime | undefined;
	let engine: FakeEngine;

	afterEach(() => {
		vi.useRealTimers();
		runtime?.dispose();
		runtime = undefined;
	});

	async function open(): Promise<AppRuntime> {
		engine = createFakeEngine();
		engine.tags.set('/books/alpha.m4b', { title: 'Alpha', artist: 'Author' });
		engine.loadTitles([audioFile('/books/alpha.m4b')], [0]);
		runtime = createAppRuntime({ engine });
		await runtime.initialize();
		return runtime;
	}

	function title(app: AppRuntime) {
		return app.metadata.view().form.fields['meta-title'];
	}

	it('shows typed text at once and keeps the newest text while earlier replies arrive', async () => {
		const app = await open();
		const replies: Array<(outcome: SessionOutcome) => void> = [];
		engine.respond = (intent) =>
			intent.kind === 'setField'
				? new Promise<SessionOutcome>((resolve) => replies.push(resolve))
				: undefined;

		app.metadata.setFieldValue({ inputId: 'meta-title', value: 'Al' });
		app.metadata.setFieldValue({ inputId: 'meta-title', value: 'Alp' });

		expect(title(app)).toMatchObject({ value: 'Alp', dirty: true });
		await vi.waitFor(() => expect(replies).toHaveLength(2));
		// The engine answers the first keystroke; the second is still what shows.
		replies[0]?.({ kind: 'applied' });
		await Promise.resolve();
		await Promise.resolve();
		expect(title(app).value).toBe('Alp');
		replies[1]?.({ kind: 'applied' });
		expect(engine.sessionIntents.filter((intent) => intent.kind === 'setField')).toEqual([
			{ kind: 'setField', field: 'title', value: 'Al' },
			{ kind: 'setField', field: 'title', value: 'Alp' },
		]);
	});

	it('maps each form control to the engine field it edits', async () => {
		const app = await open();

		app.metadata.setFieldValue({ inputId: 'meta-year', value: '2020' });
		app.metadata.setFieldValue({ inputId: 'meta-narrator', value: 'Reader' });
		app.metadata.setFieldAction({ actionId: 'meta-series-part-action', action: 'blank' });
		app.metadata.setFieldValue({ inputId: 'not-a-field', value: 'ignored' });

		await vi.waitFor(() =>
			expect(engine.sessionIntents.slice(-3)).toEqual([
				{ kind: 'setField', field: 'date', value: '2020' },
				{ kind: 'setField', field: 'narrator', value: 'Reader' },
				{ kind: 'setFieldAction', field: 'seriesPart', action: 'blank' },
			]),
		);
	});

	it('words what the engine reports about the form and the last save', async () => {
		const app = await open();

		engine.change((state) => {
			state.metadata.form.seriesPartWarning = { kind: 'missingBookNumber' };
			state.metadata.form.subseriesPartWarning = { kind: 'invalid', message: 'Bad number' };
			state.metadata.form.validationMessage = 'Bad number';
		});
		expect(app.metadata.view().form.seriesPartWarning).toEqual({
			message: 'Series detected - add Book # (series sequence) for ABS ordering.',
			visible: true,
		});
		expect(app.metadata.view().form.subseriesPartWarning).toEqual({
			message: 'Bad number',
			visible: true,
		});
		expect(app.metadata.view().statusMessage).toBe('Bad number');

		// The outcome of the last action shows ahead of a standing problem.
		engine.status({
			kind: 'saveComplete',
			succeeded: 1,
			failed: 0,
			cancelled: 0,
			waiting: 2,
			held: 1,
		});
		expect(app.metadata.view().statusMessage).toBe(
			'Metadata save complete: success=1, failed=0, cancelled=0' +
				' 2 files will be saved when the export reading it finishes.' +
				' 1 file being exported from a download was not changed; the edit is kept for a later export.',
		);

		engine.status({
			kind: 'saveFailed',
			error: { code: 'io_error', category: 'io', message: 'The disk is read-only.', detail: null },
		});
		expect(app.metadata.view().statusMessage).toBe('The disk is read-only.');

		// Saves that waited for an export report how they ended.
		engine.status({ kind: 'deferredWritesFinished', written: 1, failed: 2 });
		expect(app.metadata.view().statusMessage).toBe(
			'1 file saved after the export reading it finished.' +
				' 2 files could not be saved after the export finished. Save again to retry titles still in the list.',
		);
	});

	it('keeps typing for one title off the form of the next', async () => {
		const app = await open();
		engine.respond = (intent) =>
			intent.kind === 'setField' ? new Promise<SessionOutcome>(() => undefined) : undefined;

		app.metadata.setFieldValue({ inputId: 'meta-title', value: 'Typed for alpha' });
		expect(title(app).value).toBe('Typed for alpha');

		// The engine binds the form to another title before the keystroke's reply.
		engine.change((state) => {
			state.metadata.binding += 1;
		});
		expect(title(app).value).toBe('Alpha');
	});

	it('fetches the cover once per image the engine reports and shows it', async () => {
		const app = await open();
		const fetches = vi.spyOn(engine, 'sessionCoverArt');

		await app.metadata.applyCoverArtDrop(['/art/readme.txt', '/art/cover.PNG']);

		expect(engine.sessionIntents).toContainEqual({
			kind: 'loadCoverFromFile',
			path: '/art/cover.PNG',
		});
		await vi.waitFor(() =>
			expect(app.metadata.view().cover.imageDataUrl).toBe('data:image/jpeg;base64,AQID'),
		);
		expect(app.metadata.view().cover.hasCustomCoverArt).toBe(true);
		const fetched = fetches.mock.calls.length;
		// An edit that leaves the image alone does not fetch it again.
		app.metadata.setFieldValue({ inputId: 'meta-genre', value: 'Mystery' });
		await vi.waitFor(() => expect(app.metadata.view().form.fields['meta-genre'].dirty).toBe(true));
		expect(fetches.mock.calls.length).toBe(fetched);
	});

	it('hides a cover message after a moment', async () => {
		const app = await open();
		vi.useFakeTimers();

		await app.metadata.loadCoverArtFromUrl(' https://example.com/cover.jpg ');

		expect(engine.sessionIntents).toContainEqual({
			kind: 'loadCoverFromUrl',
			url: 'https://example.com/cover.jpg',
		});
		expect(app.metadata.view().cover.message).toEqual({
			kind: 'success',
			text: 'Cover art loaded from URL.',
		});
		expect(app.metadata.view().cover.urlInputValue).toBe('https://example.com/cover.jpg');

		vi.advanceTimersByTime(4000);
		expect(app.metadata.view().cover.message).toEqual({ kind: 'hidden' });
	});
});

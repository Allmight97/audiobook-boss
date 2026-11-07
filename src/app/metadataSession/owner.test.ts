import { coverSrc } from '../../lib/tauri/coverSrc';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import type { SessionOutcome } from '../../types/session';
import { createAppRuntime, type AppRuntime } from '../runtime';

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
		engine.loadTitles([audioFile('/books/alpha.m4b')], [0]);
		engine.seedForm({ title: 'Alpha', author: 'Author' });
		runtime = await createAppRuntime({ engine });
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

		app.metadata.setFieldValue({ field: 'title', value: 'Al' });
		app.metadata.setFieldValue({ field: 'title', value: 'Alp' });

		expect(title(app)).toMatchObject({ value: 'Alp', dirty: true });
		await vi.waitFor(() => expect(replies).toHaveLength(2));
		replies[0]?.({ kind: 'applied' });
		await Promise.resolve();
		await Promise.resolve();
		expect(title(app).value).toBe('Alp');
		replies[1]?.({ kind: 'applied' });
		expect(engine.sessionIntents.filter((intent) => intent.kind === 'setField')).toEqual([
			{ kind: 'setField', binding: 0, field: 'title', value: 'Al' },
			{ kind: 'setField', binding: 0, field: 'title', value: 'Alp' },
		]);
	});

	it('shows a refused edit and drops its typed text', async () => {
		const app = await open();
		engine.respond = (intent): SessionOutcome | undefined =>
			intent.kind === 'setField'
				? {
						kind: 'rejected',
						error: {
							code: 'invalid_input',
							category: 'validation',
							message: 'That title cannot be edited now.',
							detail: null,
						},
					}
				: undefined;

		app.metadata.setFieldValue({ field: 'title', value: 'Typed' });
		expect(title(app).value).toBe('Typed');

		await vi.waitFor(() => expect(app.engine.refusal()).toBe('That title cannot be edited now.'));
		expect(title(app).value).toBe('Alpha');
	});

	it('shows why the engine refused a save', async () => {
		const app = await open();
		engine.respond = (intent): SessionOutcome | undefined =>
			intent.kind === 'save'
				? {
						kind: 'rejected',
						error: {
							code: 'invalid_input',
							category: 'validation',
							message: 'Nothing to save.',
							detail: null,
						},
					}
				: undefined;

		expect(app.metadata.save()).toBeUndefined();

		await vi.waitFor(() => expect(app.engine.refusal()).toBe('Nothing to save.'));
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

		engine.status({
			kind: 'saveComplete',
			succeeded: 1,
			failed: 0,
			cancelled: 0,
			waiting: 2,
			held: 1,
			outputs: { updated: 2, elsewhere: 0, restartOffered: 1, failed: 0 },
		});
		expect(app.metadata.view().statusMessage).toBe(
			'Metadata save complete: success=1, failed=0, cancelled=0' +
				' 2 files will be saved when the export reading it finishes.' +
				' 1 file from a download was not changed; the edit goes with its exports.' +
				' 2 exports updated. 1 export would move; choose whether to restart.',
		);

		engine.status({
			kind: 'saveFailed',
			error: { code: 'io_error', category: 'io', message: 'The disk is read-only.', detail: null },
		});
		expect(app.metadata.view().statusMessage).toBe('The disk is read-only.');

		engine.status({ kind: 'deferredWritesFinished', written: 1, failed: 2 });
		expect(app.metadata.view().statusMessage).toBe(
			'1 file saved after the export reading it finished.' +
				' 2 files could not be saved after the export finished. Save again to retry titles still in the list.',
		);
	});

	it('names the titles whose tags could not be read beside the form that stands in for them', async () => {
		const app = await open();

		engine.status({ kind: 'tagReadFailed', label: 'alpha.m4b', more: 0 });
		expect(app.metadata.view().statusMessage).toBe(
			'Could not read the tags of alpha.m4b. The fields shown are not its tags.',
		);

		engine.status({ kind: 'tagReadFailed', label: 'alpha.m4b', more: 2 });
		expect(app.metadata.view().statusMessage).toBe(
			'Could not read the tags of alpha.m4b and 2 other files. The fields shown are not their tags.',
		);
	});

	it('keeps typing for one title off the form of the next', async () => {
		const app = await open();
		engine.respond = (intent) =>
			intent.kind === 'setField' ? new Promise<SessionOutcome>(() => undefined) : undefined;

		app.metadata.setFieldValue({ field: 'title', value: 'Typed for alpha' });
		expect(title(app).value).toBe('Typed for alpha');

		engine.change((state) => {
			state.metadata.binding += 1;
		});
		expect(title(app).value).toBe('Alpha');
	});

	it('types into the form it shows, so the engine can refuse text meant for a form it replaced', async () => {
		const app = await open();
		engine.change((state) => {
			state.metadata.binding = 7;
		});

		app.metadata.setFieldValue({ field: 'title', value: 'Typed' });
		app.metadata.setFieldAction({ field: 'author', action: 'blank' });

		await vi.waitFor(() =>
			expect(engine.sessionIntents.slice(-2)).toEqual([
				{ kind: 'setField', binding: 7, field: 'title', value: 'Typed' },
				{ kind: 'setFieldAction', binding: 7, field: 'author', action: 'blank' },
			]),
		);
	});

	it("shows the engine's cover at an address that names that image", async () => {
		const app = await open();
		engine.respond = (intent) => {
			if (intent.kind !== 'loadCoverFromDrop') return undefined;
			engine.seedCover();
			return { kind: 'applied' };
		};

		await app.metadata.applyCoverArtDrop(['/art/readme.txt', '/art/cover.PNG']);

		expect(engine.sessionIntents).toContainEqual({
			kind: 'loadCoverFromDrop',
			paths: ['/art/readme.txt', '/art/cover.PNG'],
		});
		await vi.waitFor(() => expect(app.metadata.view().cover.imageSrc).not.toBeNull());
		expect(app.metadata.view().cover.imageSrc).toBe(coverSrc('session/1/'));
		const shown = app.metadata.view().cover.imageSrc;
		app.metadata.setFieldValue({ field: 'genre', value: 'Mystery' });
		await vi.waitFor(() => expect(app.metadata.view().form.fields['meta-genre'].dirty).toBe(true));
		expect(app.metadata.view().cover.imageSrc).toBe(shown);

		engine.seedCover();
		await vi.waitFor(() => expect(app.metadata.view().cover.imageSrc).not.toBe(shown));
	});

	it('hides a cover message after a moment', async () => {
		const app = await open();
		vi.useFakeTimers();
		engine.respond = (intent) => {
			if (intent.kind !== 'loadCoverFromUrl') return undefined;
			engine.seedCover({ kind: 'loadedFromUrl' });
			return { kind: 'applied' };
		};

		app.metadata.loadCoverArtFromUrl(' https://example.com/cover.jpg ');

		await vi.waitFor(() =>
			expect(app.metadata.view().cover.message).toEqual({
				kind: 'success',
				text: 'Cover art loaded from URL.',
			}),
		);
		expect(engine.sessionIntents).toContainEqual({
			kind: 'loadCoverFromUrl',
			url: 'https://example.com/cover.jpg',
		});
		expect(app.metadata.view().cover.urlInputValue).toBe('https://example.com/cover.jpg');

		vi.advanceTimersByTime(4000);
		expect(app.metadata.view().cover.message).toEqual({ kind: 'hidden' });
	});

	it('says to select one title when a cover had none to go to', async () => {
		const app = await open();
		engine.respond = (intent) => {
			if (intent.kind !== 'loadCoverFromUrl') return undefined;
			engine.seedCover({ kind: 'noTitleForCover' });
			return { kind: 'coverLoadFailed' };
		};

		app.metadata.loadCoverArtFromUrl('https://example.com/cover.jpg');

		await vi.waitFor(() =>
			expect(app.metadata.view().cover.message).toEqual({
				kind: 'error',
				text: 'Select one title to set its cover.',
			}),
		);
	});
});

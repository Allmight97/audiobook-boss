import { afterEach, describe, expect, it, vi } from 'vitest';
import type { SessionOutcome } from '../../types/session';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { titleAudioRequest } from '../../test/fixtures/titleAudio';
import { tauriClient } from '../../lib/tauri/client';
import { createAppRuntime, type AppRuntime } from '../runtime';

describe('output plan owner', () => {
	let runtime: AppRuntime | undefined;
	let engine: FakeEngine;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	async function open(): Promise<AppRuntime> {
		engine = createFakeEngine();
		engine.loadTitles([audioFile('/books/a.m4b', { duration: 100, size: 1024 })]);
		runtime = await createAppRuntime({ engine });
		return runtime;
	}

	it('words the engine preview and naming hint', async () => {
		const app = await open();
		expect(app.output.view().previewText).toBe('Select output directory...');

		engine.change((state) => {
			state.output.directory = '/out';
			state.output.includeYear = true;
			state.output.preview = { kind: 'path', path: '/out/Author/Book/Book.m4b' };
		});
		expect(app.output.view()).toMatchObject({
			outputDirectory: '/out',
			previewText: '/out/Author/Book/Book.m4b',
		});

		engine.change((state) => {
			state.output.preview = { kind: 'unavailable', message: 'bad template' };
		});
		expect(app.output.view()).toMatchObject({
			previewText: 'bad template',
			previewTitle: 'bad template',
		});
	});

	it('sends output choices and shows typed template text until the engine confirms it', async () => {
		const app = await open();
		const replies: Array<(outcome: SessionOutcome) => void> = [];
		engine.respond = (intent) =>
			intent.kind === 'setNamingTemplate'
				? new Promise<SessionOutcome>((resolve) => replies.push(resolve))
				: undefined;

		app.output.selectNamingPreset('customTemplate');
		app.output.setAbsIncludeYear(true);
		app.output.editNamingTemplate('{author}');
		app.output.editNamingTemplate('{author}/{title}');

		expect(app.output.view().namingTemplate).toBe('{author}/{title}');
		await vi.waitFor(() => expect(replies).toHaveLength(2));
		replies[0]?.({ kind: 'applied' });
		await Promise.resolve();
		expect(app.output.view().namingTemplate).toBe('{author}/{title}');
		expect(engine.sessionIntents.slice(-4)).toEqual([
			{ kind: 'setNamingPreset', preset: 'customTemplate' },
			{ kind: 'setIncludeYear', includeYear: true },
			{ kind: 'setNamingTemplate', template: '{author}' },
			{ kind: 'setNamingTemplate', template: '{author}/{title}' },
		]);
	});

	it('shows why the engine refused a naming template', async () => {
		const app = await open();
		engine.respond = (intent): SessionOutcome | undefined =>
			intent.kind === 'setNamingTemplate'
				? {
						kind: 'rejected',
						error: {
							code: 'invalid_input',
							category: 'validation',
							message: 'The template needs a {title}.',
							detail: null,
						},
					}
				: undefined;

		app.output.editNamingTemplate('{author}');

		await vi.waitFor(() => expect(app.engine.refusal()).toBe('The template needs a {title}.'));
	});

	it('shows a directory picker that failed to open', async () => {
		const app = await open();
		vi.spyOn(tauriClient, 'openDirectory').mockRejectedValueOnce({
			code: 'dialog_failed',
			category: 'io',
			message: 'The folder dialog could not open.',
			detail: null,
		});

		await app.output.browseDirectory();

		expect(app.engine.refusal()).toBe('The folder dialog could not open.');
		expect(engine.sessionIntents.some((intent) => intent.kind === 'setOutputDirectory')).toBe(
			false,
		);
	});

	it('words each title estimate from the engine', async () => {
		const app = await open();
		const [title] = app.input.view().files;
		const id = title.inputId ?? title.path;
		expect(app.output.estimateTitleSizeText(title)).toBeNull();

		engine.seedTitleAudio(id, titleAudioRequest(), {
			estimate: { kind: 'bytes', bytes: 1_048_576 },
		});
		expect(app.output.estimateTitleSizeText(title)).toBe('Est. ~ 1.0 MB');
		engine.seedTitleAudio(id, titleAudioRequest(), { estimate: { kind: 'variesWithAudio' } });
		expect(app.output.estimateTitleSizeText(title)).toBe('Size varies with audio');
	});
});

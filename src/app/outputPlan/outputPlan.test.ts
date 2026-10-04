import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PlannedOutput } from '../../types/audio';
import type { SessionOutcome } from '../../types/session';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { titleAudioRequest } from '../../test/fixtures/titleAudio';
import { createAppRuntime, type AppRuntime } from '../runtime';

// Naming, the path preview, and size estimates are the engine's. These tests
// cover what this owner adds: wording, the intents it sends, the template
// shown while typing, and the collision dialog.

/** The outputs the engine says already exist. */
function collidedOutputs(): PlannedOutput[] {
	return [
		{
			inputIndex: 1,
			inputPath: '/books/b.m4b',
			kind: 'final',
			requestedPath: '/tmp/out/b.m4b',
			resolvedPath: '/tmp/out/b.m4b',
			renameCandidate: '/tmp/out/b-1.m4b',
			collision: {
				kind: 'existing_file',
				conflictingPath: '/tmp/out/b.m4b',
				detail: 'An existing file already occupies the destination path.',
			},
			action: 'review_required',
		},
	];
}

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
		runtime = createAppRuntime({ engine });
		await runtime.initialize();
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
			absHintHidden: false,
		});
		expect(app.output.view().absHintText).toContain('YYYY');

		engine.change((state) => {
			state.output.preview = { kind: 'unavailable', message: 'bad template' };
		});
		expect(app.output.view().previewText).toBe(
			'Output preview unavailable. Fix metadata/template and retry.',
		);
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
		expect(app.output.estimateTitleSizeText({ ...title, isValid: false })).toBeNull();
	});
});

describe('engine collision questions', () => {
	const runtimes: AppRuntime[] = [];
	afterEach(() => {
		for (const runtime of runtimes.splice(0)) runtime.dispose();
	});

	async function attach(engine: FakeEngine): Promise<AppRuntime> {
		const runtime = createAppRuntime({ engine });
		runtimes.push(runtime);
		await runtime.initialize();
		return runtime;
	}
	function seed(engine: FakeEngine, reviewId: number): void {
		engine.change((state) => {
			state.output.collisionReview = { reviewId, outputs: collidedOutputs(), preview: false };
			state.output.submissionInProgress = true;
		});
	}

	it('renders the held question and sends its identity without deciding it locally', async () => {
		const engine = createFakeEngine();
		seed(engine, 7);
		const runtime = await attach(engine);
		expect(runtime.output.collision()).toMatchObject({ isOpen: true, reviewId: 7 });
		expect(runtime.output.collision().outputs[0]?.inputPath).toBe('/books/b.m4b');
		expect(runtime.output.collision().body).toContain('1 file with the same name');
		runtime.output.chooseCollisionPolicy(7, 'rename_new');
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'chooseCollisionPolicy',
				reviewId: 7,
				policy: 'rename_new',
			}),
		);
		// The test engine has not answered the held decision. No local store closes it.
		expect(runtime.output.collision().isOpen).toBe(true);
	});

	it('disposal never answers a question and replacement restores the engine review', async () => {
		const engine = createFakeEngine();
		seed(engine, 8);
		const first = await attach(engine);
		first.dispose();
		await Promise.resolve();
		expect(engine.sessionIntents).toEqual([]);
		expect(first.output.collision().isOpen).toBe(false);
		const replacement = await attach(engine);
		expect(replacement.output.collision()).toMatchObject({ isOpen: true, reviewId: 8 });
		replacement.output.cancelCollisionReview(8);
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toEqual([{ kind: 'cancelCollisionReview', reviewId: 8 }]),
		);
	});

	it('a newer question does not cancel the old one or retarget a captured answer', async () => {
		const engine = createFakeEngine();
		seed(engine, 9);
		const runtime = await attach(engine);
		const seen = runtime.output.collision().reviewId!;
		seed(engine, 10);
		expect(runtime.output.collision().reviewId).toBe(10);
		expect(engine.sessionIntents).toEqual([]);
		runtime.output.chooseCollisionPolicy(seen, 'replace_existing');
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toEqual([
				{ kind: 'chooseCollisionPolicy', reviewId: 9, policy: 'replace_existing' },
			]),
		);
	});
});

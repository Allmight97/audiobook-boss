import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PlannedOutput } from '../../types/audio';
import type { SessionOutcome } from '../../types/session';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { titleAudioRequest } from '../../test/fixtures/titleAudio';
import { createAppRuntime, type AppRuntime } from '../runtime';
import type { CollisionView } from './collision';

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

describe('collision review', () => {
	let runtime: ReturnType<typeof createAppRuntime> | undefined;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	function collision(): CollisionView {
		return runtime!.output.collision();
	}

	it('cancel resolves null and closes the dialog', async () => {
		runtime = createAppRuntime();
		const result = runtime.output.openCollisionReview(collidedOutputs());
		runtime.output.cancelCollisionReview();
		await expect(result).resolves.toBeNull();
		expect(collision().isOpen).toBe(false);
		expect(collision().outputs).toEqual([]);
	});

	it('opening a second dialog resolves the first as cancelled', async () => {
		runtime = createAppRuntime();
		const first = runtime.output.openCollisionReview(collidedOutputs());
		const second = runtime.output.openCollisionReview(collidedOutputs());
		await expect(first).resolves.toBeNull();
		runtime.output.chooseCollisionPolicy('rename_new');
		await expect(second).resolves.toBe('rename_new');
		expect(collision().isOpen).toBe(false);
	});

	it('words how many outputs already exist', () => {
		runtime = createAppRuntime();
		void runtime.output.openCollisionReview(collidedOutputs());
		expect(collision().outputs[0]?.inputPath).toBe('/books/b.m4b');
		expect(collision().body).toBe(
			'1 file with the same name already exists in the target output folder. How do you want to resolve the conflict?',
		);
	});
});

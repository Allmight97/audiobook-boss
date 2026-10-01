import { titleAudioRequest } from '../../test/fixtures/titleAudio';
import { createRoot, createSignal, flush, runWithOwner, type Accessor } from 'solid-js';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ProcessingPreflightPlan } from '../../types/audio';
import { tauriClient } from '../../lib/tauri/client';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import type { AudioFile } from '../../types/audio';
import type { MetadataField } from '../../types/session';
import { createAppRuntime } from '../runtime';
import type { InputView } from '../inputSession';
import { createOutputOwner, type OutputPlanOwner } from '.';
import type { CollisionView } from './collision';
import { previewDraftFromMetadataView, sourcePathFromInput } from './previewDraft';
import { HIDDEN_COVER_MESSAGE } from '../metadataSession/cover';
import { toFormState } from '../metadataSession/fields';
import { projectTagPreviewValues } from '../metadataSession/tags';
import type { MetadataView } from '../metadataSession';

function fileWithDuration(duration: number): AudioFile {
	return audioFile('/books/a.m4b', { duration, size: 1024, chapters: undefined });
}

/** A runtime whose engine already holds `files`. */
async function runtimeWith(
	files: AudioFile[],
): Promise<{ runtime: ReturnType<typeof createAppRuntime>; engine: FakeEngine }> {
	const engine = createFakeEngine();
	const runtime = createAppRuntime({ engine });
	engine.loadTitles(files);
	await runtime.initialize();
	return { runtime, engine };
}

/** The metadata view with the given field values on screen. */
function metadataViewShowing(values: Partial<Record<MetadataField, string>> = {}): MetadataView {
	const form = toFormState(
		{
			mode: 'single',
			selectionCount: 0,
			fields: Object.entries(values).map(([field, value]) => ({
				field: field as MetadataField,
				value,
				action: 'keep',
				dirty: false,
				mixed: false,
			})),
			seriesPartWarning: null,
			subseriesPartWarning: null,
			validationMessage: null,
		},
		new Map(),
	);
	return {
		form,
		cover: {
			imageDataUrl: null,
			isLoading: false,
			message: HIDDEN_COVER_MESSAGE,
			isHovered: false,
			isDragOver: false,
			urlInputValue: '',
			hasCustomCoverArt: false,
			coverArtRemovalRequested: false,
		},
		tags: projectTagPreviewValues(form, ''),
		saveInProgress: false,
		statusMessage: '',
	};
}

function emptyInputView(overrides: Partial<InputView> = {}): InputView {
	return {
		files: [],
		sourceFiles: [],
		selectedSourceFiles: (overrides.selectedIndices ?? []).flatMap(
			(index) => overrides.files?.[index] ?? [],
		),
		selectedIndices: [],
		selectedAnchor: -1,
		fileCount: 0,
		hasFiles: false,
		orderLocked: false,
		errorMessage: '',
		isDragOver: false,
		supportText: '',
		sortDirection: 'none',
		sortLabel: 'Sort: A-Z',
		orderDiffersFromImport: false,
		showSortButton: false,
		showClearButton: false,
		showRestoreImportOrder: false,
		totalDurationSeconds: 0,
		...overrides,
	};
}

function collisionPlan(): ProcessingPreflightPlan {
	return {
		previewSeconds: undefined,
		collisionPolicy: 'fail',
		audioPlans: [],
		planSignature: 'sig-review',
		outputs: [
			{
				inputIndex: 0,
				inputPath: '/books/a.m4b',
				kind: 'final',
				requestedPath: '/tmp/out/a.m4b',
				resolvedPath: '/tmp/out/a.m4b',
				renameCandidate: undefined,
				collision: undefined,
				action: 'write',
			},
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
		],
	};
}

type MountedOutput = {
	readonly owner: OutputPlanOwner;
	dispose(): void;
};

function mountOutput(
	runtime: ReturnType<typeof createAppRuntime>,
	overrides: {
		readonly metadataView?: Accessor<MetadataView>;
	} = {},
): MountedOutput {
	return runWithOwner(null, () =>
		createRoot((dispose) => {
			const owner = createOutputOwner({
				persistDefaults: () => undefined,
				input: runtime.input,
				metadataView: overrides.metadataView ?? (() => metadataViewShowing()),
				encoding: runtime.encoding,
			});
			return {
				owner,
				dispose,
			};
		}),
	);
}

describe('output plan public view', () => {
	let runtime: ReturnType<typeof createAppRuntime> | undefined;
	let mounted: MountedOutput | undefined;

	afterEach(() => {
		vi.useRealTimers();
		mounted?.dispose();
		mounted = undefined;
		runtime?.dispose();
		runtime = undefined;
	});

	it('hydrates output defaults through the public strip without a preview poke API', () => {
		runtime = createAppRuntime();
		mounted = mountOutput(runtime);
		mounted.owner.applyDefaults({
			outputDirectory: '/books/out',
			outputNaming: { preset: 'absDefault', includeYear: true },
		});
		const view = mounted.owner.view();
		expect(view.outputDirectory).toBe('/books/out');
		expect(view.absIncludeYear).toBe(true);
		expect(view.absHintHidden).toBe(false);
		expect(view.absHintText).toContain('YYYY');
		expect(mounted.owner.readRequestConfig().outputDirectory).toBe('/books/out');
	});

	it('uses total target bitrate for mono and stereo output estimates', async () => {
		({ runtime } = await runtimeWith([fileWithDuration(100)]));
		mounted = mountOutput(runtime);
		await vi.waitFor(() => {
			expect(runtime!.encoding.view().flavorOptions.length).toBeGreaterThan(1);
		});
		runtime.encoding.selectTitles(runtime.input.view().files, 'encoder', 'native_aac');
		runtime.encoding.selectTitles(runtime.input.view().files, 'bitrate', '64');
		runtime.encoding.selectTitles(runtime.input.view().files, 'channels', 'mono');
		flush();
		const title = runtime.input.view().files[0]!;
		expect(mounted.owner.estimateTitleSizeText(title)).toBe('Est. ~ 804.7 KB');
		runtime.encoding.selectTitles(runtime.input.view().files, 'channels', 'stereo');
		flush();
		expect(mounted.owner.estimateTitleSizeText(title)).toBe('Est. ~ 804.7 KB');
		runtime.encoding.selectTitles(runtime.input.view().files, 'encoder', 'faac');
		runtime.encoding.selectTitles(runtime.input.view().files, 'rateControl', 'vbr');
		flush();
		expect(mounted.owner.estimateTitleSizeText(title)).toBe('Size varies with audio');
	});

	it('estimates a stack from all source sizes or durations for its selected handling', async () => {
		const first = fileWithDuration(100);
		const second = {
			...first,
			path: '/books/b.m4b',
			inputId: '/books/b.m4b',
			size: 2048,
			duration: 50,
		};
		const loaded = await runtimeWith([first]);
		runtime = loaded.runtime;
		const groupSources = (sources: AudioFile[]) =>
			loaded.engine.change((state) => {
				state.titles.titleSourcesByIdentity = { [first.path]: sources };
			});
		groupSources([first, second]);
		mounted = mountOutput(runtime);
		runtime.encoding.selectTitle(first, 'intent', 'preserve');
		flush();
		expect(mounted.owner.estimateTitleSizeText(first)).toBe('Est. ~ 3.0 KB');
		await vi.waitFor(() =>
			expect(runtime!.encoding.view().flavorOptions.length).toBeGreaterThan(1),
		);
		runtime.encoding.selectTitle(first, 'encoder', 'native_aac');
		runtime.encoding.selectTitle(first, 'bitrate', '64');
		expect(mounted.owner.estimateTitleSizeText(first)).toBe('Est. ~ 1.2 MB');
		groupSources([first, { ...second, size: undefined, duration: undefined }]);
		runtime.encoding.selectTitle(first, 'intent', 'preserve');
		expect(mounted.owner.estimateTitleSizeText(first)).toBeNull();
		runtime.encoding.selectTitle(first, 'intent', 'encode');
		expect(mounted.owner.estimateTitleSizeText(first)).toBeNull();
	});

	it('waits for Auto audio planning before estimating size and previews the selected format', async () => {
		const previewOutputPath = vi
			.spyOn(tauriClient, 'previewOutputPath')
			.mockResolvedValue('/books/out/a.mp3');
		const source = {
			...fileWithDuration(100),
			path: '/books/a.mp3',
			inputId: '/books/a.mp3',
			size: 1_048_576,
			preservation: { canPreserve: true },
		};
		const session = {
			files: [source, { ...source, path: '/books/b.m4b', inputId: '/books/b.m4b', duration: 100 }],
		};
		({ runtime } = await runtimeWith(session.files));
		mounted = mountOutput(runtime);
		mounted.owner.applyDefaults({
			outputDirectory: '/books/out',
			outputNaming: { preset: 'absDefault', includeYear: false },
		});
		await vi.waitFor(() =>
			expect(runtime!.encoding.view().flavorOptions.length).toBeGreaterThan(1),
		);
		runtime.encoding.select('encoder', 'native_aac');
		runtime.encoding.select('bitrate', '64');
		runtime.input.setAudioRequest(
			source,
			titleAudioRequest({ format: 'mp3', intent: 'auto', settings: null }),
		);
		flush();
		expect(mounted.owner.estimateTitleSizeText(source)).toBeNull();
		expect(
			mounted.owner.estimateTitleSizeText(source, {
				format: 'mp3',
				handling: 'preserve',
				settings: null,
				sampleRate: 44100,
				channels: 1,
				sourceCodec: 'MP3',
				reason: null,
			}),
		).toBe('Est. ~ 1.0 MB');
		await vi.waitFor(() =>
			expect(previewOutputPath).toHaveBeenLastCalledWith(
				expect.objectContaining({
					sourcePath: '/books/a.mp3',
					format: 'mp3',
				}),
			),
		);
		runtime.input.setAudioRequest(session.files[1]!, titleAudioRequest({ intent: 'auto' }));
		runtime.encoding.select('bitrate', '192');
		flush();
		expect(mounted.owner.estimateTitleSizeText(source)).toBeNull();
		runtime.input.setAudioRequest(
			source,
			titleAudioRequest({ format: 'mp3', intent: 'preserve', settings: null }),
		);
		flush();
		expect(mounted.owner.estimateTitleSizeText(source)).toBe('Est. ~ 1.0 MB');
		expect(mounted.owner.estimateTitleSizeText(session.files[1]!)).toBeNull();
		expect(
			mounted.owner.estimateTitleSizeText(session.files[1]!, {
				format: 'm4b',
				handling: 'encode',
				sampleRate: 44100,
				channels: 1,
				sourceCodec: 'AAC',
				reason: null,
				settings: { ...titleAudioRequest().settings!, bitrateKbps: 80 },
			}),
		).toBe('Est. ~ 1005.9 KB');
	});

	it('reads live naming template on submit before preview debounce completes', async () => {
		const previewOutputPath = vi
			.spyOn(tauriClient, 'previewOutputPath')
			.mockResolvedValue('/books/out/preview.m4b');
		runtime = createAppRuntime();
		mounted = mountOutput(runtime);
		mounted.owner.applyDefaults({
			outputDirectory: '/books/out',
			outputNaming: {
				preset: 'customTemplate',
				includeYear: false,
				customTemplate: '{old}',
			},
		});
		await vi.waitFor(() => expect(previewOutputPath).toHaveBeenCalled());
		previewOutputPath.mockClear();

		mounted.owner.editNamingTemplate('{author}/{title}');
		expect(mounted.owner.view().namingTemplate).toBe('{author}/{title}');
		expect(mounted.owner.readRequestConfig().outputNaming.customTemplate).toBe('{author}/{title}');
		await Promise.resolve();
		expect(previewOutputPath).not.toHaveBeenCalled();

		await new Promise((resolve) => setTimeout(resolve, 50));
		expect(previewOutputPath).not.toHaveBeenCalled();
		expect(mounted.owner.readRequestConfig().outputNaming.customTemplate).toBe('{author}/{title}');

		await vi.waitFor(
			() => {
				const lastCall = previewOutputPath.mock.calls[previewOutputPath.mock.calls.length - 1];
				expect(lastCall?.[0]?.outputNaming?.customTemplate).toBe('{author}/{title}');
			},
			{ timeout: 500 },
		);
		previewOutputPath.mockRestore();
	});

	it('re-reads output path preview when series part changes', async () => {
		const previewOutputPath = vi
			.spyOn(tauriClient, 'previewOutputPath')
			.mockResolvedValue('/books/out/preview.m4b');
		({ runtime } = await runtimeWith([fileWithDuration(100)]));
		const [metadataView, setMetadataView] = createSignal<MetadataView>(
			metadataViewShowing({ seriesPart: '1' }),
		);
		mounted = mountOutput(runtime, { metadataView });
		mounted.owner.applyDefaults({
			outputDirectory: '/books/out',
			outputNaming: { preset: 'absDefault', includeYear: false },
		});
		await vi.waitFor(() => expect(previewOutputPath).toHaveBeenCalled());
		const firstCall = previewOutputPath.mock.calls[previewOutputPath.mock.calls.length - 1];
		expect(firstCall?.[0]?.metadata?.series_part).toBe('1');
		const callsBefore = previewOutputPath.mock.calls.length;
		setMetadataView(metadataViewShowing({ seriesPart: '2' }));
		await vi.waitFor(() =>
			expect(previewOutputPath.mock.calls.length).toBeGreaterThan(callsBefore),
		);
		const lastCall = previewOutputPath.mock.calls[previewOutputPath.mock.calls.length - 1];
		expect(lastCall?.[0]?.metadata?.series_part).toBe('2');
		previewOutputPath.mockRestore();
	});

	it('keeps the latest in-flight path preview and ignores a stale slower answer', async () => {
		let resolveFirst: ((path: string) => void) | undefined;
		const previewOutputPath = vi.spyOn(tauriClient, 'previewOutputPath');
		previewOutputPath
			.mockImplementationOnce(
				() =>
					new Promise<string>((resolve) => {
						resolveFirst = resolve;
					}),
			)
			.mockResolvedValue('/books/out/second.m4b');
		({ runtime } = await runtimeWith([]));
		mounted = mountOutput(runtime);
		const owner = mounted.owner;
		owner.applyDefaults({
			outputDirectory: '/books/out',
			outputNaming: { preset: 'absDefault', includeYear: false },
		});
		await vi.waitFor(() => expect(previewOutputPath).toHaveBeenCalledTimes(1));
		owner.setAbsIncludeYear(true);
		await vi.waitFor(() => expect(owner.view().previewText).toBe('/books/out/second.m4b'));
		resolveFirst?.('/books/out/stale.m4b');
		await Promise.resolve();
		expect(owner.view().previewText).toBe('/books/out/second.m4b');
		previewOutputPath.mockRestore();
	});
});

describe('output path preview projection', () => {
	it('prefers the first selected file path, then the first valid file', () => {
		expect(
			sourcePathFromInput(
				emptyInputView({
					files: [
						{ path: '/skip.m4b', isValid: false },
						{ path: '/keep.m4b', isValid: true },
					],
					fileCount: 2,
					hasFiles: true,
					showSortButton: true,
					showClearButton: true,
				}),
			),
		).toBe('/keep.m4b');
	});

	it('projects public metadata view fields into the native preview draft', () => {
		const draft = previewDraftFromMetadataView(
			metadataViewShowing({ title: 'Dune', author: 'Herbert' }),
		);
		expect(draft.title).toBe('Dune');
		expect(draft.artist).toBe('Herbert');
		expect(draft.album).toBe('Dune');
		expect(draft.cover_art).toBeUndefined();
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
		const result = runtime.output.openCollisionReview(collisionPlan());
		runtime.output.cancelCollisionReview();
		await expect(result).resolves.toBeNull();
		expect(collision().isOpen).toBe(false);
		expect(collision().outputs).toEqual([]);
	});

	it('opening a second dialog resolves the first as cancelled', async () => {
		runtime = createAppRuntime();
		const first = runtime.output.openCollisionReview(collisionPlan());
		const second = runtime.output.openCollisionReview(collisionPlan());
		await expect(first).resolves.toBeNull();
		runtime.output.chooseCollisionPolicy('rename_new');
		await expect(second).resolves.toBe('rename_new');
		expect(collision().isOpen).toBe(false);
	});

	it('exposes only collided outputs', () => {
		runtime = createAppRuntime();
		void runtime.output.openCollisionReview(collisionPlan());
		expect(collision().outputs).toHaveLength(1);
		expect(collision().outputs[0]?.inputPath).toBe('/books/b.m4b');
		expect(collision().body).toBe(
			'1 file with the same name already exists in the target output folder. How do you want to resolve the conflict?',
		);
	});
});

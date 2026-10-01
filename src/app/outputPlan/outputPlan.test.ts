import { titleAudioRequest } from '../../test/fixtures/titleAudio';
import { createRoot, createSignal, flush, runWithOwner, type Accessor } from 'solid-js';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ProcessingPreflightPlan } from '../../types/audio';
import { tauriClient } from '../../lib/tauri/client';
import { createAppRuntime } from '../runtime';
import { emptyInputSession } from '../inputSession/types';
import type { InputView } from '../inputSession';
import { createOutputOwner, type OutputPlanOwner } from '.';
import type { CollisionView } from './collision';
import { previewDraftFromMetadataView, sourcePathFromInput } from './previewDraft';
import { createEmptyCoverUiState } from '../metadataSession/cover';
import { createEmptyFormState, replaceField } from '../metadataSession/fields';
import { projectTagPreviewValues } from '../metadataSession/tags';
import type { MetadataDraftValidation, MetadataView } from '../metadataSession';

function sessionWithDuration(totalDuration: number) {
	return {
		...emptyInputSession(),
		files: [
			{
				path: '/books/a.m4b',
				isValid: true,
				duration: totalDuration,
				size: 1024,
				format: 'm4b',
			},
		],
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

function emptyMetadataView(): MetadataView {
	return {
		form: createEmptyFormState(),
		cover: createEmptyCoverUiState(),
		tags: projectTagPreviewValues(createEmptyFormState(), ''),
		saveInProgress: false,
		focusedFieldId: null,
		statusMessage: '',
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
		readonly onMetadataValidation?: (validation: MetadataDraftValidation) => void;
	} = {},
): MountedOutput {
	return runWithOwner(null, () =>
		createRoot((dispose) => {
			const owner = createOutputOwner({
				persistDefaults: () => undefined,
				input: runtime.input,
				metadataView: overrides.metadataView ?? emptyMetadataView,
				encoding: runtime.encoding,
				onMetadataValidation: overrides.onMetadataValidation,
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
		runtime = createAppRuntime();
		runtime.input.replaceSession(sessionWithDuration(100));
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
		runtime = createAppRuntime();
		const session = sessionWithDuration(100);
		const first = session.files[0]!;
		const second = { ...first, path: '/books/b.m4b', size: 2048, duration: 50 };
		runtime.input.replaceSession({
			...session,
			titleSourcesByIdentity: { [first.path]: [first, second] },
		});
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
		runtime.input.replaceSession({
			...session,
			titleSourcesByIdentity: {
				[first.path]: [first, { ...second, size: undefined, duration: undefined }],
			},
		});
		runtime.encoding.selectTitle(first, 'intent', 'preserve');
		expect(mounted.owner.estimateTitleSizeText(first)).toBeNull();
		runtime.encoding.selectTitle(first, 'intent', 'encode');
		expect(mounted.owner.estimateTitleSizeText(first)).toBeNull();
	});

	it('waits for Auto audio planning before estimating size and previews the selected format', async () => {
		const previewOutputPath = vi
			.spyOn(tauriClient, 'previewOutputPath')
			.mockResolvedValue('/books/out/a.mp3');
		runtime = createAppRuntime();
		const session = sessionWithDuration(100);
		const source = {
			...session.files[0]!,
			path: '/books/a.mp3',
			size: 1_048_576,
			preservation: { canPreserve: true },
		};
		session.files = [source, { ...source, path: '/books/b.m4b', duration: 100 }];
		runtime.input.replaceSession(session);
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
		const validatePatch = vi.spyOn(tauriClient, 'validateMetadataIntentPatch').mockResolvedValue({
			isValid: true,
			metadataPatch: {},
			fieldErrors: [],
		});
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
		validatePatch.mockRestore();
	});

	it('re-reads output path preview when series part changes', async () => {
		const previewOutputPath = vi
			.spyOn(tauriClient, 'previewOutputPath')
			.mockResolvedValue('/books/out/preview.m4b');
		const validatePatch = vi.spyOn(tauriClient, 'validateMetadataIntentPatch').mockResolvedValue({
			isValid: true,
			metadataPatch: {},
			fieldErrors: [],
		});
		runtime = createAppRuntime();
		runtime.input.replaceSession(sessionWithDuration(100));
		let form = createEmptyFormState();
		form = replaceField(form, 'meta-series-part', { value: '1' });
		const [metadataView, setMetadataView] = createSignal<MetadataView>({
			form,
			cover: createEmptyCoverUiState(),
			tags: projectTagPreviewValues(createEmptyFormState(), ''),
			saveInProgress: false,
			focusedFieldId: null,
			statusMessage: '',
		});
		mounted = mountOutput(runtime, { metadataView });
		mounted.owner.applyDefaults({
			outputDirectory: '/books/out',
			outputNaming: { preset: 'absDefault', includeYear: false },
		});
		await vi.waitFor(() => expect(previewOutputPath).toHaveBeenCalled());
		const firstCall = previewOutputPath.mock.calls[previewOutputPath.mock.calls.length - 1];
		expect(firstCall?.[0]?.metadata?.series_part).toBe('1');
		const callsBefore = previewOutputPath.mock.calls.length;
		form = replaceField(metadataView().form, 'meta-series-part', { value: '2' });
		setMetadataView((current) => ({ ...current, form }));
		await vi.waitFor(() =>
			expect(previewOutputPath.mock.calls.length).toBeGreaterThan(callsBefore),
		);
		const lastCall = previewOutputPath.mock.calls[previewOutputPath.mock.calls.length - 1];
		expect(lastCall?.[0]?.metadata?.series_part).toBe('2');
		previewOutputPath.mockRestore();
		validatePatch.mockRestore();
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
		const validatePatch = vi.spyOn(tauriClient, 'validateMetadataIntentPatch').mockResolvedValue({
			isValid: true,
			metadataPatch: {},
			fieldErrors: [],
		});
		runtime = createAppRuntime();
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
		validatePatch.mockRestore();
	});

	it('forwards only the newest metadata preview validation', async () => {
		vi.spyOn(tauriClient, 'previewOutputPath').mockResolvedValue('/books/out/preview.m4b');
		const pending: Array<(isValid: boolean) => void> = [];
		const validatePatch = vi.spyOn(tauriClient, 'validateMetadataIntentPatch').mockImplementation(
			(patch) =>
				new Promise((resolve) =>
					pending.push((isValid) =>
						resolve({
							isValid,
							metadataPatch: patch,
							fieldErrors: isValid
								? []
								: [
										{
											field: 'series_part',
											code: 'series_part_contains_slash',
											message: 'Bad part',
										},
									],
						}),
					),
				),
		);
		const forwarded: boolean[] = [];
		runtime = createAppRuntime();
		const [metadataView, setMetadataView] = createSignal<MetadataView>({
			...emptyMetadataView(),
			form: replaceField(createEmptyFormState(), 'meta-series-part', { value: '7/8' }),
		});
		mounted = mountOutput(runtime, {
			metadataView,
			onMetadataValidation: (validation) => forwarded.push(validation.ok),
		});
		await vi.waitFor(() => expect(pending.length).toBeGreaterThan(0));
		const staleCount = pending.length;
		setMetadataView((current) => ({
			...current,
			form: replaceField(current.form, 'meta-series-part', { value: '2' }),
		}));
		await vi.waitFor(() => expect(pending.length).toBeGreaterThan(staleCount));
		pending[pending.length - 1]!(true);
		for (const resolveStale of pending.slice(0, -1)) resolveStale(false);
		await vi.waitFor(() => expect(forwarded).toContain(true));
		await Promise.resolve();
		expect(forwarded).toEqual([true]);
		validatePatch.mockRestore();
	});

	it('still previews the path when metadata validation fails', async () => {
		const previewOutputPath = vi
			.spyOn(tauriClient, 'previewOutputPath')
			.mockResolvedValue('/books/out/preview.m4b');
		const validatePatch = vi
			.spyOn(tauriClient, 'validateMetadataIntentPatch')
			.mockRejectedValue(new Error('validation transport failed'));
		const errors: string[] = [];
		const errorSpy = vi.spyOn(console, 'error').mockImplementation((...args: unknown[]) => {
			errors.push(String(args[0]));
		});
		runtime = createAppRuntime();
		mounted = mountOutput(runtime);
		const owner = mounted.owner;
		owner.applyDefaults({
			outputDirectory: '/books/out',
			outputNaming: { preset: 'absDefault', includeYear: false },
		});
		await vi.waitFor(() => expect(owner.view().previewText).toBe('/books/out/preview.m4b'));
		expect(errors.some((message) => message.includes('Metadata preview validation failed'))).toBe(
			true,
		);
		previewOutputPath.mockRestore();
		validatePatch.mockRestore();
		errorSpy.mockRestore();
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
		let form = createEmptyFormState();
		form = replaceField(form, 'meta-title', { value: 'Dune' });
		form = replaceField(form, 'meta-author', { value: 'Herbert' });
		const view: MetadataView = {
			form,
			cover: { ...createEmptyCoverUiState(), currentCoverArt: [1, 2, 3] },
			tags: {
				...projectTagPreviewValues(createEmptyFormState(), ''),
				title: 'Dune',
				artist: 'Herbert',
			},
			saveInProgress: false,
			focusedFieldId: null,
			statusMessage: '',
		};
		const draft = previewDraftFromMetadataView(view);
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

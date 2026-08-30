import { createRoot, createSignal, type Accessor } from 'solid-js';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	defaultEncoderSettings,
	type EncodingRequestConfig,
	type ProcessingPreflightPlan,
} from '../../types/audio';
import { createTestAppRuntime } from '../runtime/harness';
import { emptyInputSession } from '../inputSession/types';
import type { InputView } from '../inputSession';
import {
	createOutputOwner,
	readOutputRequestConfig,
	resetOutputPlanTimers,
	type OutputPlanOwner,
} from '.';
import {
	cancelCollisionDialog,
	chooseCollisionPolicy,
	getCollisionView,
	openCollisionDialog,
	resetCollisionDialog,
} from './collision';
import { previewDraftFromMetadataView, sourcePathFromInput } from './previewDraft';
import { createEmptyCoverUiState } from '../metadataSession/cover';
import { createEmptyFormState, replaceField } from '../metadataSession/fields';
import { createEmptyTagPreviewValues } from '../metadataSession/tags';
import type { MetadataDraftValidation, MetadataView } from '../metadataSession';

function sessionWithDuration(totalDuration: number) {
	return {
		...emptyInputSession(),
		fileList: {
			files: [
				{
					path: '/books/a.m4b',
					isValid: true,
					duration: totalDuration,
					size: 1024,
					format: 'm4b',
				},
			],
			selectedDecoders: [null],
			totalDuration,
			totalSize: 1024,
			validCount: 1,
			invalidCount: 0,
		},
	};
}

function emptyInputView(overrides: Partial<InputView> = {}): InputView {
	return {
		files: [],
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
		tags: createEmptyTagPreviewValues(),
		saveInProgress: false,
		focusedFieldId: null,
		statusMessage: '',
	};
}

function defaultEncodingRequest(): EncodingRequestConfig {
	return {
		encoderSettings: defaultEncoderSettings(),
		sampleRate: 'auto',
	};
}

function collisionPlan(): ProcessingPreflightPlan {
	return {
		jobType: 'batch',
		previewSeconds: undefined,
		collisionPolicy: 'fail',
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
	readonly setEncodingRequest: (config: EncodingRequestConfig) => void;
	readonly setEncodingEstimateKbps: (kbps: number) => void;
	dispose(): void;
};

function mountOutput(
	runtime: ReturnType<typeof createTestAppRuntime>,
	overrides: {
		readonly encodingRequest?: EncodingRequestConfig;
		readonly encodingEstimateKbps?: number;
		readonly metadataView?: Accessor<MetadataView>;
		readonly onMetadataValidation?: (validation: MetadataDraftValidation) => void;
	} = {},
): MountedOutput {
	return createRoot((dispose) => {
		const [encodingRequest, setEncodingRequest] = createSignal(
			overrides.encodingRequest ?? defaultEncodingRequest(),
		);
		const [encodingEstimateKbps, setEncodingEstimateKbps] = createSignal(
			overrides.encodingEstimateKbps ?? 64,
		);
		const owner = createOutputOwner({
			input: runtime.input,
			metadataView: overrides.metadataView ?? emptyMetadataView,
			encodingRequest,
			encodingEstimateKbps,
			onMetadataValidation: overrides.onMetadataValidation,
		});
		return {
			owner,
			setEncodingRequest,
			setEncodingEstimateKbps,
			dispose,
		};
	});
}

describe('output plan public view', () => {
	let runtime: ReturnType<typeof createTestAppRuntime> | undefined;
	let mounted: MountedOutput | undefined;

	afterEach(() => {
		vi.useRealTimers();
		mounted?.dispose();
		mounted = undefined;
		runtime?.dispose();
		runtime = undefined;
		resetOutputPlanTimers();
		resetCollisionDialog();
	});

	it('hydrates output defaults through the public strip without a preview poke API', () => {
		runtime = createTestAppRuntime();
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
		expect(readOutputRequestConfig().outputDirectory).toBe('/books/out');
	});

	it('derives the encoder-header estimate from public Input duration and encoder request config', () => {
		runtime = createTestAppRuntime();
		runtime.input.replaceSession(sessionWithDuration(100));
		mounted = mountOutput(runtime, {
			encodingRequest: {
				encoderSettings: {
					...defaultEncoderSettings(),
					bitrateMode: { mode: 'cbr' },
					bitrateKbps: 64,
					channels: 'stereo',
				},
				sampleRate: 'auto',
			},
			encodingEstimateKbps: 64,
		});
		expect(mounted.owner.estimatedSizeText()).toBe('~ 1.2 MB');
	});

	it('changes the encoder-header size when FDK VBR quality changes encoded bitrate', () => {
		runtime = createTestAppRuntime();
		runtime.input.replaceSession(sessionWithDuration(100));
		const stickyBitrateKbps = 64;
		mounted = mountOutput(runtime, {
			encodingRequest: {
				encoderSettings: {
					...defaultEncoderSettings(),
					bitrateMode: { mode: 'vbr', value: 1 },
					bitrateKbps: stickyBitrateKbps,
					channels: 'auto',
				},
				sampleRate: 'auto',
			},
			encodingEstimateKbps: 32,
		});
		const quality1 = mounted.owner.estimatedSizeText();
		mounted.setEncodingRequest({
			encoderSettings: {
				...defaultEncoderSettings(),
				bitrateMode: { mode: 'vbr', value: 5 },
				bitrateKbps: stickyBitrateKbps,
				channels: 'auto',
			},
			sampleRate: 'auto',
		});
		mounted.setEncodingEstimateKbps(96);
		const quality5 = mounted.owner.estimatedSizeText();
		expect(quality1).toBe('~ 402.3 KB');
		expect(quality5).toBe('~ 1.2 MB');
	});

	it('keeps the empty estimate placeholder when Input has no files', () => {
		runtime = createTestAppRuntime();
		mounted = mountOutput(runtime);
		expect(mounted.owner.estimatedSizeText()).toBe('~ --- MB');
	});

	it('commits a typed template only after the 150 ms debounce', () => {
		vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
		runtime = createTestAppRuntime();
		mounted = mountOutput(runtime);
		mounted.owner.applyDefaults({
			outputDirectory: '/books/out',
			outputNaming: {
				preset: 'customTemplate',
				includeYear: false,
				customTemplate: '{old}',
			},
		});
		mounted.owner.selectNamingPreset('customTemplate');
		mounted.owner.editNamingTemplate('{author}/{title}');
		expect(mounted.owner.view().namingTemplate).toBe('{author}/{title}');
		expect(readOutputRequestConfig().outputNaming.customTemplate).toBe('{old}');
		vi.advanceTimersByTime(149);
		expect(readOutputRequestConfig().outputNaming.customTemplate).toBe('{old}');
		vi.advanceTimersByTime(1);
		expect(readOutputRequestConfig().outputNaming.customTemplate).toBe('{author}/{title}');
		vi.useRealTimers();
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
			tags: { ...createEmptyTagPreviewValues(), title: 'Dune', artist: 'Herbert' },
			saveInProgress: false,
			focusedFieldId: null,
			statusMessage: '',
		};
		const draft = previewDraftFromMetadataView(view);
		expect(draft.title).toBe('Dune');
		expect(draft.artist).toBe('Herbert');
		expect(draft.album).toBe('Dune');
		expect(draft.cover_art).toEqual([1, 2, 3]);
	});
});

describe('collision review', () => {
	afterEach(() => {
		resetCollisionDialog();
	});

	it('cancel resolves null and closes the dialog', async () => {
		const result = openCollisionDialog(collisionPlan());
		cancelCollisionDialog();
		await expect(result).resolves.toBeNull();
		expect(getCollisionView().isOpen).toBe(false);
		expect(getCollisionView().outputs).toEqual([]);
	});

	it('opening a second dialog resolves the first as cancelled', async () => {
		const first = openCollisionDialog(collisionPlan());
		const second = openCollisionDialog(collisionPlan());
		await expect(first).resolves.toBeNull();
		chooseCollisionPolicy('rename_new');
		await expect(second).resolves.toBe('rename_new');
		expect(getCollisionView().isOpen).toBe(false);
	});

	it('exposes only collided outputs', () => {
		openCollisionDialog(collisionPlan());
		expect(getCollisionView().outputs).toHaveLength(1);
		expect(getCollisionView().outputs[0]?.inputPath).toBe('/books/b.m4b');
		expect(getCollisionView().body).toBe(
			'1 file with the same name already exists in the target output folder. How do you want to resolve the conflict?',
		);
	});
});

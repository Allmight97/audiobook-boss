/**
 * UI Workflow Smoke Test for the Metadata Session owner (docs/ubiquitous-language.md).
 *
 * Every other metadata test mocks one side of the stage→save handoff; these two
 * scenarios run the REAL metadataSession state so a strip regression cannot hide
 * behind per-owner mocks:
 *   1. edit→save (metadata-only, no encoding): fileList staging feeds the same
 *      pending store that saveMetadataFromUI drains into saveMetadataBatch.
 *   2. lookup-apply→same path: a lookup queue apply stages through the same
 *      seam and drains through the same save.
 *   3. staged intent→output review→WorkRuntime submission: processing drains
 *      the same pending store into both the output-plan review and final
 *      retained processing payload.
 */
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { defaultEncoderSettings, type FileListInfo } from '../../types/audio';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import type { WorkSubmissionAccepted } from '../../types/workRuntime';
import type { ProcessingWorkflowServices } from '../statusPanel/processingWorkflow';
import type { ProcessingStatus } from '../statusPanel/state';

const context = vi.hoisted(() => ({
	saveMetadataBatchMock: vi.fn(),
	validateMetadataIntentPatchMock: vi.fn(),
	readMetadataFormMock: vi.fn(() => ({}) as Record<string, unknown>),
	hasDirtyMetadataFieldsMock: vi.fn(() => true),
	resetDirtyStateMock: vi.fn(),
	getSelectedFilesMock: vi.fn(() => [] as Array<{ path: string; isValid: boolean }>),
	getCurrentFileListMock: vi.fn(() => ({
		files: [] as Array<{ path: string; isValid: boolean }>,
	})),
	statusPanelProcessing: false,
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		saveMetadataBatch: context.saveMetadataBatchMock,
		validateMetadataIntentPatch: context.validateMetadataIntentPatchMock,
		searchOnlineMetadata: vi.fn(),
		loadCoverArtFromUrl: vi.fn(),
		listen: vi.fn(async () => () => {}),
	},
}));

vi.mock('../metadataForm', () => ({
	readMetadataForm: context.readMetadataFormMock,
	hasDirtyMetadataFields: context.hasDirtyMetadataFieldsMock,
	resetDirtyState: context.resetDirtyStateMock,
	applyMetadataToForm: vi.fn(),
	initMetadataFormEvents: vi.fn(),
	onMetadataFormFieldInput: vi.fn(),
	onMetadataFormActionSelectChange: vi.fn(),
}));

vi.mock('../statusPanel/index', () => ({
	initStatusPanel: vi.fn(),
	isStatusPanelProcessing: () => context.statusPanelProcessing,
	pushStatusPanelTransientStatus: (message: string) => {
		const status = document.getElementById('status-text');
		if (status instanceof HTMLElement) {
			status.textContent = message;
		}
	},
}));

vi.mock('../outputPanel', () => ({
	updateOutputPath: vi.fn(),
	updateEstimatedSize: vi.fn(),
}));

vi.mock('../coverArt', () => ({
	clearCoverArt: vi.fn(),
	setCoverArt: vi.fn(),
	setCustomCoverArt: vi.fn(),
	refreshCoverArtDisplay: vi.fn(),
	getCurrentCoverArt: vi.fn(() => null),
}));

vi.mock('../tagPreview', () => ({
	updateTagPreview: vi.fn(),
	initTagPreview: vi.fn(),
	calculateTSOA: vi.fn(() => ''),
	TagPreviewIsland: {},
}));

// Keep the REAL fileList metadata staging chain, but pin its private state and
// selection reads so the scenario stays deterministic.
vi.mock('../fileList/state.svelte', () => ({
	getCurrentFileList: context.getCurrentFileListMock,
	getSelectedFileIndices: vi.fn(() => new Set<number>()),
	getSortAscending: vi.fn(() => true),
	isOrderLocked: vi.fn(() => false),
	onOrderLockChange: vi.fn(() => () => undefined),
	setCurrentFileList: vi.fn(),
}));

vi.mock('../fileList/metadataPanel', () => ({
	getSelectedFiles: context.getSelectedFilesMock,
	ensureMetadataForFiles: vi.fn(async () => undefined),
}));

vi.mock('../fileList', async () => {
	const staging = await vi.importActual<typeof import('../fileList/metadataStaging')>(
		'../fileList/metadataStaging',
	);
	return {
		getCurrentFileList: context.getCurrentFileListMock,
		getSelectedFiles: context.getSelectedFilesMock,
		getSelectedFileIndices: vi.fn(() => new Set<number>()),
		selectFile: vi.fn(async () => undefined),
		persistPendingMetadataDraftsForCurrentSelection:
			staging.persistPendingMetadataDraftsForCurrentSelection,
		stageMetadataToSelection: staging.stageMetadataToSelection,
	};
});

const FILE_A = { path: '/books/edited.m4b', isValid: true };
const FILE_B = { path: '/books/looked-up.m4b', isValid: true };
const FILE_C = { path: '/books/third.m4b', isValid: true };

function smokeFileList(): FileListInfo {
	return {
		files: [
			{
				inputId: 'input-smoke',
				path: FILE_A.path,
				size: 1024,
				duration: 60,
				format: 'm4b',
				bitrate: 64,
				sampleRate: 44100,
				channels: 1,
				codecLabel: 'AAC',
				selectedDecoder: undefined,
				isValid: true,
				error: undefined,
			},
		],
		selectedDecoders: [null],
		totalDuration: 60,
		totalSize: 1024,
		validCount: 1,
		invalidCount: 0,
	};
}

function processingContext() {
	return {
		updateStatus: vi.fn((_status: ProcessingStatus) => undefined),
		setProcessingState: vi.fn(),
		updateArtThumbnail: vi.fn(async () => undefined),
		startProgressListener: vi.fn(async () => undefined),
		setCurrentWorkKind: vi.fn(),
		setBatchCompletionMessage: vi.fn(),
		handleCancellation: vi.fn(),
		resetToIdle: vi.fn(),
	};
}

function acceptedProcessingSubmission(): WorkSubmissionAccepted {
	return {
		operationId: 'operation-smoke',
		snapshot: {
			operationId: 'operation-smoke',
			sequence: 1,
			kind: 'processingMerge',
			status: 'accepted',
			title: 'Merge encode (1 file)',
			createdAtMs: 1,
			startedAtMs: undefined,
			finishedAtMs: undefined,
			cancellable: true,
			cancelRequested: false,
			lanes: ['analysis', 'encodeCpu', 'outputCommit'],
			sourceInputIds: ['input-smoke'],
			progress: {
				stage: 'pending',
				percentage: 0,
				message: 'Accepted.',
				currentItemIndex: undefined,
				totalItems: 1,
				bytesDownloaded: undefined,
				bytesTotal: undefined,
				etaSeconds: undefined,
			},
			children: [],
			terminalSummary: undefined,
			warnings: [],
			errors: [],
		},
	};
}

describe('metadata session smoke (edit→save and lookup→save through one seam)', () => {
	let metadataSession: typeof import('../metadataSession');

	beforeAll(async () => {
		document.body.innerHTML = '<div id="status-text">Idle</div>';
		metadataSession = await import('../metadataSession');
	});

	beforeEach(() => {
		metadataSession.clearMetadataSession();
		context.saveMetadataBatchMock.mockReset();
		context.validateMetadataIntentPatchMock.mockReset();
		context.validateMetadataIntentPatchMock.mockImplementation(async (patch: unknown) => ({
			metadataPatch: patch,
			fieldErrors: [],
		}));
		context.readMetadataFormMock.mockReset();
		context.hasDirtyMetadataFieldsMock.mockReset();
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		context.resetDirtyStateMock.mockReset();
		context.getSelectedFilesMock.mockReset();
		context.getCurrentFileListMock.mockReset();
		context.statusPanelProcessing = false;
	});

	it('persists a form edit through staging and drains it via saveMetadataFromUI', async () => {
		context.getCurrentFileListMock.mockReturnValue({ files: [FILE_A] });
		context.getSelectedFilesMock.mockReturnValue([FILE_A]);
		context.readMetadataFormMock.mockReturnValue({ title: 'Edited Title' });
		context.saveMetadataBatchMock.mockResolvedValue({
			summary: { total: 1, succeeded: 1, skipped: 0, cancelled: 0, failed: 0 },
			results: [{ inputIndex: 0, filePath: FILE_A.path, status: 'success', message: 'saved' }],
		});
		metadataSession.cacheMetadataForFile(FILE_A.path, {
			title: 'Old Title',
			album_sort: 'Artifact Keep',
		});

		const { persistPendingMetadataDraftsForCurrentSelection } = await import('../fileList');
		await expect(persistPendingMetadataDraftsForCurrentSelection()).resolves.toBe(true);

		// The edit landed in the session's pending truth, artifact fields untouched.
		expect(metadataSession.collectActionableMetadataIntent([FILE_A.path])).toEqual({
			[FILE_A.path]: { title: { op: 'set', value: 'Edited Title' } },
		});
		expect(metadataSession.getMetadataForFile(FILE_A.path)?.album_sort).toBe('Artifact Keep');

		await metadataSession.saveMetadataFromUI();

		expect(context.saveMetadataBatchMock).toHaveBeenCalledTimes(1);
		expect(context.saveMetadataBatchMock).toHaveBeenCalledWith([
			{
				filePath: FILE_A.path,
				metadataPatch: { title: { op: 'set', value: 'Edited Title' } },
			},
		]);
		// Success drained the pending marker and reset the form dirty state.
		expect(metadataSession.collectActionableMetadataIntent([FILE_A.path])).toBeNull();
		expect(context.resetDirtyStateMock).toHaveBeenCalled();
	});

	it('applies a lookup result through the same staging seam and the same save drain', async () => {
		context.getCurrentFileListMock.mockReturnValue({ files: [FILE_B] });
		context.getSelectedFilesMock.mockReturnValue([]);
		context.saveMetadataBatchMock.mockResolvedValue({
			summary: { total: 1, succeeded: 1, skipped: 0, cancelled: 0, failed: 0 },
			results: [{ inputIndex: 0, filePath: FILE_B.path, status: 'success', message: 'saved' }],
		});

		const { makeMetadataLookupWorkflowServicesLayer, runMetadataLookupWorkflow } = await import(
			'../metadataLookup/metadataLookupWorkflow'
		);

		const lookupResult = {
			title: 'Looked Up Title',
			authors: ['Author Person'],
			narrators: [],
			series: null,
			seriesPart: null,
			subseries: null,
			subseriesPart: null,
			description: null,
			publishedDate: null,
			coverUrl: null,
			source: 'audnexus',
		};
		const lookupState = {
			isOpen: true,
			titleQuery: 'looked up',
			authorQuery: '',
			source: 'auto' as const,
			applyMode: 'queue' as const,
			replaceCoverArt: false,
			statusMessage: '',
			statusVariant: 'info' as const,
			queueContext: '',
			results: [lookupResult],
			isQueueMode: true,
			skipEnabled: true,
			hasSearched: true,
		};
		const queueState = { queue: [{ file: FILE_B, index: 0 }], index: 0 };

		const layer = makeMetadataLookupWorkflowServicesLayer({
			getLookupState: () => lookupState,
			getQueueState: () => queueState,
			setMetadataLookupQueue: vi.fn(),
			clearMetadataLookupQueue: vi.fn(),
			setMetadataLookupQueueIndex: vi.fn(),
			getSelectedFileIndices: () => new Set([0]),
			getCurrentFileList: () => ({ files: [FILE_B] }) as never,
			// The seam under test: the REAL session functions, not fakes.
			getMetadataForFile: metadataSession.getMetadataForFile,
			stageMetadataIntentPatch: metadataSession.stageMetadataIntentPatch,
			selectFile: vi.fn(async () => undefined),
			applyMetadataToForm: vi.fn(),
			readMetadataForm: vi.fn(() => ({
				title: 'Looked Up Title',
				artist: 'Author Person',
				album: 'Looked Up Title',
			})),
			updateOutputPath: vi.fn(),
			updateEstimatedSize: vi.fn(),
			updateTagPreview: vi.fn(),
			clearCoverArt: vi.fn(),
			setCoverArt: vi.fn(),
			setCustomCoverArt: vi.fn(),
			refreshCoverArtDisplay: vi.fn(),
			searchOnlineMetadata: vi.fn(),
			loadCoverArtFromUrl: vi.fn(),
			focusElementById: vi.fn(),
			queueMicrotask: (callback: () => void) => callback(),
			console: { error: vi.fn(), warn: vi.fn() },
		} as never);

		await runMetadataLookupWorkflow(layer, { type: 'applyResult', index: 0 });

		// The lookup apply landed in the SAME pending store the primary path uses.
		expect(
			metadataSession.collectActionableMetadataIntent([FILE_B.path])?.[FILE_B.path],
		).toMatchObject({
			title: { op: 'set', value: 'Looked Up Title' },
			artist: { op: 'set', value: 'Author Person' },
		});

		await metadataSession.saveMetadataFromUI();

		expect(context.saveMetadataBatchMock).toHaveBeenCalledTimes(1);
		const [requests] = context.saveMetadataBatchMock.mock.calls[0] as [
			Array<{ filePath: string; metadataPatch: Record<string, unknown> }>,
		];
		expect(requests).toHaveLength(1);
		expect(requests[0].filePath).toBe(FILE_B.path);
		expect(requests[0].metadataPatch).toMatchObject({
			title: { op: 'set', value: 'Looked Up Title' },
		});
		expect(metadataSession.collectActionableMetadataIntent([FILE_B.path])).toBeNull();
	});

	it('saves shared multi-file fields and explicit blanks as one batch payload per selected file', async () => {
		const files = [FILE_A, FILE_B, FILE_C];
		context.getCurrentFileListMock.mockReturnValue({ files });
		context.getSelectedFilesMock.mockReturnValue(files);
		context.readMetadataFormMock.mockReturnValue({ title: 'Shared title', genre: '' });
		context.saveMetadataBatchMock.mockResolvedValue({
			summary: { total: 3, succeeded: 3, skipped: 0, cancelled: 0, failed: 0 },
			results: files.map((file, inputIndex) => ({
				inputIndex,
				filePath: file.path,
				status: 'success',
				message: 'saved',
			})),
		});

		const { stageMetadataToSelection } = await import('../fileList');
		await expect(stageMetadataToSelection()).resolves.toBe(true);
		await metadataSession.saveMetadataFromUI();

		expect(context.saveMetadataBatchMock).toHaveBeenCalledWith(
			files.map((file) => ({
				filePath: file.path,
				metadataPatch: {
					title: { op: 'set', value: 'Shared title' },
					genre: { op: 'clear' },
				},
			})),
		);
	});

	it('drains staged metadata into output review and retained processing submission', async () => {
		const stagedIntent: MetadataIntentPatch = {
			title: { op: 'set', value: 'Submitted Title' },
			series: { op: 'set', value: 'Submitted Series' },
			series_part: { op: 'set', value: '3' },
		};
		expect(metadataSession.stageMetadataIntentPatch(FILE_A.path, stagedIntent)).toBe('staged');
		const expectedIntent = {
			[FILE_A.path]: stagedIntent,
		};

		const { makeProcessingWorkflowServicesLayer, startProcessing } = await import(
			'../statusPanel/processingWorkflow'
		);
		const reviewOutputPlan: ProcessingWorkflowServices['runOutputPlanReviewWorkflow'] = vi.fn(
			async ({ payload, metadataIntentByPath, previewSeconds }) => ({
				status: 'approved' as const,
				payload: { ...payload, preflightSignature: 'smoke-preflight' },
				plan: {
					jobType: payload.jobType ?? 'merge',
					previewSeconds: previewSeconds ?? null,
					collisionPolicy: payload.collisionPolicy ?? 'fail',
					planSignature: metadataIntentByPath ? 'smoke-preflight' : 'missing-intent',
					outputs: [
						{
							inputIndex: 0,
							inputPath: FILE_A.path,
							kind: 'final' as const,
							requestedPath: '/tmp/out/Submitted Title.m4b',
							resolvedPath: '/tmp/out/Submitted Title.m4b',
							renameCandidate: undefined,
							collision: undefined,
							action: 'write' as const,
						},
					],
				},
			}),
		);
		const submitProcessingOperation: ProcessingWorkflowServices['submitProcessingOperation'] =
			vi.fn(async () => acceptedProcessingSubmission());

		const layer = makeProcessingWorkflowServicesLayer({
			updateOutputPath: vi.fn(),
			getCurrentFileList: () => smokeFileList(),
			getSelectedFileIndex: () => 0,
			getSelectedFileIndices: () => new Set([0]),
			readProcessingRequestConfig: () => ({
				encoderSettings: defaultEncoderSettings(),
				sampleRate: 'auto',
				outputDirectory: '/tmp/out',
				outputNaming: {
					preset: 'absDefault',
					includeYear: false,
					customTemplate: undefined,
				},
			}),
			getJobType: () => 'merge',
			hasDirtyMetadataFields: () => false,
			readMetadataForm: vi.fn(() => ({})),
			collectActionableMetadataIntent: metadataSession.collectActionableMetadataIntent,
			getMetadataForFile: metadataSession.getMetadataForFile,
			cacheMetadataForFile: metadataSession.cacheMetadataForFile,
			stageMetadataIntentPatch: metadataSession.stageMetadataIntentPatch,
			stageMetadataToSelection: vi.fn(async () => true),
			setJobControlsEnabled: vi.fn(),
			setFileOrderLocked: vi.fn(),
			validateMetadataIntentPatch: vi.fn(async (metadataPatch) => ({
				isValid: true,
				metadataPatch,
				fieldErrors: [],
			})),
			readAudioMetadata: vi.fn(async () => ({})),
			processAudiobookFiles: vi.fn(),
			submitProcessingOperation,
			runOutputPlanReviewWorkflow: reviewOutputPlan,
			openGeneratedPreviewIfSingle: vi.fn(async () => undefined),
			feedback: { showError: vi.fn() },
			console: { error: vi.fn(), log: vi.fn(), warn: vi.fn() },
		});

		await startProcessing(processingContext(), undefined, layer);

		expect(reviewOutputPlan).toHaveBeenCalledWith(
			expect.objectContaining({
				metadataIntentByPath: expectedIntent,
			}),
		);
		expect(submitProcessingOperation).toHaveBeenCalledWith(
			expect.objectContaining({
				payload: expect.objectContaining({
					preflightSignature: 'smoke-preflight',
				}),
				metadataIntent: expectedIntent,
			}),
		);
	});
});

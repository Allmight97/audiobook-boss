import { titleAudioRequest } from '../../../test/fixtures/titleAudio';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { StatusPanelRuntime } from '../runtime';
import { createAppRuntime } from '../../runtime';
import { createFakeEngine } from '../../../test/fixtures/fakeEngine';
import { tauriClient } from '../../../lib/tauri/client';
import { createStatusViewStore } from '../view';
import {
	makeProcessingWorkflowServicesLayer,
	startProcessing,
	type ProcessingWorkflowContext,
	type ProcessingWorkflowServices,
} from '../workflow';
import type {
	AudioFile,
	ProcessCommandResult,
	ProcessingPreflightPlan,
	ProcessingRequestConfig,
	ProcessPayload,
} from '../../../types/audio';
import type { WorkSubmissionAccepted } from '../../../types/workRuntime';
import type { OutputPlanReviewResult } from '../../outputPlan';

function audioFile(path: string, overrides: Partial<AudioFile> = {}): AudioFile {
	return {
		path,
		size: 1,
		duration: 1,
		format: 'm4b' as const,
		bitrate: undefined,
		sampleRate: undefined,
		channels: undefined,
		codecLabel: undefined,
		selectedDecoder: undefined,
		isValid: true,
		error: undefined,
		...overrides,
	};
}

function titles(paths = ['/books/a.m4b']): AudioFile[] {
	return paths.map((path, index) => audioFile(path, { inputId: `input-${index + 1}` }));
}

function processingConfig(): ProcessingRequestConfig {
	return {
		audioRequests: [titleAudioRequest()],
		outputDirectory: '/tmp/out',
		outputNaming: { preset: 'absDefault', includeYear: false, customTemplate: undefined },
	};
}

function preflightPlan(payload: ProcessPayload): ProcessingPreflightPlan {
	return {
		previewSeconds: undefined,
		collisionPolicy: payload.collisionPolicy ?? 'fail',
		audioPlans: [],
		planSignature: 'preflight-approved',
		outputs: payload.inputFiles.map((inputPath, inputIndex) => ({
			inputIndex,
			inputPath,
			kind: 'final',
			requestedPath: `/tmp/out/${inputIndex}.m4b`,
			resolvedPath: `/tmp/out/${inputIndex}.m4b`,
			renameCandidate: undefined,
			collision: undefined,
			action: 'write',
			review: undefined,
		})),
	};
}

function successResult(): ProcessCommandResult {
	return {
		summary: { total: 1, succeeded: 1, skipped: 0, cancelled: 0, failed: 0 },
		terminalClass: 'success',
		results: [
			{
				inputIndex: 0,
				status: 'success',
				message: 'ok',
				jobId: 'job-1',
				error: undefined,
				outputPath: undefined,
				previewActualSeconds: undefined,
			},
		],
	};
}

function acceptedSubmission(): WorkSubmissionAccepted {
	return {
		operationId: 'operation-1',
		snapshot: {
			operationId: 'operation-1',
			sequence: 1,
			revision: 1,
			createdRevision: 1,
			kind: 'processingBatch',
			status: 'accepted',
			title: 'a.m4b',
			createdAtMs: 1,
			startedAtMs: undefined,
			finishedAtMs: undefined,
			cancellable: true,
			cancelRequested: false,
			lanes: ['analysis', 'encodeCpu', 'outputCommit'],
			sourceInputIds: ['current-input-1'],
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
			errors: [],
			logTail: [],
		},
	};
}

function workflowContext(): ProcessingWorkflowContext {
	return {
		updateStatus: vi.fn(),
		setProcessingState: vi.fn(),
		updateArtThumbnail: vi.fn(async () => undefined),
		startProgressListener: vi.fn(async () => undefined),
		setBatchCompletionMessage: vi.fn(),
		reconcileProcessResult: vi.fn(),
		handleCancellation: vi.fn(),
		resetToIdle: vi.fn(),
	};
}

function workflowServices(overrides: Partial<ProcessingWorkflowServices> = {}) {
	const feedback = { showError: vi.fn() };
	const runOutputPlanReviewWorkflowMock: ProcessingWorkflowServices['runOutputPlanReviewWorkflow'] =
		vi.fn(
			async ({ payload }): Promise<OutputPlanReviewResult> => ({
				status: 'approved',
				payload: { ...payload, preflightSignature: 'preflight-approved' },
				plan: preflightPlan(payload),
			}),
		);
	const remoteSource: ProcessingWorkflowServices['remoteSource'] = {
		processingAssets: vi.fn(() => undefined),
		withSubmissionRetention: vi.fn(async (_inputIds, submit) => submit()),
	};
	const services: ProcessingWorkflowServices = {
		currentTitles: vi.fn(() => titles()),
		sourcesFor: (file) => [file],
		readProcessingRequestConfig: vi.fn((titles: readonly AudioFile[]) => ({
			...processingConfig(),
			audioRequests: titles.map(() => titleAudioRequest()),
		})),
		stageMetadata: vi.fn(async () => ({ status: 'staged' as const })),
		intentsForProcess: vi.fn(async () => null),
		setJobControlsEnabled: vi.fn(),
		setFileOrderLocked: vi.fn(),
		processAudiobookFiles: vi.fn(async () => successResult()),
		submitProcessingOperation: vi.fn(async () => acceptedSubmission()),
		remoteSource,
		runOutputPlanReviewWorkflow: runOutputPlanReviewWorkflowMock,
		openGeneratedPreviewIfSingle: vi.fn(async () => undefined),
		feedback,
		console: {
			error: vi.fn(),
			log: vi.fn(),
			warn: vi.fn(),
		},
		...overrides,
	};
	return { services, feedback };
}

async function runWithServices(
	context: ProcessingWorkflowContext,
	services: ProcessingWorkflowServices,
): Promise<void> {
	await startProcessing(context, undefined, makeProcessingWorkflowServicesLayer(services));
}

describe('ProcessingWorkflow', () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it.each([undefined, 30])(
		'reports an unavailable request before preparation or submission (preview: %s)',
		async (previewSeconds) => {
			const ctx = workflowContext();
			const { services, feedback } = workflowServices({
				readProcessingRequestConfig: () => {
					throw new Error('Encoder availability is not ready.');
				},
				currentTitles: () => titles(['/books/a.m4b', '/books/b.m4b']),
			});
			await startProcessing(ctx, { previewSeconds }, makeProcessingWorkflowServicesLayer(services));
			expect(feedback.showError).toHaveBeenCalledWith(
				expect.stringContaining('Encoder availability is not ready.'),
			);
			expect(services.stageMetadata).not.toHaveBeenCalled();
			expect(services.runOutputPlanReviewWorkflow).not.toHaveBeenCalled();
			expect(ctx.setProcessingState).not.toHaveBeenCalled();
			expect(services.processAudiobookFiles).not.toHaveBeenCalled();
			expect(services.submitProcessingOperation).not.toHaveBeenCalled();
		},
	);

	it('coordinates approved processing through injected services without changing the public runtime API', async () => {
		const ctx = workflowContext();
		const { services } = workflowServices();

		await runWithServices(ctx, services);

		expect(services.runOutputPlanReviewWorkflow).toHaveBeenCalledTimes(1);
		expect(ctx.setProcessingState).toHaveBeenCalledWith(true);
		expect(ctx.updateStatus).toHaveBeenCalledWith({
			stage: 'analyzing',
			percentage: 0,
			message: 'Starting processing...',
		});
		expect(services.setJobControlsEnabled).toHaveBeenCalledWith(false);
		expect(services.setFileOrderLocked).toHaveBeenCalledWith(true);
		expect(ctx.updateArtThumbnail).toHaveBeenCalledTimes(1);
		expect(ctx.startProgressListener).not.toHaveBeenCalled();
		expect(services.submitProcessingOperation).toHaveBeenCalledWith({
			payload: expect.objectContaining({
				inputFiles: ['/books/a.m4b'],
				preflightSignature: 'preflight-approved',
			}),
			metadataIntent: null,
			title: 'a.m4b',
		});
		expect(services.remoteSource.withSubmissionRetention).toHaveBeenCalledWith(
			['input-1'],
			expect.any(Function),
		);
		expect(services.processAudiobookFiles).not.toHaveBeenCalled();
		expect(ctx.reconcileProcessResult).not.toHaveBeenCalled();
		expect(ctx.setProcessingState).toHaveBeenLastCalledWith(false);
		expect(services.setJobControlsEnabled).toHaveBeenLastCalledWith(true);
		expect(services.setFileOrderLocked).toHaveBeenLastCalledWith(false);
		expect(ctx.setBatchCompletionMessage).toHaveBeenLastCalledWith(null);
	});

	it('submits files in the current file-list order', async () => {
		const ctx = workflowContext();
		const { services } = workflowServices({
			currentTitles: vi.fn(() => [
				audioFile('/books/2 - Early Chapter.mp3', { inputId: 'second' }),
				audioFile('/books/10 - Last Chapter.mp3', { inputId: 'tenth' }),
			]),
		});
		await runWithServices(ctx, services);

		expect(services.submitProcessingOperation).toHaveBeenCalledWith(
			expect.objectContaining({
				payload: expect.objectContaining({
					inputFiles: ['/books/2 - Early Chapter.mp3', '/books/10 - Last Chapter.mp3'],
					inputIds: ['second', 'tenth'],
				}),
			}),
		);
	});

	it('submits a stack and a separate title while retaining every ordered source', async () => {
		const anchor = audioFile('/books/one.m4b', { inputId: 'one' });
		const second = audioFile('/books/two.m4b', { inputId: 'two' });
		const separate = audioFile('/books/other.m4b', { inputId: 'other' });
		const { services } = workflowServices({
			currentTitles: () => [anchor, separate],
			sourcesFor: (file) => (file.path === anchor.path ? [second, anchor] : [file]),
			readProcessingRequestConfig: (titles) => ({
				...processingConfig(),
				audioRequests: titles.map((file) =>
					titleAudioRequest({ intent: file.path === anchor.path ? 'auto' : 'encode' }),
				),
			}),
		});
		await runWithServices(workflowContext(), services);
		expect(services.submitProcessingOperation).toHaveBeenCalledWith(
			expect.objectContaining({
				payload: expect.objectContaining({
					inputFiles: [anchor.path, separate.path],
					inputIds: ['one', 'other'],
					audioRequests: [titleAudioRequest(), titleAudioRequest({ intent: 'encode' })],
					titleSources: {
						[anchor.path]: [
							{ path: second.path, inputId: 'two' },
							{ path: anchor.path, inputId: 'one' },
						],
					},
				}),
			}),
		);
		expect(services.intentsForProcess).toHaveBeenCalledWith([anchor.path, separate.path]);
		expect(services.remoteSource.withSubmissionRetention).toHaveBeenCalledWith(
			['two', 'one', 'other'],
			expect.any(Function),
		);
	});

	it('names the Work Center operation after its books, preferring staged titles', async () => {
		const tidy = audioFile('/books/tidy.m4b', { inputId: 'tidy', tagTitle: 'Tidy First' });
		const other = audioFile('/books/other.m4b', { inputId: 'other', tagTitle: 'Other' });
		const single = workflowServices({ currentTitles: () => [tidy] }).services;
		const renamed = workflowServices({
			currentTitles: () => [tidy, other],
			intentsForProcess: vi.fn(async () => ({
				[tidy.path]: { title: { op: 'set' as const, value: 'Tidy First, 2nd Ed.' } },
			})),
		}).services;

		await runWithServices(workflowContext(), single);
		await runWithServices(workflowContext(), renamed);

		expect(single.submitProcessingOperation).toHaveBeenCalledWith(
			expect.objectContaining({ title: 'Tidy First' }),
		);
		expect(renamed.submitProcessingOperation).toHaveBeenCalledWith(
			expect.objectContaining({ title: 'Tidy First, 2nd Ed. + 1 more' }),
		);
	});

	it('passes acquired supplemental PDF assets into processing payload by FileList input id', async () => {
		const supplementalAssetsByInputId = {
			'current-input-1': [
				{
					assetId: 'pdf-1',
					inputId: 'current-input-1',
					titleId: 'B000000001',
					path: '/session/book.pdf',
					fileName: 'Being You - A New Science of Consciousness - Supplemental PDF.pdf',
					sizeBytes: 32,
					sha256: 'pdf-sha',
				},
			],
		};
		const ctx = workflowContext();
		const { services } = workflowServices({
			currentTitles: vi.fn(() => [audioFile('/session/book.m4b', { inputId: 'current-input-1' })]),
			remoteSource: {
				processingAssets: vi.fn(() => supplementalAssetsByInputId),
				withSubmissionRetention: vi.fn(async (_inputIds, submit) => submit()),
			},
		});

		await runWithServices(ctx, services);

		expect(services.submitProcessingOperation).toHaveBeenCalledWith({
			payload: expect.objectContaining({
				inputFiles: ['/session/book.m4b'],
				inputIds: ['current-input-1'],
				supplementalAssetsByInputId: {
					'current-input-1': [
						{
							assetId: 'pdf-1',
							inputId: 'current-input-1',
							titleId: 'B000000001',
							path: '/session/book.pdf',
							fileName: 'Being You - A New Science of Consciousness - Supplemental PDF.pdf',
							sizeBytes: 32,
							sha256: 'pdf-sha',
						},
					],
				},
			}),
			metadataIntent: null,
			title: 'book.m4b',
		});
	});

	it('stops before listener startup when output-plan review blocks processing', async () => {
		const ctx = workflowContext();
		const { services, feedback } = workflowServices({
			runOutputPlanReviewWorkflow: vi.fn(
				async ({ payload }): Promise<OutputPlanReviewResult> => ({
					status: 'blocked',
					message: 'Output path collides with source.',
					plan: preflightPlan(payload),
				}),
			),
		});

		await runWithServices(ctx, services);

		expect(feedback.showError).toHaveBeenCalledWith('Output path collides with source.');
		expect(ctx.setProcessingState).not.toHaveBeenCalled();
		expect(ctx.startProgressListener).not.toHaveBeenCalled();
		expect(services.processAudiobookFiles).not.toHaveBeenCalled();
		expect(services.submitProcessingOperation).not.toHaveBeenCalled();
	});

	it('routes structured cancellation failures to cancellation handling instead of error reset', async () => {
		const ctx = workflowContext();
		const { services, feedback } = workflowServices({
			submitProcessingOperation: vi.fn(async () => {
				throw {
					code: 'cancelled',
					category: 'cancellation',
					message: 'Processing was cancelled.',
					detail: 'user requested stop',
				};
			}),
		});

		await runWithServices(ctx, services);

		expect(ctx.handleCancellation).toHaveBeenCalledTimes(1);
		expect(ctx.resetToIdle).not.toHaveBeenCalled();
		expect(feedback.showError).not.toHaveBeenCalled();
	});

	it.each(['preflight', 'submission'] as const)(
		'keeps a %s rejection visible after returning the real status runtime to idle',
		async (failureStage) => {
			const view = createStatusViewStore();
			const message =
				'FAAC HE-AAC requires a supported output sample rate. Choose 32000, 44100, or 48000 Hz.';
			const reject = async () => {
				throw { code: 'invalid_input', category: 'validation', message };
			};
			const { services } = workflowServices({
				feedback: { showError: (text) => view.showError(text) },
				...(failureStage === 'preflight'
					? { runOutputPlanReviewWorkflow: vi.fn(reject) }
					: { submitProcessingOperation: vi.fn(reject) }),
			});
			const unlockWorkbench = vi.fn();
			const runtime = new StatusPanelRuntime({
				view,
				unlockWorkbench,
				workflowLayer: makeProcessingWorkflowServicesLayer(services),
			});

			await runtime.startProcessing();

			expect(view.snapshot().stepText).toBe(`Error: Processing failed: ${message}`);
			expect(view.snapshot().isProcessing).toBe(false);
			expect(unlockWorkbench).toHaveBeenCalled();
			if (failureStage === 'preflight') {
				expect(services.submitProcessingOperation).not.toHaveBeenCalled();
			}
		},
	);

	it('submits background processing inside Remote Source retention', async () => {
		let allowSubmission!: () => void;
		const gate = new Promise<void>((resolve) => {
			allowSubmission = resolve;
		});
		let retentionActive = false;
		const withSubmissionRetention: ProcessingWorkflowServices['remoteSource']['withSubmissionRetention'] =
			vi.fn(async (_inputIds, submit) => {
				await gate;
				retentionActive = true;
				try {
					return await submit();
				} finally {
					retentionActive = false;
				}
			});
		const submittedWhileRetained: boolean[] = [];
		const { services } = workflowServices({
			remoteSource: {
				processingAssets: vi.fn(() => undefined),
				withSubmissionRetention,
			},
			submitProcessingOperation: vi.fn(async () => {
				submittedWhileRetained.push(retentionActive);
				return acceptedSubmission();
			}),
		});
		const pending = runWithServices(workflowContext(), services);
		await vi.waitFor(() => expect(withSubmissionRetention).toHaveBeenCalled());
		expect(services.submitProcessingOperation).not.toHaveBeenCalled();
		allowSubmission();
		await pending;
		expect(withSubmissionRetention).toHaveBeenCalledWith(['input-1'], expect.any(Function));
		expect(submittedWhileRetained).toEqual([true]);
	});
});

describe('mixed preserved and encoded books', () => {
	it('submits each valid book with its chosen route and metadata through one reviewed output plan', async () => {
		const books = titles([
			'/books/prey1.m4b',
			'/books/prey2.m4b',
			'/books/prey3.m4b',
			'/books/large.m4a',
			'/books/standard.mp3',
		]);
		books.splice(2, 0, audioFile('/books/broken.m4b', { isValid: false, inputId: 'invalid' }));
		const patches = Object.fromEntries(
			books
				.filter((file) => file.isValid)
				.map((file, index) => [
					file.path,
					{ title: { op: 'set' as const, value: `Library title ${index + 1}` } },
				]),
		);
		const { services } = workflowServices({
			currentTitles: () => books,
			readProcessingRequestConfig: vi.fn((titles: readonly AudioFile[]) => ({
				...processingConfig(),
				audioRequests: titles.map((file) =>
					titleAudioRequest({
						intent: ['input-1', 'input-2', 'input-3'].includes(file.inputId ?? '')
							? 'auto'
							: 'encode',
					}),
				),
			})),
			intentsForProcess: vi.fn(async () => patches),
		});
		await runWithServices(workflowContext(), services);
		expect(services.readProcessingRequestConfig).toHaveBeenCalledWith(
			books.filter((file) => file.isValid),
		);
		expect(services.submitProcessingOperation).toHaveBeenCalledWith({
			payload: expect.objectContaining({
				inputFiles: [
					'/books/prey1.m4b',
					'/books/prey2.m4b',
					'/books/prey3.m4b',
					'/books/large.m4a',
					'/books/standard.mp3',
				],
				inputIds: ['input-1', 'input-2', 'input-3', 'input-4', 'input-5'],
				audioRequests: [
					titleAudioRequest(),
					titleAudioRequest(),
					titleAudioRequest(),
					titleAudioRequest({ intent: 'encode' }),
					titleAudioRequest({ intent: 'encode' }),
				],
				outputDir: '/tmp/out',
				preflightSignature: 'preflight-approved',
			}),
			metadataIntent: patches,
			title: 'Library title 1 + 4 more',
		});
	});
});

it('submits MP3 pass-through with no encoder settings', async () => {
	const books = titles(['/books/original.mp3']);
	books[0]!.preservation = { canPreserve: true };
	const preflight = vi
		.spyOn(tauriClient, 'preflightProcessingPlan')
		.mockImplementation(async ({ payload }) => preflightPlan(payload));
	const submit = vi
		.spyOn(tauriClient, 'submitProcessingOperation')
		.mockResolvedValue(acceptedSubmission());
	const readMetadata = vi
		.spyOn(tauriClient, 'readAudioMetadata')
		.mockResolvedValue({ title: 'Original' });
	const engine = createFakeEngine();
	const runtime = createAppRuntime({ engine });

	try {
		engine.loadTitles(books);
		await runtime.initialize();
		runtime.input.setAudioRequest(books[0]!, titleAudioRequest({ format: 'mp3', settings: null }));
		runtime.output.applyDefaults({
			outputDirectory: '/tmp/out',
			outputNaming: { preset: 'absDefault', includeYear: false },
		});
		await runtime.processing.start();
		expect(submit).toHaveBeenCalledWith(
			expect.objectContaining({
				payload: expect.objectContaining({
					audioRequests: [titleAudioRequest({ format: 'mp3', settings: null })],
				}),
			}),
		);
	} finally {
		runtime.dispose();
		readMetadata.mockRestore();
		preflight.mockRestore();
		submit.mockRestore();
	}
});

import { describe, expect, it, vi } from 'vitest';
import { Effect, runAppEffect } from '../../lib/effect/appEffect';
import {
	defaultEncoderSettings,
	type ProcessPayload,
	type ProcessingPreflightPlan,
} from '../../types/audio';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import {
	makeOutputPlanWorkflowServicesLayer,
	outputPathPreviewBody,
	outputPlanReviewBody,
	type OutputPlanWorkflowServices,
} from './workflow';
import { EMPTY_PREVIEW_TEXT, EMPTY_PREVIEW_TITLE } from './types';

function payload(overrides: Partial<ProcessPayload> = {}): ProcessPayload {
	return {
		inputFiles: ['/books/a.m4b'],
		outputDir: '/tmp/out',
		settings: defaultEncoderSettings(),
		sampleRate: 'auto',
		jobType: 'merge',
		outputNaming: { preset: 'absDefault', includeYear: false, customTemplate: undefined },
		...overrides,
	};
}

function plan(overrides: Partial<ProcessingPreflightPlan> = {}): ProcessingPreflightPlan {
	return {
		jobType: 'merge',
		previewSeconds: undefined,
		collisionPolicy: 'fail',
		planSignature: 'sig-clean',
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
		],
		...overrides,
	};
}

function makeHarness(overrides: Partial<OutputPlanWorkflowServices> = {}) {
	const preview = { text: '', title: '' };
	let latestRequestId = 0;
	const services: OutputPlanWorkflowServices = {
		updateMetadataIntentWarnings: vi.fn(async () => undefined),
		beginOutputPreviewRequest: vi.fn(() => {
			latestRequestId += 1;
			return latestRequestId;
		}),
		isLatestOutputPreviewRequest: vi.fn((requestId) => requestId === latestRequestId),
		setOutputPreview: vi.fn((text: string, title = text) => {
			preview.text = text;
			preview.title = title;
		}),
		showOutputError: vi.fn(),
		previewOutputPath: vi.fn(async () => '/tmp/out/a.m4b'),
		preflightProcessingPlan: vi.fn(async () => plan()),
		openCollisionDialog: vi.fn(async () => null),
		console: { error: vi.fn() },
		...overrides,
	};

	return {
		preview,
		services,
		layer: makeOutputPlanWorkflowServicesLayer(services),
	};
}

const previewContext = {
	outputDirectory: '/tmp/out',
	sourcePath: '/books/a.m4b',
	outputNaming: { preset: 'absDefault' as const, includeYear: false, customTemplate: undefined },
	metadataDraft: {
		title: 'A',
		album: 'A',
		artist: 'Author',
		composer: '',
		genre: '',
		description: '',
		series: '',
		subseries: '',
	},
};

describe('OutputPlanWorkflow', () => {
	it('updates output preview from the output artifact boundary', async () => {
		const harness = makeHarness();

		await runAppEffect(
			outputPathPreviewBody('final', previewContext).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.previewOutputPath).toHaveBeenCalledWith(
			expect.objectContaining({
				outputDir: '/tmp/out',
				sourcePath: '/books/a.m4b',
				outputKind: 'final',
			}),
		);
		expect(harness.preview.text).toBe('/tmp/out/a.m4b');
	});

	it('reports missing output directory without crossing the boundary', async () => {
		const harness = makeHarness();

		await runAppEffect(
			outputPathPreviewBody('final', { ...previewContext, outputDirectory: '' }).pipe(
				Effect.provide(harness.layer),
			),
		);

		expect(harness.services.previewOutputPath).not.toHaveBeenCalled();
		expect(harness.preview.text).toBe(EMPTY_PREVIEW_TEXT);
		expect(harness.preview.title).toBe(EMPTY_PREVIEW_TITLE);
	});

	it('ignores stale preview responses', async () => {
		const harness = makeHarness({
			isLatestOutputPreviewRequest: vi.fn(() => false),
		});

		await runAppEffect(
			outputPathPreviewBody('preview', previewContext).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.previewOutputPath).toHaveBeenCalled();
		expect(harness.services.setOutputPreview).not.toHaveBeenCalled();
	});

	it('logs metadata warning validation failures without blocking preview', async () => {
		const cause = new Error('validation transport failed');
		const harness = makeHarness({
			updateMetadataIntentWarnings: vi.fn(async () => {
				throw cause;
			}),
		});

		await runAppEffect(
			outputPathPreviewBody('preview', previewContext).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.console.error).toHaveBeenCalledWith(
			'Metadata preview validation failed:',
			cause,
		);
		expect(harness.services.showOutputError).toHaveBeenCalledWith(
			'Failed to validate metadata preview.',
		);
		expect(harness.services.previewOutputPath).toHaveBeenCalled();
		expect(harness.preview.text).toBe('/tmp/out/a.m4b');
	});

	it('surfaces preview failures without throwing to UI callers', async () => {
		const cause = new Error('template invalid');
		const harness = makeHarness({
			previewOutputPath: vi.fn(async () => {
				throw cause;
			}),
		});

		await runAppEffect(
			outputPathPreviewBody('final', previewContext).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.preview.text).toBe(
			'Output preview unavailable. Fix metadata/template and retry.',
		);
		expect(harness.services.showOutputError).toHaveBeenCalledWith(
			`Rust preview failed: ${String(cause)}`,
		);
	});

	it('approves clean preflight without collision review', async () => {
		const cleanPlan = plan();
		const harness = makeHarness({
			preflightProcessingPlan: vi.fn(async () => cleanPlan),
		});

		const result = await runAppEffect(
			outputPlanReviewBody({
				payload: payload(),
				metadataIntentByPath: null,
				previewSeconds: null,
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.openCollisionDialog).not.toHaveBeenCalled();
		expect(result).toEqual({
			status: 'approved',
			payload: expect.objectContaining({
				collisionPolicy: 'fail',
				preflightSignature: 'sig-clean',
			}),
			plan: cleanPlan,
		});
	});

	it('blocks hard output-plan failures without opening review', async () => {
		const blockedPlan = plan({
			outputs: [
				{
					...plan().outputs[0],
					collision: {
						kind: 'source_destination_overlap',
						conflictingPath: '/books/a.m4b',
						detail: 'Output path resolves to an input source file.',
					},
					review: {
						canProceed: false,
						message: 'Output path resolves to an input source file.',
					},
					action: 'review_required',
				},
			],
		});
		const harness = makeHarness({
			preflightProcessingPlan: vi.fn(async () => blockedPlan),
		});

		const result = await runAppEffect(
			outputPlanReviewBody({ payload: payload(), metadataIntentByPath: null }).pipe(
				Effect.provide(harness.layer),
			),
		);

		expect(harness.services.openCollisionDialog).not.toHaveBeenCalled();
		expect(result).toEqual({
			status: 'blocked',
			message: 'Output path resolves to an input source file.',
			plan: blockedPlan,
		});
	});

	it('returns cancelled when collision review is cancelled', async () => {
		const reviewPlan = plan({
			outputs: [
				{
					...plan().outputs[0],
					collision: {
						kind: 'existing_file',
						conflictingPath: '/tmp/out/a.m4b',
						detail: 'An existing file already occupies the destination path.',
					},
					review: { canProceed: true, message: 'Review required.' },
					action: 'review_required',
				},
			],
		});
		const harness = makeHarness({
			preflightProcessingPlan: vi.fn(async () => reviewPlan),
			openCollisionDialog: vi.fn(async () => null),
		});

		const result = await runAppEffect(
			outputPlanReviewBody({ payload: payload(), metadataIntentByPath: null }).pipe(
				Effect.provide(harness.layer),
			),
		);

		expect(result).toEqual({ status: 'cancelled' });
	});

	it('runs reviewed preflight with the selected collision policy', async () => {
		const initialPlan = plan({
			planSignature: 'sig-review',
			outputs: [
				{
					...plan().outputs[0],
					collision: {
						kind: 'existing_file',
						conflictingPath: '/tmp/out/a.m4b',
						detail: 'An existing file already occupies the destination path.',
					},
					review: { canProceed: true, message: 'Review required.' },
					action: 'review_required',
				},
			],
		});
		const reviewedPlan = plan({
			collisionPolicy: 'rename_new',
			planSignature: 'sig-reviewed',
			outputs: [{ ...initialPlan.outputs[0], action: 'rename_new' }],
		});
		const preflightProcessingPlan = vi
			.fn()
			.mockResolvedValueOnce(initialPlan)
			.mockResolvedValueOnce(reviewedPlan);
		const metadataIntentByPath: Record<string, MetadataIntentPatch> = {
			'/books/a.m4b': { title: { op: 'set', value: 'A' } },
		};
		const processPayload = payload();
		const harness = makeHarness({
			preflightProcessingPlan,
			openCollisionDialog: vi.fn(async () => 'rename_new' as const),
		});

		const result = await runAppEffect(
			outputPlanReviewBody({
				payload: processPayload,
				metadataIntentByPath,
				previewSeconds: 30,
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(preflightProcessingPlan).toHaveBeenNthCalledWith(2, {
			payload: { ...processPayload, collisionPolicy: 'rename_new' },
			metadataIntent: metadataIntentByPath,
			previewSeconds: 30,
		});
		expect(result).toEqual({
			status: 'approved',
			payload: expect.objectContaining({
				collisionPolicy: 'rename_new',
				preflightSignature: 'sig-reviewed',
			}),
			plan: reviewedPlan,
		});
	});
});

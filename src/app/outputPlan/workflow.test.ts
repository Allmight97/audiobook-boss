import { titleAudioRequest } from '../../test/fixtures/titleAudio';
import { describe, expect, it, vi } from 'vitest';
import { Effect, runAppEffect } from '../../lib/effect/appEffect';
import type { ProcessPayload, ProcessingPreflightPlan } from '../../types/audio';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import {
	makeOutputPlanWorkflowServicesLayer,
	outputPlanReviewBody,
	type OutputPlanWorkflowServices,
} from './workflow';

function payload(overrides: Partial<ProcessPayload> = {}): ProcessPayload {
	return {
		inputFiles: ['/books/a.m4b'],
		outputDir: '/tmp/out',
		audioRequests: [titleAudioRequest()],
		outputNaming: { preset: 'absDefault', includeYear: false, customTemplate: undefined },
		...overrides,
	};
}

function plan(overrides: Partial<ProcessingPreflightPlan> = {}): ProcessingPreflightPlan {
	return {
		previewSeconds: undefined,
		collisionPolicy: 'fail',
		audioPlans: [],
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
	const services: OutputPlanWorkflowServices = {
		preflightProcessingPlan: vi.fn(async () => plan()),
		openCollisionDialog: vi.fn(async () => null),
		...overrides,
	};

	return {
		services,
		layer: makeOutputPlanWorkflowServicesLayer(services),
	};
}

describe('OutputPlanWorkflow', () => {
	it('preserves the actionable backend reason when preflight rejects', async () => {
		const message = 'FAAC HE-AAC does not support 22050 Hz. Choose a supported output sample rate.';
		const { layer, services } = makeHarness({
			preflightProcessingPlan: vi.fn(async () => {
				throw { code: 'invalid_input', category: 'validation', message };
			}),
		});
		await expect(
			runAppEffect(
				outputPlanReviewBody({ payload: payload(), metadataIntentByPath: null }).pipe(
					Effect.provide(layer),
				),
			),
		).rejects.toMatchObject({ message });
		expect(services.openCollisionDialog).not.toHaveBeenCalled();
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
			audioPlans: [],
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
			audioPlans: [],
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

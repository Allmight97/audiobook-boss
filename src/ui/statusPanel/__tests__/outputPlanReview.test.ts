import { describe, expect, it, vi } from 'vitest';
import { defaultEncoderSettings } from '../../../types/audio';
import type {
	CollisionPolicy,
	ProcessPayload,
	ProcessingPreflightPlan,
} from '../../../types/audio';
import type { MetadataIntentPatch } from '../../../types/metadataIntent';
import { reviewOutputPlanForProcessing } from '../outputPlanReview';

function payload(overrides: Partial<ProcessPayload> = {}): ProcessPayload {
	return {
		inputFiles: ['/books/a.m4b'],
		outputDir: '/tmp/out',
		settings: defaultEncoderSettings(),
		externalToolchain: {},
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

describe('reviewOutputPlanForProcessing', () => {
	it('approves clean preflight with signature and without dialog', async () => {
		const cleanPlan = plan();
		const preflightProcessingPlan = vi.fn(async () => cleanPlan);
		const openCollisionDialog = vi.fn<() => Promise<CollisionPolicy | null>>();

		const result = await reviewOutputPlanForProcessing(
			{ payload: payload(), metadataIntent: null, previewSeconds: null },
			{ preflightProcessingPlan, openCollisionDialog },
		);

		expect(openCollisionDialog).not.toHaveBeenCalled();
		expect(result).toEqual({
			status: 'approved',
			payload: expect.objectContaining({
				collisionPolicy: 'fail',
				preflightSignature: 'sig-clean',
			}),
			plan: cleanPlan,
		});
	});

	it('returns blocked for source overlap and skips dialog', async () => {
		const blockedPlan = plan({
			outputs: [
				{
					inputIndex: 0,
					inputPath: '/books/a.m4b',
					kind: 'final',
					requestedPath: '/books/a.m4b',
					resolvedPath: '/books/a.m4b',
					renameCandidate: undefined,
					collision: {
						kind: 'source_destination_overlap',
						conflictingPath: '/books/a.m4b',
						detail: 'Output path resolves to an input source file.',
					},
					action: 'review_required',
				},
			],
		});
		const preflightProcessingPlan = vi.fn(async () => blockedPlan);
		const openCollisionDialog = vi.fn<() => Promise<CollisionPolicy | null>>();

		const result = await reviewOutputPlanForProcessing(
			{ payload: payload(), metadataIntent: null },
			{ preflightProcessingPlan, openCollisionDialog },
		);

		expect(openCollisionDialog).not.toHaveBeenCalled();
		expect(result).toEqual({
			status: 'blocked',
			message: 'Output path resolves to an input source file.',
			plan: blockedPlan,
		});
	});

	it('runs a second preflight with selected collision policy', async () => {
		const initialPlan = plan({
			planSignature: 'sig-review',
			outputs: [
				{
					inputIndex: 0,
					inputPath: '/books/a.m4b',
					kind: 'final',
					requestedPath: '/tmp/out/a.m4b',
					resolvedPath: '/tmp/out/a.m4b',
					renameCandidate: '/tmp/out/a-1.m4b',
					collision: {
						kind: 'existing_file',
						conflictingPath: '/tmp/out/a.m4b',
						detail: 'An existing file already occupies the destination path.',
					},
					action: 'review_required',
				},
			],
		});
		const reviewedPlan = plan({
			collisionPolicy: 'rename_new',
			planSignature: 'sig-reviewed',
			outputs: [
				{
					...initialPlan.outputs[0],
					resolvedPath: '/tmp/out/a-1.m4b',
					action: 'rename_new',
				},
			],
		});
		const preflightProcessingPlan = vi
			.fn()
			.mockResolvedValueOnce(initialPlan)
			.mockResolvedValueOnce(reviewedPlan);
		const openCollisionDialog = vi.fn(async () => 'rename_new' as CollisionPolicy);
		const metadataIntent: Record<string, MetadataIntentPatch> = {
			'/books/a.m4b': { title: { op: 'set', value: 'A' } },
		};
		const processPayload = payload();

		const result = await reviewOutputPlanForProcessing(
			{ payload: processPayload, metadataIntent, previewSeconds: 30 },
			{ preflightProcessingPlan, openCollisionDialog },
		);

		expect(preflightProcessingPlan).toHaveBeenNthCalledWith(2, {
			payload: { ...processPayload, collisionPolicy: 'rename_new' },
			metadataIntent,
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

	it('returns cancelled when the dialog is cancelled', async () => {
		const initialPlan = plan({
			outputs: [
				{
					...plan().outputs[0],
					collision: {
						kind: 'existing_file',
						conflictingPath: '/tmp/out/a.m4b',
						detail: 'An existing file already occupies the destination path.',
					},
					action: 'review_required',
				},
			],
		});
		const preflightProcessingPlan = vi.fn(async () => initialPlan);
		const openCollisionDialog = vi.fn(async () => null);

		const result = await reviewOutputPlanForProcessing(
			{ payload: payload(), metadataIntent: null },
			{ preflightProcessingPlan, openCollisionDialog },
		);

		expect(preflightProcessingPlan).toHaveBeenCalledTimes(1);
		expect(result).toEqual({ status: 'cancelled' });
	});
});

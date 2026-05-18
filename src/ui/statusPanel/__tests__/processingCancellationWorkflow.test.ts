import { describe, expect, it, vi } from 'vitest';
import { Effect, runAppEffect } from '../../../lib/effect/appEffect';
import {
	makeProcessingCancellationWorkflowServicesLayer,
	processingCancellationWorkflowExecution,
	type ProcessingCancellationWorkflowServices,
} from '../processingCancellationWorkflow';
import type { ProcessingStatus } from '../state';

function activeStatus(): ProcessingStatus {
	return {
		stage: 'converting',
		percentage: 25,
		message: 'Converting...',
	};
}

function makeHarness(overrides: Partial<ProcessingCancellationWorkflowServices> = {}) {
	const services: ProcessingCancellationWorkflowServices = {
		cancelProcessing: vi.fn(async () => 'cancel requested'),
		setCancelAllButtonPending: vi.fn(),
		showError: vi.fn(),
		console: { error: vi.fn() },
		...overrides,
	};

	return {
		services,
		layer: makeProcessingCancellationWorkflowServicesLayer(services),
	};
}

describe('ProcessingCancellationWorkflow', () => {
	it('sets pending state around cancel-all and reports cancellation requested', async () => {
		const updateStatus = vi.fn();
		const harness = makeHarness();

		await runAppEffect(
			processingCancellationWorkflowExecution({
				type: 'cancelAll',
				currentStatus: activeStatus(),
				updateStatus,
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.cancelProcessing).toHaveBeenCalledWith();
		expect(harness.services.setCancelAllButtonPending).toHaveBeenNthCalledWith(1, true);
		expect(harness.services.setCancelAllButtonPending).toHaveBeenLastCalledWith(false);
		expect(updateStatus).toHaveBeenCalledWith({
			stage: 'converting',
			percentage: 25,
			message: 'Cancellation requested…',
		});
	});

	it('restores pending state and reports cancel-all failure', async () => {
		const cause = new Error('cancel failed');
		const updateStatus = vi.fn();
		const harness = makeHarness({
			cancelProcessing: vi.fn(async () => {
				throw cause;
			}),
		});

		await runAppEffect(
			processingCancellationWorkflowExecution({
				type: 'cancelAll',
				currentStatus: activeStatus(),
				updateStatus,
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(updateStatus).not.toHaveBeenCalled();
		expect(harness.services.setCancelAllButtonPending).toHaveBeenLastCalledWith(false);
		expect(harness.services.console.error).toHaveBeenCalledWith(
			'Failed to cancel processing:',
			cause,
		);
		expect(harness.services.showError).toHaveBeenCalledWith(
			'Failed to cancel processing. Please try again.',
		);
	});

	it('cancels a specific job by id', async () => {
		const harness = makeHarness();

		await runAppEffect(
			processingCancellationWorkflowExecution({ type: 'cancelJob', jobId: 'job-1' }).pipe(
				Effect.provide(harness.layer),
			),
		);

		expect(harness.services.cancelProcessing).toHaveBeenCalledWith('job-1');
		expect(harness.services.showError).not.toHaveBeenCalled();
	});

	it('reports per-job cancellation failure without changing render state directly', async () => {
		const cause = new Error('job cancel failed');
		const harness = makeHarness({
			cancelProcessing: vi.fn(async () => {
				throw cause;
			}),
		});

		await runAppEffect(
			processingCancellationWorkflowExecution({ type: 'cancelJob', jobId: 'job-2' }).pipe(
				Effect.provide(harness.layer),
			),
		);

		expect(harness.services.console.error).toHaveBeenCalledWith(
			'Failed to cancel job job-2:',
			cause,
		);
		expect(harness.services.showError).toHaveBeenCalledWith('Failed to cancel job job-2');
	});
});

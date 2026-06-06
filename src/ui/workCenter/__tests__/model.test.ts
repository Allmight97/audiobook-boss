import { describe, expect, it } from 'vitest';
import type { OperationSnapshot } from '../../../types/workRuntime';
import { applyProgress, upsertOperation, type WorkCenterModel } from '../model';

function operation(
	id: string,
	sequence: number,
	title: string,
	childCount = 1,
): OperationSnapshot {
	const children = Array.from({ length: childCount }, (_, index) => ({
		childJobId: `${id}-child-${index}`,
		operationId: id,
		label: `${title} ${index + 1}`,
		status: 'queued' as const,
		lane: 'encodeCpu' as const,
		progress: {
			stage: 'pending' as const,
			percentage: 0,
			message: 'Queued.',
			currentItemIndex: undefined,
			totalItems: childCount,
			bytesDownloaded: undefined,
			bytesTotal: undefined,
			etaSeconds: undefined,
		},
		sourcePath: `/tmp/${id}-${index}.m4b`,
		inputIndex: index,
		inputId: `${id}-${index}`,
		jobId: undefined,
		cancellable: false,
		cancelRequested: false,
		message: undefined,
	}));
	return {
		operationId: id,
		sequence,
		kind: 'processingBatch',
		status: 'accepted',
		title,
		createdAtMs: sequence,
		startedAtMs: undefined,
		finishedAtMs: undefined,
		cancellable: true,
		cancelRequested: false,
		lanes: ['analysis', 'encodeCpu'],
		sourceInputIds: [id],
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
		children,
		terminalSummary: undefined,
		warnings: [],
		errors: [],
	};
}

describe('Work Center model', () => {
	it('upserts one operation without erasing existing operations', () => {
		let model: WorkCenterModel = { operations: [] };
		model = upsertOperation(model, operation('op-1', 1, 'First'));
		model = upsertOperation(model, operation('op-2', 2, 'Second'));

		expect(model.operations.map((item) => item.operationId)).toEqual(['op-2', 'op-1']);
	});

	it('applies progress only to the matching operation id', () => {
		let model: WorkCenterModel = { operations: [] };
		model = upsertOperation(model, operation('op-1', 1, 'First'));
		model = upsertOperation(model, operation('op-2', 2, 'Second'));

		const updated = applyProgress(model, {
			operation_id: 'op-1',
			operation_kind: 'processingBatch',
			stage: 'converting',
			percentage: 42,
			message: 'Converting.',
			current_file: undefined,
			eta_seconds: undefined,
			job_id: 'job-1',
			input_index: 0,
		});

		expect(updated.operations.find((item) => item.operationId === 'op-1')?.progress.percentage).toBe(42);
		expect(updated.operations.find((item) => item.operationId === 'op-2')?.progress.percentage).toBe(0);
	});

	it('aggregates child-scoped batch progress instead of marking the whole operation complete', () => {
		let model: WorkCenterModel = { operations: [] };
		model = upsertOperation(model, operation('op-1', 1, 'Batch', 2));

		const updated = applyProgress(model, {
			operation_id: 'op-1',
			operation_kind: 'processingBatch',
			stage: 'completed',
			percentage: 100,
			message: 'First item complete.',
			current_file: undefined,
			eta_seconds: undefined,
			job_id: 'job-1',
			input_index: 0,
		});
		const updatedOperation = updated.operations[0];

		expect(updatedOperation.status).toBe('running');
		expect(updatedOperation.progress.stage).toBe('converting');
		expect(updatedOperation.progress.percentage).toBe(50);
		expect(updatedOperation.children[0].status).toBe('completed');
		expect(updatedOperation.children[1].status).toBe('queued');
	});
});

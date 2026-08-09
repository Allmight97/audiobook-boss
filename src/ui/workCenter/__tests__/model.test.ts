import { describe, expect, it } from 'vitest';
import type { OperationSnapshot } from '../../../types/workRuntime';
import { replaceOperations, upsertOperation } from '../model';

function operation(id: string, sequence: number, childCount = 1): OperationSnapshot {
	const children = Array.from({ length: childCount }, (_, index) => ({
		childJobId: `${id}-child-${index}`,
		operationId: id,
		label: `Child ${index + 1}`,
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
		title: `Operation ${id}`,
		createdAtMs: 1000,
		startedAtMs: undefined,
		finishedAtMs: undefined,
		cancellable: true,
		cancelRequested: false,
		lanes: ['analysis', 'encodeCpu', 'outputCommit'],
		sourceInputIds: [id],
		progress: {
			stage: 'pending',
			percentage: 0,
			message: 'Accepted.',
			currentItemIndex: undefined,
			totalItems: childCount,
			bytesDownloaded: undefined,
			bytesTotal: undefined,
			etaSeconds: undefined,
		},
		children,
		terminalSummary: undefined,
		warnings: [],
		errors: [],
		logTail: [],
	};
}

describe('Work Center model', () => {
	it('upserts one operation without erasing existing operations', () => {
		let model = { operations: [] as OperationSnapshot[] };

		model = upsertOperation(model, operation('op-1', 1));
		model = upsertOperation(model, operation('op-2', 2));

		expect(model.operations.map((item) => item.operationId)).toEqual(['op-2', 'op-1']);
	});

	it('replaceOperations sorts operations by descending sequence', () => {
		const list = {
			operations: [operation('first', 1), operation('second', 3), operation('third', 2)],
		};

		const model = replaceOperations({ operations: [] }, list);

		expect(model.operations.map((operation) => operation.operationId)).toEqual([
			'second',
			'third',
			'first',
		]);
	});

	it('keeps running work above queued and terminal history', () => {
		const running = { ...operation('running', 1), status: 'running' as const };
		const queued = operation('queued', 3);
		const completed = { ...operation('completed', 4), status: 'completed' as const };
		const model = replaceOperations(
			{ operations: [] },
			{ operations: [completed, queued, running] },
		);
		expect(model.operations.map((item) => item.operationId)).toEqual([
			'running',
			'queued',
			'completed',
		]);
	});
});

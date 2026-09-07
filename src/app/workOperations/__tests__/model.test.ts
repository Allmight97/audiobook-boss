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
		revision: 1,
		createdRevision: sequence,
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
	it('keeps newer operation state across delayed event and list responses', () => {
		const completed = { ...operation('op-1', 1), revision: 3, status: 'completed' as const };
		const older = { ...completed, revision: 2, status: 'running' as const };
		const model = { membershipRevision: 0, operations: [completed] };
		expect(upsertOperation(model, older).operations).toEqual([completed]);
		expect(
			replaceOperations(model, { membershipRevision: 1, operations: [older] }).operations,
		).toEqual([completed]);
	});

	it('preserves a new operation missing from a delayed initial list', () => {
		const newer = operation('op-2', 2);
		const model = { membershipRevision: 0, operations: [newer] };
		expect(
			replaceOperations(model, {
				membershipRevision: 1,
				operations: [operation('op-1', 1)],
			}).operations.map((item) => item.operationId),
		).toEqual(['op-2', 'op-1']);
	});

	it('honors backend pruning without resurrecting history from delayed responses', () => {
		const retired = { ...operation('op-1', 1), status: 'completed' as const };
		const retained = operation('op-2', 2);
		const model = replaceOperations(
			{ membershipRevision: 0, operations: [retired, retained] },
			{ membershipRevision: 3, operations: [retained] },
		);
		expect(model.operations).toEqual([retained]);
		expect(upsertOperation(model, retired).operations).toEqual([retained]);
		expect(
			replaceOperations(model, { membershipRevision: 1, operations: [retired] }).operations,
		).toEqual([retained]);
	});

	it('upserts one operation without erasing existing operations', () => {
		let model = { membershipRevision: 0, operations: [] as OperationSnapshot[] };

		model = upsertOperation(model, operation('op-1', 1));
		model = upsertOperation(model, operation('op-2', 2));

		expect(model.operations.map((item) => item.operationId)).toEqual(['op-2', 'op-1']);
	});

	it('replaceOperations sorts operations by descending sequence', () => {
		const list = {
			membershipRevision: 3,
			operations: [operation('first', 1), operation('second', 3), operation('third', 2)],
		};

		const model = replaceOperations({ membershipRevision: 0, operations: [] }, list);

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
			{ membershipRevision: 0, operations: [] },
			{ membershipRevision: 4, operations: [completed, queued, running] },
		);
		expect(model.operations.map((item) => item.operationId)).toEqual([
			'running',
			'queued',
			'completed',
		]);
	});
});

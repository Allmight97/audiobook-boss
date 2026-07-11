import { describe, expect, it } from 'vitest';
import type { OperationSnapshot } from '../../../types/workRuntime';
import {
	deriveWorkActivityByInputId,
	deriveWorkOperationCounts,
	replaceOperations,
	upsertOperation,
} from '../model';

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
	};
}

describe('Work Center model', () => {
	it('counts running, queued, and terminal operations for the operations bar', () => {
		const queued = operation('queued', 1);
		const running = operation('running', 2);
		running.status = 'running';
		const cancelling = operation('cancelling', 3);
		cancelling.status = 'cancelling';
		const done = operation('done', 4);
		done.status = 'completed';
		const failed = operation('failed', 5);
		failed.status = 'failed';

		expect(deriveWorkOperationCounts([queued, running, cancelling, done, failed])).toEqual({
			running: 2,
			queued: 1,
			done: 2,
		});
	});

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

	it('joins batch children to their input ids and maps terminal child status', () => {
		const snapshot = operation('batch', 1);
		snapshot.children[0] = {
			...snapshot.children[0]!,
			status: 'completed',
			inputId: 'input-batch',
		};

		expect(deriveWorkActivityByInputId([snapshot]).get('input-batch')).toMatchObject({
			status: 'done',
			sequence: 1,
		});
	});

	it('keeps a skipped child distinct from completed work', () => {
		const snapshot = operation('batch', 1);
		snapshot.children[0] = {
			...snapshot.children[0]!,
			status: 'skipped',
			inputId: 'input-skipped',
		};

		expect(deriveWorkActivityByInputId([snapshot]).get('input-skipped')).toMatchObject({
			status: 'skipped',
		});
	});

	it('projects a terminal merge operation state onto every source input id when children have no input ids', () => {
		const snapshot = operation('merge', 2);
		snapshot.status = 'mixed';
		snapshot.sourceInputIds = ['input-a', 'input-b'];
		snapshot.children = snapshot.children.map((child) => ({ ...child, inputId: undefined }));

		const activity = deriveWorkActivityByInputId([snapshot]);
		expect(activity.get('input-a')).toMatchObject({ status: 'failed', sequence: 2 });
		expect(activity.get('input-b')).toMatchObject({ status: 'failed', sequence: 2 });
	});

	it('excludes metadata-save operations that have neither child input ids nor source input ids', () => {
		const snapshot = operation('metadata', 3);
		snapshot.kind = 'metadataSave';
		snapshot.sourceInputIds = [];
		snapshot.children = snapshot.children.map((child) => ({ ...child, inputId: undefined }));

		expect(deriveWorkActivityByInputId([snapshot]).size).toBe(0);
	});

	it('maps cancelled work to cancelled, never failed', () => {
		const batch = operation('batch-cancel', 1);
		batch.children[0] = { ...batch.children[0]!, inputId: 'input-c', status: 'cancelled' };
		const merge = operation('merge-cancel', 2);
		merge.status = 'cancelled';
		merge.sourceInputIds = ['input-d'];
		merge.children = merge.children.map((child) => ({ ...child, inputId: undefined }));

		const activity = deriveWorkActivityByInputId([batch, merge]);
		expect(activity.get('input-c')).toMatchObject({ status: 'cancelled' });
		expect(activity.get('input-d')).toMatchObject({ status: 'cancelled' });
	});

	it('keeps the latest sequence per input and maps terminal failures', () => {
		const older = operation('older', 1);
		older.children[0] = { ...older.children[0]!, inputId: 'input-1', status: 'completed' };
		const newer = operation('newer', 2);
		newer.children[0] = { ...newer.children[0]!, inputId: 'input-1', status: 'failed' };

		expect(deriveWorkActivityByInputId([older, newer]).get('input-1')).toMatchObject({
			status: 'failed',
			sequence: 2,
		});
	});
});

import { describe, expect, it } from 'vitest';
import type { OperationSnapshot } from '../../lib/generated/tauri';
import { applyWorkOperations, emptyWorkCenterModel, visibleOperations } from './workOperations';

function operation(id: string, sequence: number, childCount = 1): OperationSnapshot {
	const children = Array.from({ length: childCount }, (_, index) => ({
		childJobId: `${id}-child-${index}`,
		operationId: id,
		label: `Child ${index + 1}`,
		status: 'queued' as const,
		progress: {
			stage: 'pending' as const,
			percentage: 0,
			message: 'Queued.',
			etaSeconds: null,
		},
		sourcePath: `/tmp/${id}-${index}.m4b`,
		inputIndex: index,
		inputId: `${id}-${index}`,
		sourceInputIds: [`${id}-${index}`].filter((sourceId): sourceId is string => Boolean(sourceId)),
		jobId: null,
		cancellable: false,
		cancelRequested: false,
		message: null,
		warnings: [],
		startedAtMs: null,
		finishedAtMs: null,
		outputPath: null,
		supplementalWarning: null,
		outputUpdate: null,
	}));

	return {
		operationId: id,
		sequence,
		revision: 1,
		kind: 'processingBatch',
		status: 'accepted',
		phase: 'accepted',
		title: `Operation ${id}`,
		startedAtMs: null,
		finishedAtMs: null,
		cancellable: true,
		cancelRequested: false,
		progress: {
			stage: 'pending',
			percentage: 0,
			message: 'Accepted.',
			etaSeconds: null,
		},
		children,
		logTail: [],
	};
}

describe('work center model', () => {
	it('keeps the concurrency lock of the newest update, with or without a changed operation', () => {
		let model = applyWorkOperations(emptyWorkCenterModel(), {
			revision: 2,
			order: [],
			concurrencyLocked: true,
			changed: null,
		});
		expect(model.concurrencyLocked).toBe(true);

		model = applyWorkOperations(model, {
			revision: 1,
			order: [],
			concurrencyLocked: false,
			changed: null,
		});
		expect(model.concurrencyLocked).toBe(true);

		model = applyWorkOperations(model, {
			revision: 3,
			order: [],
			concurrencyLocked: false,
			changed: null,
		});
		expect(model.concurrencyLocked).toBe(false);
	});

	const at = (op: OperationSnapshot, revision: number) => ({ ...op, revision });
	const ids = (model: ReturnType<typeof emptyWorkCenterModel>) =>
		visibleOperations(model).map((item) => `${item.operationId}@${item.revision}`);

	it('keeps the newest snapshot of each operation and the newest order, whatever the arrival order', () => {
		const a = operation('a', 1);
		const b = operation('b', 2);
		let model = emptyWorkCenterModel();
		model = applyWorkOperations(model, {
			revision: 3,
			order: ['b', 'a'],
			concurrencyLocked: false,
			changed: at(b, 1),
		});
		model = applyWorkOperations(model, {
			revision: 2,
			order: ['a'],
			concurrencyLocked: false,
			changed: at(a, 2),
		});
		model = applyWorkOperations(model, {
			revision: 1,
			order: ['a'],
			concurrencyLocked: false,
			changed: at(a, 1),
		});

		expect(ids(model)).toEqual(['b@1', 'a@2']);
	});

	it('hides an operation the newest order lists until its snapshot arrives', () => {
		const a = operation('a', 1);
		let model = applyWorkOperations(emptyWorkCenterModel(), {
			revision: 2,
			order: ['b', 'a'],
			concurrencyLocked: false,
			changed: a,
		});
		expect(ids(model)).toEqual(['a@1']);

		model = applyWorkOperations(model, {
			revision: 1,
			order: ['b'],
			concurrencyLocked: false,
			changed: operation('b', 2),
		});
		expect(ids(model)).toEqual(['b@1', 'a@1']);
	});

	it('does not bring back an operation the engine removed', () => {
		const a = operation('a', 1);
		let model = applyWorkOperations(emptyWorkCenterModel(), {
			revision: 5,
			order: [],
			concurrencyLocked: false,
			changed: operation('b', 2),
		});
		model = applyWorkOperations(model, {
			revision: 4,
			order: ['a'],
			concurrencyLocked: false,
			changed: at(a, 3),
		});

		expect(ids(model)).toEqual([]);
	});

	it('lets a late initial list fill in snapshots without undoing newer updates', () => {
		const a = operation('a', 1);
		let model = applyWorkOperations(emptyWorkCenterModel(), {
			revision: 4,
			order: ['a'],
			concurrencyLocked: false,
			changed: at(a, 3),
		});
		model = applyWorkOperations(model, {
			revision: 2,
			order: ['a'],
			concurrencyLocked: false,
			operations: [at(a, 1)],
		});

		expect(ids(model)).toEqual(['a@3']);
	});
});

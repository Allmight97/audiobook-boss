import { describe, expect, it } from 'vitest';
import type { OperationSnapshot } from '../../../types/workRuntime';
import {
	applyOperationSnapshot,
	applyWorkOperations,
	emptyWorkCenterModel,
	visibleOperations,
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
		sourceInputIds: [`${id}-${index}`].filter((id): id is string => Boolean(id)),
		jobId: undefined,
		cancellable: false,
		cancelRequested: false,
		message: undefined,
	}));

	return {
		operationId: id,
		sequence,
		revision: 1,
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
		errors: [],
		logTail: [],
	};
}

describe('work center model', () => {
	const at = (op: OperationSnapshot, revision: number) => ({ ...op, revision });
	const ids = (model: ReturnType<typeof emptyWorkCenterModel>) =>
		visibleOperations(model).map((item) => `${item.operationId}@${item.revision}`);

	it('keeps the newest snapshot of each operation and the newest order, whatever the arrival order', () => {
		const a = operation('a', 1);
		const b = operation('b', 2);
		let model = emptyWorkCenterModel();
		model = applyWorkOperations(model, { revision: 3, order: ['b', 'a'], changed: at(b, 1) });
		model = applyWorkOperations(model, { revision: 2, order: ['a'], changed: at(a, 2) });
		model = applyWorkOperations(model, { revision: 1, order: ['a'], changed: at(a, 1) });

		expect(ids(model)).toEqual(['b@1', 'a@2']);
	});

	it('hides an operation the newest order lists until its snapshot arrives', () => {
		const a = operation('a', 1);
		let model = applyWorkOperations(emptyWorkCenterModel(), {
			revision: 2,
			order: ['b', 'a'],
			changed: a,
		});
		expect(ids(model)).toEqual(['a@1']);

		model = applyWorkOperations(model, { revision: 1, order: ['b'], changed: operation('b', 2) });
		expect(ids(model)).toEqual(['b@1', 'a@1']);
	});

	it('does not bring back an operation the engine removed', () => {
		const a = operation('a', 1);
		let model = applyWorkOperations(emptyWorkCenterModel(), {
			revision: 5,
			order: [],
			changed: operation('b', 2),
		});
		model = applyWorkOperations(model, { revision: 4, order: ['a'], changed: at(a, 3) });

		expect(ids(model)).toEqual([]);
	});

	it('lets a late initial list fill in snapshots without undoing newer updates', () => {
		const a = operation('a', 1);
		let model = applyWorkOperations(emptyWorkCenterModel(), {
			revision: 4,
			order: ['a'],
			changed: at(a, 3),
		});
		model = applyWorkOperations(model, { revision: 2, order: ['a'], operations: [at(a, 1)] });

		expect(ids(model)).toEqual(['a@3']);
	});

	it('applies a cancel reply only when it is newer than what is shown', () => {
		const a = operation('a', 1);
		let model = applyWorkOperations(emptyWorkCenterModel(), {
			revision: 1,
			order: ['a'],
			changed: at(a, 2),
		});
		expect(applyOperationSnapshot(model, at(a, 1))).toBe(model);

		model = applyOperationSnapshot(model, at(a, 3));
		expect(ids(model)).toEqual(['a@3']);
	});
});

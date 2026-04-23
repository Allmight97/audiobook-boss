import { beforeEach, describe, expect, it } from 'vitest';
import type { ProcessingPreflightPlan } from '../../../types/audio';
import {
	cancelCollisionDialog,
	chooseCollisionPolicy,
	collisionDialogState,
	openCollisionDialog,
} from '../state.svelte';

function plan(): ProcessingPreflightPlan {
	return {
		jobType: 'batch',
		previewSeconds: undefined,
		collisionPolicy: 'fail',
		planSignature: 'sig-review',
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
			{
				inputIndex: 1,
				inputPath: '/books/b.m4b',
				kind: 'final',
				requestedPath: '/tmp/out/b.m4b',
				resolvedPath: '/tmp/out/b.m4b',
				renameCandidate: '/tmp/out/b-1.m4b',
				collision: {
					kind: 'existing_file',
					conflictingPath: '/tmp/out/b.m4b',
					detail: 'An existing file already occupies the destination path.',
				},
				action: 'review_required',
			},
		],
	};
}

describe('collision dialog state', () => {
	beforeEach(() => {
		cancelCollisionDialog();
	});

	it('cancel resolves null and closes the dialog', async () => {
		const result = openCollisionDialog(plan());

		cancelCollisionDialog();

		await expect(result).resolves.toBeNull();
		expect(collisionDialogState.isOpen).toBe(false);
		expect(collisionDialogState.outputs).toEqual([]);
	});

	it('opening a second dialog resolves the first as cancelled', async () => {
		const first = openCollisionDialog(plan());
		const second = openCollisionDialog(plan());

		await expect(first).resolves.toBeNull();

		chooseCollisionPolicy('rename_new');

		await expect(second).resolves.toBe('rename_new');
		expect(collisionDialogState.isOpen).toBe(false);
	});

	it('exposes only collided outputs', () => {
		openCollisionDialog(plan());

		expect(collisionDialogState.outputs).toHaveLength(1);
		expect(collisionDialogState.outputs[0]?.inputPath).toBe('/books/b.m4b');
		expect(collisionDialogState.body).toBe(
			'1 file with the same name already exists in the target output folder. How do you want to resolve the conflict?',
		);
	});
});

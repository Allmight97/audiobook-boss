import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import type { ProcessingPreflightPlan } from '../../../types/audio';
import CollisionDialogIsland from '../CollisionDialogIsland.svelte';
import { cancelCollisionDialog, collisionDialogState, openCollisionDialog } from '../state.svelte';

function plan(): ProcessingPreflightPlan {
	return {
		jobType: 'batch',
		previewSeconds: undefined,
		collisionPolicy: 'fail',
		planSignature: 'sig-review',
		outputs: [
			{
				inputIndex: 0,
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

describe('CollisionDialogIsland', () => {
	beforeEach(() => {
		cancelCollisionDialog();
	});

	it('closes on Escape through the existing cancel callback, matching the Cancel button', async () => {
		openCollisionDialog(plan());
		render(CollisionDialogIsland);

		await fireEvent.keyDown(screen.getByTestId('collision-dialog-close'), { key: 'Escape' });

		expect(collisionDialogState.isOpen).toBe(false);
	});
});

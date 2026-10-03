import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { CollisionPolicy, PlannedOutput } from '../../../types/audio';
import type { SessionIntent, SessionOutput, SubmissionStatus } from '../../../types/session';
import { fakeOutput } from '../../../test/fixtures/fakeEngine';
import { runSubmission, type SubmitDeps } from '../submit';

const openPath = vi.hoisted(() => vi.fn(async () => undefined));
vi.mock('../../../lib/tauri/client', () => ({ tauriClient: { openPath } }));

function statusPanel() {
	return {
		updateStatus: vi.fn(),
		setProcessingState: vi.fn(),
		updateArtThumbnail: vi.fn(async () => undefined),
		startProgressListener: vi.fn(async () => undefined),
		setBatchCompletionMessage: vi.fn(),
		reconcileProcessResult: vi.fn(),
		handleCancellation: vi.fn(),
		resetToIdle: vi.fn(),
	};
}

/** An engine that answers each intent with the next status in `answers`. */
function submitDeps(answers: SubmissionStatus[], reviewChoice: CollisionPolicy | null = null) {
	let output: SessionOutput = fakeOutput();
	const sent: SessionIntent[] = [];
	const deps = {
		link: {
			output: () => output,
			send: vi.fn(async (intent: SessionIntent) => {
				sent.push(intent);
				output = { ...output, submission: answers.shift() ?? null };
				return { kind: 'applied' } as const;
			}),
		},
		reviewCollisions: vi.fn(async (_outputs: readonly PlannedOutput[]) => reviewChoice),
		setControlsEnabled: vi.fn(),
		showError: vi.fn(),
	} satisfies SubmitDeps;
	return { deps, sent };
}

const collision: SubmissionStatus = { kind: 'reviewRequired', outputs: [], preview: false };

describe('runSubmission', () => {
	beforeEach(() => openPath.mockClear());

	it('restarts one title by the offer the user confirmed', async () => {
		const panel = statusPanel();
		const { deps, sent } = submitDeps([
			{
				kind: 'finishedBeforeRestart',
				outputs: { updated: 1, elsewhere: 1, restartOffered: 0, failed: 0 },
			},
		]);
		const restart = {
			titleId: 'alpha',
			operationId: 'op-1',
			revision: 3,
			from: '/a/Old.m4b',
			to: '/a/New.m4b',
		};

		await runSubmission(panel, deps, { restart });

		expect(sent).toEqual([{ kind: 'restartTitle', titleId: 'alpha', revision: 3 }]);
		expect(panel.updateStatus).toHaveBeenCalledWith(
			expect.objectContaining({ stage: 'completed' }),
		);
	});

	it('continues a held collision review after attaching without submitting again', async () => {
		const panel = statusPanel();
		const { deps, sent } = submitDeps(
			[collision, { kind: 'submitted', operationId: 'op-1', title: 'Alpha' }],
			'rename_new',
		);
		await deps.link.send({ kind: 'submit' });
		sent.length = 0;
		deps.link.send.mockClear();
		await runSubmission(panel, deps, { resumeReview: true });
		expect(sent).toEqual([{ kind: 'chooseCollisionPolicy', policy: 'rename_new' }]);
		expect(deps.reviewCollisions).toHaveBeenCalledTimes(1);
	});

	it('asks about collisions, sends the choice, and reports the accepted export', async () => {
		const panel = statusPanel();
		const { deps, sent } = submitDeps(
			[collision, { kind: 'submitted', operationId: 'op-1', title: 'Alpha' }],
			'rename_new',
		);

		await runSubmission(panel, deps);

		expect(sent).toEqual([
			{ kind: 'submit' },
			{ kind: 'chooseCollisionPolicy', policy: 'rename_new' },
		]);
		expect(panel.updateStatus).toHaveBeenLastCalledWith(
			expect.objectContaining({ stage: 'completed', message: 'Submitted to Work Center.' }),
		);
		expect(deps.setControlsEnabled).toHaveBeenLastCalledWith(true);
	});

	it('cancels the review when the user declines and starts nothing', async () => {
		const panel = statusPanel();
		const { deps, sent } = submitDeps([collision, { kind: 'cancelled' }], null);

		await runSubmission(panel, deps);

		expect(sent[sent.length - 1]).toEqual({ kind: 'cancelCollisionReview' });
		expect(panel.resetToIdle).toHaveBeenCalled();
		expect(deps.showError).not.toHaveBeenCalled();
	});

	it('words why the engine refused', async () => {
		const { deps } = submitDeps([{ kind: 'refused', reason: { kind: 'noOutputDirectory' } }]);

		await runSubmission(statusPanel(), deps);

		expect(deps.showError).toHaveBeenCalledWith('Choose an output folder before processing.');
	});

	it('treats a cancelled preview as cancellation, not failure', async () => {
		const panel = statusPanel();
		const { deps } = submitDeps([
			{
				kind: 'failed',
				error: {
					code: 'processing_cancelled',
					category: 'cancellation',
					message: 'Processing was cancelled.',
					detail: null,
				},
			},
		]);

		await runSubmission(panel, deps, { previewSeconds: 30 });

		expect(panel.handleCancellation).toHaveBeenCalledTimes(1);
		expect(deps.showError).not.toHaveBeenCalled();
	});
});

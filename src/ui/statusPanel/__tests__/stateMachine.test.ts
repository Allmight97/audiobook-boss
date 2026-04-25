import { describe, expect, it } from 'vitest';
import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../../types/events';
import type { ProcessCommandResult } from '../../../types/audio';
import {
	BATCH_COMPLETION_HOLD_MS,
	MERGE_SKIP_HOLD_MS,
	SINGLE_COMPLETION_HOLD_MS,
	applyCancellation,
	applyProgress,
	applyQueueSnapshot,
	buildBatchCompletionFeedback,
	completeSingleCompletionHold,
	createStatusPanelModel,
	reconcileProcessResult,
	withBatchCompletionMessage,
} from '../domain/stateMachine';

function toCounts(model: ReturnType<typeof createStatusPanelModel>): Record<string, number> {
	return Array.from(model.jobProgress.values()).reduce<Record<string, number>>((acc, job) => {
		acc[job.status] = (acc[job.status] ?? 0) + 1;
		return acc;
	}, {});
}

function labelMap(model: ReturnType<typeof createStatusPanelModel>): string[] {
	return model.queueOrder.map((key) => model.jobProgress.get(key)?.label ?? '');
}

describe('statusPanel state machine', () => {
	it('builds queue snapshot ordering and preserves queue labels', () => {
		const model = createStatusPanelModel();
		const snapshot: ProcessingQueueEvent = {
			items: [
				{ input_index: 2, file_path: '/books/c.m4b' },
				{ input_index: 0, file_path: '/books/a.m4b' },
				{ input_index: 1, file_path: '/books/b.m4b' },
			],
			max_concurrent: 2,
		};

		const { model: next } = applyQueueSnapshot(model, snapshot, 1000);

		expect(next.queueOrder).toEqual(['idx:2', 'idx:0', 'idx:1']);
		expect(labelMap(next)).toEqual(['c.m4b', 'a.m4b', 'b.m4b']);
		expect(next.currentStatus).toEqual({
			stage: 'analyzing',
			percentage: 0,
			message: 'Queued 3 files',
		});
		expect(next.currentStatus).not.toHaveProperty('currentFile');
		expect(next.currentStatus).not.toHaveProperty('etaSeconds');
	});

	it('handles progress events that arrive before queue snapshot', () => {
		const model = createStatusPanelModel();
		const earlyProgress: ProcessingProgressEvent = {
			input_index: 1,
			stage: 'converting',
			percentage: 55,
			message: 'Converting',
		};
		const afterEarly = applyProgress(model, earlyProgress, 1_000);
		expect(afterEarly.model.jobProgress.get('idx:1')?.status).toBe('processing');
		expect(afterEarly.model.currentStatus.stage).toBe('converting');

		const snapshot: ProcessingQueueEvent = {
			items: [
				{ input_index: 0, file_path: '/books/a.m4b' },
				{ input_index: 1, file_path: '/books/b.m4b' },
				{ input_index: 2, file_path: '/books/c.m4b' },
			],
			max_concurrent: 3,
		};
		const afterSnapshot = applyQueueSnapshot(afterEarly.model, snapshot, 2_000);

		expect(labelMap(afterSnapshot.model)).toEqual(['a.m4b', 'b.m4b', 'c.m4b']);
		expect(afterSnapshot.model.latestProgressEvent).toEqual(earlyProgress);
		expect(afterSnapshot.model.jobProgress.get('idx:1')?.status).toBe('queued');
		expect(toCounts(afterSnapshot.model).queued).toBe(3);
		expect(afterSnapshot.model.currentStatus).toEqual({
			stage: 'analyzing',
			percentage: 0,
			message: 'Queued 3 files',
		});
	});

	it('aggregates processing, completed, skipped, failed, and cancelled states', () => {
		const snapshot: ProcessingQueueEvent = {
			items: [
				{ input_index: 0, file_path: '/books/a.m4b' },
				{ input_index: 1, file_path: '/books/b.m4b' },
				{ input_index: 2, file_path: '/books/c.m4b' },
				{ input_index: 3, file_path: '/books/d.m4b' },
				{ input_index: 4, file_path: '/books/e.m4b' },
			],
			max_concurrent: 2,
		};
		const progressEvents: ProcessingProgressEvent[] = [
			{ input_index: 0, stage: 'converting', percentage: 50, message: 'Converting' },
			{ input_index: 1, stage: 'completed', percentage: 100, message: 'Done' },
			{ input_index: 2, stage: 'skipped', percentage: 100, message: 'Skipped' },
			{ input_index: 3, stage: 'failed', percentage: 100, message: 'Failed' },
			{ input_index: 4, stage: 'cancelled', percentage: 100, message: 'Cancelled' },
		];
		const result = progressEvents.reduce(
			(state, event) => applyProgress(state.model, event, 1_000 + event.input_index! * 1000),
			applyQueueSnapshot(createStatusPanelModel(), snapshot, 500),
		);
		const counts = toCounts(result.model);
		expect(result.model.currentStatus.stage).toBe('failed');
		expect(result.model.currentStatus.percentage).toBe(83.3);
		expect(counts.processing).toBe(1);
		expect(counts.completed).toBe(1);
		expect(counts.skipped).toBe(1);
		expect(counts.failed).toBe(1);
		expect(counts.cancelled).toBe(1);
	});

	it('bypasses throttle for stage transitions while coalescing same-stage updates', () => {
		const snapshot: ProcessingQueueEvent = {
			items: [{ input_index: 0, file_path: '/books/a.m4b' }],
			max_concurrent: 1,
		};
		const base = applyQueueSnapshot(createStatusPanelModel(), snapshot, 100).model;

		const initial = applyProgress(
			base,
			{
				input_index: 0,
				stage: 'analyzing',
				percentage: 10,
				message: 'Analyzing',
			},
			1_000,
		);
		const sameStageIgnored = applyProgress(
			initial.model,
			{
				input_index: 0,
				stage: 'analyzing',
				percentage: 25,
				message: 'Analyzing again',
			},
			1_250,
		);

		const stageTransition = applyProgress(
			sameStageIgnored.model,
			{
				input_index: 0,
				stage: 'writing',
				percentage: 35,
				message: 'Writing',
			},
			1_300,
		);

		expect(sameStageIgnored.model.jobProgress.get('idx:0')?.percentage).toBe(10);
		expect(stageTransition.model.jobProgress.get('idx:0')?.percentage).toBe(35);
		expect(stageTransition.model.currentStatus.message).toBe('Writing');
	});

	it('emits a single-job completion hold intent for terminal non-batch events', () => {
		const result = applyProgress(
			createStatusPanelModel(),
			{
				job_id: 'job-single',
				stage: 'completed',
				percentage: 100,
				message: 'Done',
			},
			5_000,
		);

		expect(result.intents).toEqual([
			{
				kind: 'single-completion-hold',
				jobKey: 'job:job-single',
				terminalStage: 'completed',
				message: 'Done',
				holdMs: SINGLE_COMPLETION_HOLD_MS,
			},
		]);
		expect(result.model.currentStatus.stage).toBe('completed');
		expect(result.model.jobProgress.get('job:job-single')?.status).toBe('completed');
	});

	it('reports a single skipped terminal event as skipped, not cancelled', () => {
		const result = applyProgress(
			createStatusPanelModel(),
			{
				job_id: 'job-single',
				stage: 'skipped',
				percentage: 100,
				message: 'Skipped existing output',
			},
			5_000,
		);

		expect(result.intents).toEqual([
			{
				kind: 'single-completion-hold',
				jobKey: 'job:job-single',
				terminalStage: 'skipped',
				message: 'Skipped existing output',
				holdMs: SINGLE_COMPLETION_HOLD_MS,
			},
		]);

		const completed = completeSingleCompletionHold(result.model, 'job:job-single', {
			terminalStage: 'skipped',
			message: 'Skipped existing output',
		});

		expect(completed.feedback).toEqual({
			kind: 'info',
			message: 'Skipped existing output',
		});
	});

	it('emits a batch completion hold intent with final feedback classification', () => {
		const snapshot: ProcessingQueueEvent = {
			items: [
				{ input_index: 0, file_path: '/books/a.m4b' },
				{ input_index: 1, file_path: '/books/b.m4b' },
			],
			max_concurrent: 2,
		};
		let result = applyQueueSnapshot(
			withBatchCompletionMessage(createStatusPanelModel(), 'Processed 1/2. Cancelled: 1.'),
			snapshot,
			1000,
		);
		result = applyProgress(
			result.model,
			{
				input_index: 0,
				stage: 'completed',
				percentage: 100,
				message: 'Done',
			},
			1100,
		);
		result = applyProgress(
			result.model,
			{
				input_index: 1,
				stage: 'cancelled',
				percentage: 100,
				message: 'Cancelled',
			},
			1200,
		);

		expect(result.intents).toEqual([
			{
				kind: 'batch-completion-hold',
				holdMs: BATCH_COMPLETION_HOLD_MS,
			},
		]);
		expect(buildBatchCompletionFeedback(result.model)).toEqual({
			kind: 'success',
			message: 'Processed 1/2. Cancelled: 1.',
		});
	});

	it('creates a merge skip completion hold and label', () => {
		const result = reconcileProcessResult(
			createStatusPanelModel(),
			{
				jobType: 'merge',
				summary: { total: 1, succeeded: 0, skipped: 1, cancelled: 0, failed: 0 },
				results: [
					{
						status: 'skipped',
						message: 'Skipped existing output at /books/output.m4b',
						error: null,
						jobId: 'merge-job',
					},
				],
			},
			1_000,
		);

		expect(result.intents).toEqual([
			{
				kind: 'merge-skip-hold',
				jobKey: 'job:merge-job',
				message: 'Skipped existing output at /books/output.m4b',
				holdMs: MERGE_SKIP_HOLD_MS,
			},
		]);
		expect(result.model.jobProgress.get('job:merge-job')).toMatchObject({
			label: 'Merge output',
			status: 'skipped',
			message: 'Skipped existing output at /books/output.m4b',
		});
	});

	it('marks active rows cancelled on cancellation when jobs are still running', () => {
		const snapshot: ProcessingQueueEvent = {
			items: [
				{ input_index: 0, file_path: '/books/a.m4b' },
				{ input_index: 1, file_path: '/books/b.m4b' },
				{ input_index: 2, file_path: '/books/c.m4b' },
			],
			max_concurrent: 2,
		};
		let result = applyQueueSnapshot(createStatusPanelModel(), snapshot, 1000);
		result = applyProgress(
			result.model,
			{
				input_index: 0,
				stage: 'converting',
				percentage: 15,
				message: 'Converting',
			},
			1_100,
		);
		result = applyProgress(
			result.model,
			{
				input_index: 1,
				stage: 'converting',
				percentage: 20,
				message: 'Converting',
			},
			1_200,
		);
		const cancelled = applyCancellation(result.model, 1_250);

		expect(cancelled.intents).toEqual([
			{
				kind: 'batch-completion-hold',
				holdMs: BATCH_COMPLETION_HOLD_MS,
			},
		]);
		expect(buildBatchCompletionFeedback(cancelled.model)).toEqual({
			kind: 'info',
			message: 'Processing was cancelled.',
		});
		expect(cancelled.model.jobProgress.get('idx:0')?.status).toBe('cancelled');
		expect(cancelled.model.jobProgress.get('idx:1')?.status).toBe('cancelled');
		expect(cancelled.model.jobProgress.get('idx:2')?.status).toBe('cancelled');
	});

	it('repairs rows from command results for skipped, cancelled, and failed terminal statuses', () => {
		const snapshot: ProcessingQueueEvent = {
			items: [
				{ input_index: 0, file_path: '/books/a.m4b' },
				{ input_index: 1, file_path: '/books/b.m4b' },
				{ input_index: 2, file_path: '/books/c.m4b' },
			],
			max_concurrent: 2,
		};
		let result = applyQueueSnapshot(createStatusPanelModel(), snapshot, 1000);
		result = applyProgress(
			result.model,
			{
				input_index: 0,
				job_id: 'job-0',
				stage: 'converting',
				percentage: 20,
				message: 'Converting',
			},
			1_100,
		);
		const reconcile: ProcessCommandResult = {
			jobType: 'batch',
			summary: { total: 3, succeeded: 0, skipped: 1, cancelled: 1, failed: 1 },
			results: [
				{
					status: 'skipped',
					jobId: 'job-0',
					message: 'Skipped existing',
					error: null,
				},
				{
					status: 'cancelled',
					inputIndex: 1,
					message: 'Processing was cancelled',
					error: null,
				},
				{
					status: 'failed',
					inputIndex: 2,
					message: 'Encoding failed',
					error: null,
				},
				{
					status: 'success',
					inputIndex: 0,
					message: 'ignored',
					error: null,
					jobId: 'job-0',
				},
			],
		};
		const repaired = reconcileProcessResult(result.model, reconcile, 1_500);

		expect(repaired.model.jobProgress.get('idx:0')?.status).toBe('skipped');
		expect(repaired.model.jobProgress.get('idx:1')?.status).toBe('cancelled');
		expect(repaired.model.jobProgress.get('idx:2')?.status).toBe('failed');
		expect(toCounts(repaired.model).queued ?? 0).toBe(0);
		expect(repaired.model.currentStatus.stage).toBe('failed');
	});
});

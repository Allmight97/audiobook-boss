import { cleanup, render, screen } from '@solidjs/testing-library';
import { afterEach, expect, it, vi } from 'vitest';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../../app/runtime';
import type { OperationSnapshot } from '../../../types/workRuntime';
import { WorkCenterView } from '..';

let runtime: AppRuntime | undefined;
afterEach(() => {
	cleanup();
	runtime?.dispose();
	vi.restoreAllMocks();
});

function completedOperation(): OperationSnapshot {
	const progress = { stage: 'complete' as const, percentage: 100, message: 'Complete.' };
	return {
		operationId: 'batch',
		sequence: 1,
		revision: 1,
		createdRevision: 1,
		kind: 'processingBatch',
		status: 'completed',
		title: 'Batch encode (2 files)',
		createdAtMs: 0,
		startedAtMs: 1_000,
		finishedAtMs: 20_000,
		cancellable: false,
		cancelRequested: false,
		lanes: ['encodeCpu'],
		sourceInputIds: [],
		progress,
		children: [
			{
				childJobId: 'first',
				operationId: 'batch',
				label: 'A Change of Plans.m4b',
				status: 'completed',
				lane: 'encodeCpu',
				progress,
				cancellable: false,
				cancelRequested: false,
				startedAtMs: 2_000,
				finishedAtMs: 19_685,
			},
			{
				childJobId: 'second',
				operationId: 'batch',
				label: 'Feedback.m4b',
				status: 'completed',
				lane: 'encodeCpu',
				progress,
				cancellable: false,
				cancelRequested: false,
				startedAtMs: 2_000,
				finishedAtMs: 15_150,
			},
		],
		warnings: [],
		errors: [],
		logTail: [],
	};
}

function showOperation(operation: OperationSnapshot) {
	runtime = createAppRuntime();
	vi.spyOn(runtime.workOperations, 'view').mockReturnValue({
		initialized: true,
		operations: [operation],
		cancelPendingByOperationId: {},
		errorMessage: null,
	});
	render(() => (
		<AppRuntimeProvider runtime={runtime!}>
			<WorkCenterView />
		</AppRuntimeProvider>
	));
}

it('renders operation wall time and each file duration independently in the existing status labels', () => {
	showOperation(completedOperation());
	expect(screen.getByText('Completed in 00:19')).toHaveClass('work-status', 'is-completed');
	expect(screen.getByText('Done in 00:18')).toBeVisible();
	expect(screen.getByText('Done in 00:13')).toBeVisible();
	expect(screen.getByText('100%')).toBeVisible();
});

it('keeps plain completion labels when timestamps are unavailable', () => {
	const operation = completedOperation();
	operation.startedAtMs = undefined;
	for (const child of operation.children) child.finishedAtMs = undefined;
	showOperation(operation);
	expect(screen.getByText('Completed')).toBeVisible();
	expect(screen.getAllByText('Done')).toHaveLength(2);
	expect(screen.queryByText(/in 00:00/)).not.toBeInTheDocument();
});

it('keeps durations beyond an hour in minutes and seconds', () => {
	const operation = completedOperation();
	operation.finishedAtMs = 3_662_000;
	showOperation(operation);
	expect(screen.getByText('Completed in 61:01')).toBeVisible();
});

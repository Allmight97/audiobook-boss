import { cleanup, render, screen } from '@solidjs/testing-library';
import { afterEach, expect, it, vi } from 'vitest';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../../app/runtime';
import type { RestartOffer } from '../../../types/session';
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
				sourceInputIds: [],
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
				sourceInputIds: [],
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
		errors: [],
		logTail: [],
	};
}

function showOperation(operation: OperationSnapshot, offers: RestartOffer[] = []) {
	runtime = createAppRuntime();
	vi.spyOn(runtime.processing, 'restartOffers').mockReturnValue(offers);
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

it('shows each failed file with its own reason', () => {
	const operation = completedOperation();
	operation.status = 'mixed';
	operation.children[0] = {
		...operation.children[0]!,
		status: 'failed',
		message: 'File validation failed: File not found: A Change of Plans.m4b',
	};
	operation.children[1] = { ...operation.children[1]!, message: 'Saved metadata: Feedback.m4b' };
	showOperation(operation);
	expect(
		screen.getByText('File validation failed: File not found: A Change of Plans.m4b'),
	).toBeVisible();
	expect(screen.queryByText('Saved metadata: Feedback.m4b')).not.toBeInTheDocument();
});

it('shows a finished file whose PDF was not saved as done with the warning', () => {
	const operation = completedOperation();
	operation.children[0] = {
		...operation.children[0]!,
		supplementalWarning: 'Audiobook output was created, but the PDF could not be committed.',
	};
	showOperation(operation);
	expect(screen.getByText('Done in 00:18')).toBeVisible();
	expect(
		screen.getByText('Audiobook output was created, but the PDF could not be committed.'),
	).toBeVisible();
});

it('cancels one title in a running batch and hides title cancel for a single title', () => {
	const operation = completedOperation();
	operation.status = 'running';
	operation.cancellable = true;
	operation.children = operation.children.map((child) => ({
		...child,
		status: 'running',
		cancellable: true,
	}));
	showOperation(operation);
	const cancel = vi.spyOn(runtime!.workOperations, 'cancel').mockResolvedValue();

	screen.getByTitle('Cancel Feedback.m4b only').click();

	expect(cancel).toHaveBeenCalledWith('batch', 'second');
	cleanup();
	runtime?.dispose();
	showOperation({ ...operation, children: [operation.children[0]!] });
	expect(screen.queryByTitle(/only$/)).not.toBeInTheDocument();
});

it('reveals only completed titles in the host file manager', () => {
	const operation = completedOperation();
	operation.children[0] = {
		...operation.children[0]!,
		outputPath: '/Library/A Change of Plans.m4b',
	};
	operation.children[1] = {
		...operation.children[1]!,
		status: 'cancelled' as const,
		outputPath: '/x.m4b',
	};
	showOperation(operation);
	const reveal = vi.spyOn(runtime!.workOperations, 'revealOutput').mockResolvedValue();
	const buttons = screen.getAllByRole('button', { name: /^Show in / });
	expect(buttons).toHaveLength(1);
	buttons[0]!.click();
	expect(reveal).toHaveBeenCalledWith(operation.children[0]);
});

it('keeps retryable Restart and Keep actions on only the matching export row', () => {
	const operation = completedOperation();
	operation.children[0]!.inputId = 'first-title';
	operation.children[1]!.inputId = 'other-title';
	const offer = {
		titleId: 'first-title',
		operationId: 'batch',
		revision: 3,
		from: '/Old.m4b',
		to: '/New.m4b',
	};
	showOperation(operation, [offer]);
	const restart = vi.spyOn(runtime!.processing, 'restart').mockResolvedValue();
	const keep = vi.spyOn(runtime!.processing, 'keepLocation').mockResolvedValue();
	screen.getByRole('button', { name: 'Restart' }).click();
	screen.getByRole('button', { name: 'Restart' }).click();
	expect(restart).toHaveBeenCalledTimes(2);
	expect(restart).toHaveBeenLastCalledWith(offer);
	screen.getByRole('button', { name: 'Keep Location' }).click();
	expect(keep).toHaveBeenCalledWith(offer);
	cleanup();
	runtime?.dispose();
	showOperation({ ...operation, operationId: 'older-batch' }, [offer]);
	expect(screen.queryByRole('button', { name: 'Restart' })).not.toBeInTheDocument();
});

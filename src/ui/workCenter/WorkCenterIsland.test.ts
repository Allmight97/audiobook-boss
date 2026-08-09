import { render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { OperationSnapshot } from '../../types/workRuntime';
import WorkCenterIsland from './WorkCenterIsland.svelte';

const context = vi.hoisted(() => ({
	state: {
		initialized: true,
		operations: [] as OperationSnapshot[],
		cancelPendingByOperationId: {} as Record<string, boolean>,
		errorMessage: null as string | null,
	},
}));

vi.mock('./state.svelte', () => ({
	cancelWorkOperation: vi.fn(),
	initializeWorkCenter: vi.fn(async () => undefined),
	openChildSource: vi.fn(),
	workCenterState: context.state,
}));

function runningOperation(): OperationSnapshot {
	return {
		operationId: 'op-eta',
		sequence: 1,
		kind: 'processingBatch',
		status: 'running',
		title: 'ETA proof',
		createdAtMs: 1,
		startedAtMs: 2,
		finishedAtMs: undefined,
		cancellable: true,
		cancelRequested: false,
		lanes: ['encodeCpu'],
		sourceInputIds: ['input-1'],
		progress: {
			stage: 'converting',
			percentage: 64,
			message: 'Encoding audio...',
			currentItemIndex: 0,
			totalItems: 1,
			bytesDownloaded: undefined,
			bytesTotal: undefined,
			etaSeconds: 242,
		},
		children: [],
		terminalSummary: undefined,
		warnings: [],
		errors: [],
		logTail: [],
	};
}

function child(
	status: OperationSnapshot['children'][number]['status'],
	etaSeconds?: number,
): OperationSnapshot['children'][number] {
	return {
		childJobId: `child-${status}`,
		operationId: 'op-eta',
		label: `${status}.m4b`,
		status,
		lane: 'encodeCpu',
		progress: {
			stage: status === 'running' ? 'converting' : 'complete',
			percentage: status === 'running' ? 64 : 100,
			message: status,
			etaSeconds,
		},
		cancellable: status === 'running',
		cancelRequested: false,
	};
}

describe('WorkCenterIsland', () => {
	beforeEach(() => {
		context.state.operations = [];
		context.state.cancelPendingByOperationId = {};
		context.state.errorMessage = null;
	});

	it('renders backend-authored ETA beside the matching operation progress', () => {
		context.state.operations = [runningOperation()];

		render(WorkCenterIsland);

		expect(screen.getByText('64% · 04:02 left')).toBeInTheDocument();
	});

	it('renders ETA only for the running child when aggregate ETA is unavailable', () => {
		const operation = runningOperation();
		operation.progress.etaSeconds = undefined;
		operation.children = [child('running', 242), child('completed', 242), child('queued')];
		context.state.operations = [operation];

		render(WorkCenterIsland);

		expect(screen.getByText('Running · 04:02 left')).toBeInTheDocument();
		expect(screen.getByText('Done')).toBeInTheDocument();
		expect(screen.getByText('Queued')).toBeInTheDocument();
		expect(screen.getAllByText(/04:02 left/)).toHaveLength(1);
	});

	it('suppresses stale aggregate ETA after the operation becomes terminal', () => {
		const operation = runningOperation();
		operation.status = 'cancelled';
		context.state.operations = [operation];

		render(WorkCenterIsland);

		expect(screen.getByText('64%')).toBeInTheDocument();
		expect(screen.queryByText(/04:02 left/)).not.toBeInTheDocument();
	});
});

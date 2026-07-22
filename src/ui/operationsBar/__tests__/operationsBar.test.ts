import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo } from '../../../types/audio';
import { EVENTS, type WorkOperationSnapshotEvent } from '../../../types/events';
import type { OperationListSnapshot, OperationSnapshot } from '../../../types/workRuntime';
import { tauriClient } from '../../../lib/tauri/client';
import { setCurrentFileList, setOrderLocked } from '../../fileList/state.svelte';
import { initStatusPanel } from '../../statusPanel';
import {
	resetStatusPanelViewState,
	statusPanelViewState,
} from '../../statusPanel/viewState.svelte';
import { applyOperationListSnapshot, disposeWorkCenter } from '../../workCenter/state.svelte';
import OperationsBarIsland from '../OperationsBarIsland.svelte';

function operation(status: OperationSnapshot['status'] = 'running'): OperationSnapshot {
	return {
		operationId: 'operation-1',
		sequence: 1,
		kind: 'processingBatch',
		status,
		title: 'The Way of Kings — batch encode',
		createdAtMs: 1,
		startedAtMs: 2,
		finishedAtMs: undefined,
		cancellable: true,
		cancelRequested: false,
		lanes: ['analysis', 'encodeCpu', 'outputCommit'],
		sourceInputIds: ['input-1'],
		progress: {
			stage: 'converting',
			percentage: 64,
			message: 'Encoding chunk 8/12',
			currentItemIndex: 0,
			totalItems: 1,
			bytesDownloaded: undefined,
			bytesTotal: undefined,
			etaSeconds: undefined,
		},
		children: [
			{
				childJobId: 'child-1',
				operationId: 'operation-1',
				label: 'The Way of Kings.m4b',
				status: 'running',
				lane: 'encodeCpu',
				progress: {
					stage: 'converting',
					percentage: 64,
					message: 'Encoding chunk 8/12',
					currentItemIndex: 0,
					totalItems: 1,
					bytesDownloaded: undefined,
					bytesTotal: undefined,
					etaSeconds: undefined,
				},
				sourcePath: '/books/the-way-of-kings.m4b',
				inputIndex: 0,
				inputId: 'input-1',
				jobId: 'job-1',
				cancellable: true,
				cancelRequested: false,
				message: 'Encoding chunk 8/12',
			},
		],
		terminalSummary: undefined,
		warnings: [],
		errors: [],
		logTail: [],
	};
}

function operationList(...operations: OperationSnapshot[]): OperationListSnapshot {
	return { operations };
}

function fileList(): FileListInfo {
	return {
		files: [
			{
				path: '/books/the-way-of-kings.m4b',
				inputId: 'input-1',
				isValid: true,
				duration: 60,
				size: 1024,
				format: 'm4b',
			},
		],
		validCount: 1,
		invalidCount: 0,
		totalDuration: 60,
		totalSize: 1024,
		selectedDecoders: [null],
	};
}

describe('OperationsBarIsland', () => {
	beforeEach(() => {
		disposeWorkCenter();
		applyOperationListSnapshot(operationList());
		resetStatusPanelViewState();
		setOrderLocked(false);
		setCurrentFileList(fileList());
		vi.restoreAllMocks();
	});

	afterEach(() => {
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = undefined;
		disposeWorkCenter();
	});

	it('transitions collapsed to open to pinned, blocks disclosure while pinned, then unpins to open', async () => {
		render(OperationsBarIsland);
		const disclosure = screen.getByRole('button', { name: 'Toggle operations' });

		expect(disclosure).toHaveAttribute('aria-expanded', 'false');
		await fireEvent.click(disclosure);
		expect(disclosure).toHaveAttribute('aria-expanded', 'true');

		const pin = screen.getByRole('button', { name: 'Pin operations open' });
		await fireEvent.click(pin);
		expect(pin).toHaveAttribute('aria-pressed', 'true');
		await fireEvent.click(disclosure);
		expect(disclosure).toHaveAttribute('aria-expanded', 'true');

		await fireEvent.click(screen.getByRole('button', { name: 'Unpin operations' }));
		expect(disclosure).toHaveAttribute('aria-expanded', 'true');
	});

	it('toggles the body from the bar row while controls keep their own click behavior', async () => {
		const { container } = render(OperationsBarIsland);
		const row = container.querySelector('.operations-bar-row') as HTMLElement;
		const disclosure = screen.getByRole('button', { name: 'Toggle operations' });

		expect(disclosure).toHaveAttribute('aria-expanded', 'false');
		await fireEvent.click(row);
		expect(disclosure).toHaveAttribute('aria-expanded', 'true');
		await fireEvent.click(row);
		expect(disclosure).toHaveAttribute('aria-expanded', 'false');
	});

	it('hides the running/queued/done badge cluster at idle 0/0/0 and shows it once a count is non-zero', async () => {
		const { container } = render(OperationsBarIsland);

		expect(container.querySelector('.app-badge-info')).not.toBeInTheDocument();
		expect(container.querySelector('.app-badge-muted')).not.toBeInTheDocument();
		expect(container.querySelector('.app-badge-ok')).not.toBeInTheDocument();

		applyOperationListSnapshot(operationList(operation()));
		await waitFor(() => {
			expect(container.querySelector('.app-badge-info')).toHaveTextContent('1 running');
		});
		expect(container.querySelector('.app-badge-muted')).toHaveTextContent('0 queued');
		expect(container.querySelector('.app-badge-ok')).toHaveTextContent('0 done');
	});

	it('uses book totals and flips the visible pin label', async () => {
		render(OperationsBarIsland);
		expect(screen.getByLabelText('File totals')).toHaveTextContent('1 book');
		const pin = screen.getByRole('button', { name: 'Pin operations open' });
		expect(pin).toHaveTextContent('⚲ pin');
		await fireEvent.click(pin);
		expect(pin).toHaveTextContent('⚲ pinned');
	});

	it('renders a running background transport when foreground transport is idle', async () => {
		const { container } = render(OperationsBarIsland);
		applyOperationListSnapshot(
			operationList({ ...operation(), progress: { ...operation().progress, etaSeconds: 242 } }),
		);

		await waitFor(() => {
			expect(
				screen.getByText('The Way of Kings — batch encode · 64% · 04:02 left'),
			).toBeInTheDocument();
		});
		expect(screen.queryByTestId('status-transport-progress')).not.toBeInTheDocument();

		const track = container.querySelector('.operations-bar-background-track') as HTMLElement;
		expect(track).toHaveAttribute('role', 'progressbar');
		expect(track).toHaveAttribute('aria-valuemin', '0');
		expect(track).toHaveAttribute('aria-valuemax', '100');
		expect(track).toHaveAttribute('aria-valuenow', '64');
	});

	it('lets a running operation outrank persistent foreground feedback', async () => {
		const { showError } = await import('../../statusPanel/viewState.svelte');
		showError('Preview failed.');
		render(OperationsBarIsland);
		applyOperationListSnapshot(operationList(operation()));

		await waitFor(() => {
			expect(screen.getByText(/The Way of Kings — batch encode · 64%/)).toBeInTheDocument();
		});
		expect(screen.queryByText(/Error: Preview failed./)).not.toBeInTheDocument();
	});

	it('preserves a verdict written while a background operation was already running', async () => {
		const { showError } = await import('../../statusPanel/viewState.svelte');
		render(OperationsBarIsland);

		// Foreground preview is active when the background operation appears —
		// not a takeover of an idle row, so nothing may be cleared.
		statusPanelViewState.isProcessing = true;
		await tick();
		applyOperationListSnapshot(operationList(operation()));
		await tick();

		// The preview finishes DURING the background operation: its verdict is
		// fresh, not stale, and must survive the row being in background mode.
		statusPanelViewState.isProcessing = false;
		showError('Preview failed.');
		await tick();

		// Background operation terminalizes → the row reverts to the status
		// transport → the fresh verdict wins, instead of being erased.
		applyOperationListSnapshot(operationList({ ...operation(), status: 'completed' }));
		await waitFor(() => {
			expect(screen.getByText(/Preview failed./)).toBeInTheDocument();
		});
	});

	it('renders a cancelling operation on the background transport, not idle', async () => {
		const { container } = render(OperationsBarIsland);
		applyOperationListSnapshot(operationList({ ...operation(), status: 'cancelling' }));

		await waitFor(() => {
			expect(container.querySelector('.operations-bar-background-line')).toHaveTextContent(
				'The Way of Kings — batch encode',
			);
		});
		expect(screen.queryByText(/^Idle$/)).not.toBeInTheDocument();
	});

	it('appends the order-lock suffix to idle transport', async () => {
		setOrderLocked(true);
		render(OperationsBarIsland);
		await waitFor(() => {
			expect(screen.getByText(/Idle/)).toBeInTheDocument();
			expect(screen.getByText('· order locked (submitting)')).toBeInTheDocument();
		});
	});

	it('renders background snapshots through the Work Center apply seam, expands child lanes, and cancels through the existing path', async () => {
		const cancelSpy = vi.spyOn(tauriClient, 'cancelWorkOperation').mockResolvedValue(operation());
		render(OperationsBarIsland);
		applyOperationListSnapshot(operationList(operation()));

		await waitFor(() => {
			expect(screen.getByText('The Way of Kings — batch encode')).toBeInTheDocument();
		});
		await fireEvent.click(
			screen.getByRole('button', { name: 'Expand The Way of Kings — batch encode' }),
		);
		expect(screen.getByTestId('operation-lane-encodeCpu')).toHaveTextContent('Encode');
		expect(screen.getByText('The Way of Kings.m4b')).toBeInTheDocument();

		await fireEvent.click(
			screen.getByRole('button', { name: 'Cancel The Way of Kings — batch encode' }),
		);
		expect(cancelSpy).toHaveBeenCalledWith('operation-1');
	});

	it('carries runtime events through the Work Center UI from progress to cancellation terminal truth', async () => {
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
		// A stale preview verdict from an earlier, unrelated run must not
		// resurface once this background operation finishes and the transport
		// row reverts to StatusTransportIsland (F11).
		const { showError } = await import('../../statusPanel/viewState.svelte');
		showError('Stale preview verdict.');
		const running = {
			...operation(),
			progress: { ...operation().progress, etaSeconds: 242 },
			logTail: [{ timestampMs: 3, message: 'Encoding chunk 8/12' }],
		};
		const cancelling: OperationSnapshot = {
			...running,
			status: 'cancelling',
			cancelRequested: true,
			progress: { ...running.progress, message: 'Cancellation requested' },
			children: running.children.map((child) => ({ ...child, cancelRequested: true })),
			logTail: [...running.logTail, { timestampMs: 4, message: 'Cancellation requested' }],
		};
		const cancelled: OperationSnapshot = {
			...cancelling,
			status: 'cancelled',
			finishedAtMs: 5,
			cancellable: false,
			progress: {
				...cancelling.progress,
				stage: 'cancelled',
				message: 'Cancelled 1/1.',
			},
			children: cancelling.children.map((child) => ({
				...child,
				status: 'cancelled',
				cancellable: false,
				progress: { ...child.progress, stage: 'cancelled', message: 'Cancelled' },
			})),
			terminalSummary: {
				total: 1,
				succeeded: 0,
				skipped: 0,
				cancelled: 1,
				failed: 0,
				message: 'Cancelled 1/1.',
			},
			logTail: [...cancelling.logTail, { timestampMs: 5, message: 'Operation cancelled' }],
		};
		const listenSpy = vi.spyOn(tauriClient, 'listen').mockResolvedValue(vi.fn());
		vi.spyOn(tauriClient, 'listWorkOperations').mockResolvedValue(operationList());
		const cancelSpy = vi.spyOn(tauriClient, 'cancelWorkOperation').mockResolvedValue(cancelling);
		const { container } = render(OperationsBarIsland);

		await waitFor(() => expect(listenSpy).toHaveBeenCalledTimes(2));
		expect(listenSpy.mock.calls.map(([event]) => String(event))).toEqual([
			EVENTS.WORK_OPERATION_SNAPSHOT,
			EVENTS.WORK_OPERATION_LIST_SNAPSHOT,
		]);
		const snapshotHandler = listenSpy.mock.calls[0]?.[1] as
			| ((event: { payload: WorkOperationSnapshotEvent }) => void)
			| undefined;
		if (!snapshotHandler) throw new Error('Expected the WorkRuntime snapshot listener');

		snapshotHandler({ payload: { snapshot: running } });
		await waitFor(() => {
			expect(
				screen.getByText('The Way of Kings — batch encode · 64% · 04:02 left'),
			).toBeInTheDocument();
		});

		await fireEvent.click(screen.getByRole('button', { name: 'Toggle operations' }));
		await fireEvent.click(
			screen.getByRole('button', { name: 'Expand The Way of Kings — batch encode' }),
		);
		expect(container.querySelector('.op-log')).toHaveTextContent('Encoding chunk 8/12');

		await fireEvent.click(
			screen.getByRole('button', { name: 'Cancel The Way of Kings — batch encode' }),
		);
		await waitFor(() => expect(cancelSpy).toHaveBeenCalledWith('operation-1'));
		expect(screen.getByText('cancelling')).toBeInTheDocument();
		expect(
			screen.queryByRole('button', { name: 'Cancel The Way of Kings — batch encode' }),
		).not.toBeInTheDocument();
		expect(container.querySelector('.operations-bar-background-line')).toHaveTextContent(
			'The Way of Kings — batch encode',
		);

		snapshotHandler({ payload: { snapshot: cancelled } });
		await waitFor(() => expect(screen.getByText('cancelled')).toBeInTheDocument());
		expect(screen.getByText('Cancelled 1/1.')).toBeInTheDocument();
		expect(screen.getByText(/^Idle$/)).toBeInTheDocument();
		expect(screen.queryByText(/Stale preview verdict\./)).not.toBeInTheDocument();
	});

	it('keeps foreground transport and background operation snapshots in separate lanes', async () => {
		render(OperationsBarIsland);
		expect(screen.getByTestId('status-transport-progress')).toHaveStyle({ width: '0%' });

		applyOperationListSnapshot(operationList(operation()));
		await tick();
		expect(screen.queryByTestId('status-transport-progress')).not.toBeInTheDocument();
		expect(screen.getByText('The Way of Kings — batch encode')).toBeInTheDocument();

		initStatusPanel().applyProgress({
			operation_kind: 'processingBatch',
			stage: 'converting',
			input_index: 0,
			job_id: 'preview-job',
			current_file: '/books/foreground-preview.m4b',
			percentage: 23,
			message: 'Previewing',
			eta_seconds: undefined,
		});
		await waitFor(() => {
			expect(screen.getByTestId('status-transport-progress')).toHaveStyle({ width: '23%' });
		});
		expect(screen.getByText('The Way of Kings — batch encode')).toBeInTheDocument();
	});

	it('does not toggle disclosure or preventDefault when Enter/Space targets the pin button', async () => {
		render(OperationsBarIsland);
		const disclosure = screen.getByRole('button', { name: 'Toggle operations' });
		const pin = screen.getByRole('button', { name: 'Pin operations open' });

		expect(await fireEvent.keyDown(pin, { key: 'Enter' })).toBe(true);
		expect(disclosure).toHaveAttribute('aria-expanded', 'false');
		expect(await fireEvent.keyDown(pin, { key: ' ' })).toBe(true);
		expect(disclosure).toHaveAttribute('aria-expanded', 'false');
	});

	it('does not toggle disclosure or preventDefault when Enter/Space targets Cancel All', async () => {
		statusPanelViewState.isProcessing = true;
		statusPanelViewState.hasCancellableForegroundJob = true;
		render(OperationsBarIsland);
		const disclosure = screen.getByRole('button', { name: 'Toggle operations' });
		const cancelAll = screen.getByRole('button', { name: 'Cancel All' });

		expect(await fireEvent.keyDown(cancelAll, { key: 'Enter' })).toBe(true);
		expect(disclosure).toHaveAttribute('aria-expanded', 'false');
		expect(await fireEvent.keyDown(cancelAll, { key: ' ' })).toBe(true);
		expect(disclosure).toHaveAttribute('aria-expanded', 'false');
	});

	it('still toggles disclosure via Enter/Space on the row itself', async () => {
		render(OperationsBarIsland);
		const disclosure = screen.getByRole('button', { name: 'Toggle operations' });

		expect(await fireEvent.keyDown(disclosure, { key: 'Enter' })).toBe(false);
		expect(disclosure).toHaveAttribute('aria-expanded', 'true');
		expect(await fireEvent.keyDown(disclosure, { key: ' ' })).toBe(false);
		expect(disclosure).toHaveAttribute('aria-expanded', 'false');
	});

	it('renders a metadata batch-save snapshot through the Work Center seam', async () => {
		const metadataSave = {
			...operation('completed'),
			operationId: 'metadata-save-1',
			kind: 'metadataSave' as const,
			title: 'Save metadata (3 files)',
			sourceInputIds: [],
			children: [],
		};
		render(OperationsBarIsland);
		applyOperationListSnapshot(operationList(metadataSave));

		await waitFor(() => {
			expect(screen.getByText('Save metadata (3 files)')).toBeInTheDocument();
		});
	});
});

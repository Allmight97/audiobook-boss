import { createSignal, type Accessor } from 'solid-js';
import type { EngineLink } from '../engineLink';
import { toUserMessage } from '../../lib/tauri/appError';
import { tauriClient } from '../../lib/tauri/client';
import type { OperationId, OperationSnapshot } from '../../types/workRuntime';

export type WorkOperationsView = {
	readonly operations: ReadonlyArray<OperationSnapshot>;
	/** Where an accepted operation waits, counted from 1; absent once it starts. */
	queuePosition: (operationId: OperationId) => number | null;
	readonly cancelPendingByOperationId: Readonly<Partial<Record<string, boolean>>>;
	readonly errorMessage: string | null;
};

export type WorkOperationsOwner = {
	readonly view: Accessor<WorkOperationsView>;
	/** Cancels the whole operation, or only the title named by `childJobId`. */
	cancel: (operationId: OperationId, childJobId?: string) => Promise<void>;
	revealOutput: (child: { outputPath?: string | null }) => Promise<void>;
	reset: () => void;
};

const CANCEL_ERROR_PREFIX = 'Failed to cancel';
const REVEAL_ERROR_PREFIX = 'Failed to show the exported file';

/** Work runs now, a cancel included until it lands; only running work shows motion. */
export function isWorkRunning(status: OperationSnapshot['status']): boolean {
	return status === 'running' || status === 'cancelling';
}

export function createWorkOperationsOwner(deps: {
	readonly link: EngineLink;
}): WorkOperationsOwner {
	let cancelPendingByOperationId: Readonly<Partial<Record<string, boolean>>> = {};
	let errorMessage: string | null = null;
	let disposed = false;
	const [rev, bump] = createSignal(0, { ownedWrite: true });

	function showError(message: string | null): void {
		if (disposed) return;
		errorMessage = message;
		bump((n) => n + 1);
	}

	function setCancelPending(operationId: OperationId, pending: boolean): void {
		if (disposed) return;
		const { [operationId]: _removed, ...others } = cancelPendingByOperationId;
		cancelPendingByOperationId = pending ? { ...others, [operationId]: true } : others;
		bump((n) => n + 1);
	}

	async function sendCancel(operationId: OperationId, childJobId?: string): Promise<void> {
		try {
			await deps.link.cancelWork(operationId, childJobId);
			if (errorMessage?.startsWith(CANCEL_ERROR_PREFIX)) showError(null);
		} catch (error) {
			showError(
				`${CANCEL_ERROR_PREFIX} ${childJobId ? 'title' : 'operation'}: ${toUserMessage(error)}`,
			);
		}
	}

	return {
		view: () => {
			rev();
			return {
				operations: deps.link.workOperations(),
				queuePosition: (operationId) => {
					const index = deps.link.workQueue().indexOf(operationId);
					return index < 0 ? null : index + 1;
				},
				cancelPendingByOperationId,
				errorMessage,
			};
		},
		async cancel(operationId, childJobId) {
			if (childJobId !== undefined) {
				await sendCancel(operationId, childJobId);
				return;
			}
			setCancelPending(operationId, true);
			await sendCancel(operationId);
			setCancelPending(operationId, false);
		},
		async revealOutput(child) {
			if (!child.outputPath) return;
			if (errorMessage?.startsWith(REVEAL_ERROR_PREFIX)) showError(null);
			try {
				await tauriClient.revealPath(child.outputPath);
			} catch (error) {
				showError(`${REVEAL_ERROR_PREFIX}: ${toUserMessage(error)}`);
			}
		},
		reset() {
			disposed = true;
		},
	};
}

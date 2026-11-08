import { createSignal, type Accessor } from 'solid-js';
import type { EngineLink } from '../engineLink';
import { toUserMessage } from '../../lib/tauri/appError';
import { tauriClient } from '../../lib/tauri/client';
import type { OperationId, OperationSnapshot } from '../../lib/generated/tauri';

export type WorkOperationsError = {
	readonly kind: 'cancel' | 'reveal';
	readonly message: string;
};

export type WorkOperationsView = {
	readonly operations: ReadonlyArray<OperationSnapshot>;
	readonly cancelPendingByOperationId: Readonly<Partial<Record<string, boolean>>>;
	readonly error: WorkOperationsError | null;
};

export type WorkOperationsOwner = {
	readonly view: Accessor<WorkOperationsView>;
	cancel: (operationId: OperationId, childJobId?: string) => Promise<void>;
	revealOutput: (child: { outputPath?: string | null }) => Promise<void>;
	reset: () => void;
};

export function createWorkOperationsOwner(deps: {
	readonly link: EngineLink;
}): WorkOperationsOwner {
	let cancelPendingByOperationId: Readonly<Partial<Record<string, boolean>>> = {};
	let error: WorkOperationsError | null = null;
	let disposed = false;
	const [rev, bump] = createSignal(0, { ownedWrite: true });

	function setError(next: WorkOperationsError | null): void {
		if (disposed) return;
		error = next;
		bump((n) => n + 1);
	}

	function clearError(kind: WorkOperationsError['kind']): void {
		if (error?.kind === kind) setError(null);
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
			clearError('cancel');
		} catch (cause) {
			setError({
				kind: 'cancel',
				message: `Failed to cancel ${childJobId ? 'title' : 'operation'}: ${toUserMessage(cause)}`,
			});
		}
	}

	return {
		view: () => {
			rev();
			return {
				operations: deps.link.workOperations(),
				cancelPendingByOperationId,
				error,
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
			clearError('reveal');
			try {
				await tauriClient.revealPath(child.outputPath);
			} catch (cause) {
				setError({
					kind: 'reveal',
					message: `Failed to show the exported file: ${toUserMessage(cause)}`,
				});
			}
		},
		reset() {
			disposed = true;
		},
	};
}

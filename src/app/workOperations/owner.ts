import { createSignal, type Accessor } from 'solid-js';
import type { OperationId } from '../../types/workRuntime';
import type { RemoteSourceOwner } from '../remoteSource';
import {
	createWorkOperationsSession,
	emptyWorkOperationsView,
	type WorkOperationsView,
} from './runtime';

export type WorkOperationsOwner = {
	readonly view: Accessor<WorkOperationsView>;
	initialize(): Promise<void>;
	/** Cancels the whole operation, or only the title named by `childJobId`. */
	cancel(operationId: OperationId, childJobId?: string): Promise<void>;
	revealOutput(child: { outputPath?: string | null }): Promise<void>;
	reset(): void;
};

export type WorkOperationsOwnerDeps = {
	readonly remoteSource: Pick<RemoteSourceOwner, 'settleTerminalWork'>;
};

export function createWorkOperationsOwner(deps: WorkOperationsOwnerDeps): WorkOperationsOwner {
	let snapshot = emptyWorkOperationsView();
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	function publish(next: WorkOperationsView): void {
		snapshot = next;
		bump((n) => n + 1);
	}
	const session = createWorkOperationsSession(publish, deps);

	return {
		view: () => {
			rev();
			return snapshot;
		},
		initialize() {
			return session.initialize();
		},
		cancel(operationId, childJobId) {
			return session.cancel(operationId, childJobId);
		},
		revealOutput(child) {
			return session.revealOutput(child);
		},
		reset() {
			session.dispose();
			publish(emptyWorkOperationsView());
		},
	};
}

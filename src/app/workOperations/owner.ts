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
	cancel(operationId: OperationId): Promise<void>;
	openSource(child: { sourcePath?: string | null }): Promise<void>;
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
		cancel(operationId) {
			return session.cancel(operationId);
		},
		openSource(child) {
			return session.openSource(child);
		},
		reset() {
			session.dispose();
			publish(emptyWorkOperationsView());
		},
	};
}

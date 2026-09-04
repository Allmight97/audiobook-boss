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

export function createWorkOperationsOwner(deps: {
	readonly remoteSource: Pick<RemoteSourceOwner, 'settleTerminalWork'>;
}): WorkOperationsOwner {
	const [view, setView] = createSignal(emptyWorkOperationsView());
	const session = createWorkOperationsSession(setView, {
		settleTerminalWork: (input) => deps.remoteSource.settleTerminalWork(input),
	});

	return {
		view,
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
			setView(emptyWorkOperationsView());
		},
	};
}

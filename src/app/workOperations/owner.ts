import { createSignal, type Accessor } from 'solid-js';
import type { OperationId } from '../../types/workRuntime';
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

export function createWorkOperationsOwner(): WorkOperationsOwner {
	const [view, setView] = createSignal(emptyWorkOperationsView());
	const session = createWorkOperationsSession(setView);

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

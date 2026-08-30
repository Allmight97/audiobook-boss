import { createSignal, type Accessor } from 'solid-js';
import type { OperationId } from '../../types/workRuntime';
import {
	bindWorkOperationsPublisher,
	cancelWorkOperation,
	disposeWorkCenter,
	initializeWorkCenter,
	openChildSource,
	snapshotWorkOperationsView,
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
	const [view, setView] = createSignal(snapshotWorkOperationsView());
	bindWorkOperationsPublisher(setView);

	return {
		view,
		initialize() {
			return initializeWorkCenter();
		},
		cancel(operationId) {
			return cancelWorkOperation(operationId);
		},
		openSource(child) {
			return openChildSource(child);
		},
		reset() {
			disposeWorkCenter();
			setView(snapshotWorkOperationsView());
		},
	};
}

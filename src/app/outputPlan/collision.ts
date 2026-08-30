import { createSignal, type Accessor } from 'solid-js';
import type { CollisionPolicy, PlannedOutput, ProcessingPreflightPlan } from '../../types/audio';

export type CollisionView = {
	readonly isOpen: boolean;
	readonly outputs: ReadonlyArray<PlannedOutput>;
	readonly title: string;
	readonly body: string;
};

export function emptyCollisionView(): CollisionView {
	return {
		isOpen: false,
		outputs: [],
		title: 'Resolve Existing File Conflicts',
		body: '',
	};
}

function collisionBody(outputs: ReadonlyArray<PlannedOutput>): string {
	const count = outputs.length;
	if (count === 1) {
		return '1 file with the same name already exists in the target output folder. How do you want to resolve the conflict?';
	}
	return `${count} files with the same name already exist in the target output folders. How do you want to resolve the conflicts?`;
}

export type CollisionReview = {
	readonly view: Accessor<CollisionView>;
	open(plan: ProcessingPreflightPlan): Promise<CollisionPolicy | null>;
	choose(policy: CollisionPolicy): void;
	cancel(): void;
	reset(): void;
};

export function createCollisionReview(): CollisionReview {
	const [view, setView] = createSignal(emptyCollisionView());
	let pendingResolve: ((policy: CollisionPolicy | null) => void) | null = null;

	function settle(policy: CollisionPolicy | null): void {
		const resolve = pendingResolve;
		pendingResolve = null;
		setView(emptyCollisionView());
		resolve?.(policy);
	}

	return {
		view,
		open(plan) {
			if (pendingResolve) {
				pendingResolve(null);
			}
			const outputs = plan.outputs.filter((output) => output.collision != null);
			setView({
				isOpen: true,
				outputs,
				title: 'Resolve Existing File Conflicts',
				body: collisionBody(outputs),
			});
			return new Promise<CollisionPolicy | null>((resolve) => {
				pendingResolve = resolve;
			});
		},
		choose(policy) {
			settle(policy);
		},
		cancel() {
			settle(null);
		},
		reset() {
			settle(null);
		},
	};
}

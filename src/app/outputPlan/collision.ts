import type { CollisionPolicy, PlannedOutput, ProcessingPreflightPlan } from '../../types/audio';

export type CollisionView = {
	readonly isOpen: boolean;
	readonly outputs: ReadonlyArray<PlannedOutput>;
	readonly title: string;
	readonly body: string;
};

const listeners = new Set<() => void>();
let pendingResolve: ((policy: CollisionPolicy | null) => void) | null = null;
let collisionView: CollisionView = emptyCollisionView();

export function emptyCollisionView(): CollisionView {
	return {
		isOpen: false,
		outputs: [],
		title: 'Resolve Existing File Conflicts',
		body: '',
	};
}

export function getCollisionView(): CollisionView {
	return collisionView;
}

export function subscribeCollisionView(listener: () => void): () => void {
	listeners.add(listener);
	return () => {
		listeners.delete(listener);
	};
}

function publish(next: CollisionView): void {
	collisionView = next;
	for (const listener of listeners) listener();
}

function collisionBody(outputs: ReadonlyArray<PlannedOutput>): string {
	const count = outputs.length;
	if (count === 1) {
		return '1 file with the same name already exists in the target output folder. How do you want to resolve the conflict?';
	}
	return `${count} files with the same name already exist in the target output folders. How do you want to resolve the conflicts?`;
}

export function openCollisionDialog(
	plan: ProcessingPreflightPlan,
): Promise<CollisionPolicy | null> {
	if (pendingResolve) {
		pendingResolve(null);
	}

	const outputs = plan.outputs.filter((output) => output.collision != null);
	publish({
		isOpen: true,
		outputs,
		title: 'Resolve Existing File Conflicts',
		body: collisionBody(outputs),
	});

	return new Promise<CollisionPolicy | null>((resolve) => {
		pendingResolve = resolve;
	});
}

export function chooseCollisionPolicy(policy: CollisionPolicy): void {
	const resolve = pendingResolve;
	pendingResolve = null;
	publish(emptyCollisionView());
	resolve?.(policy);
}

export function cancelCollisionDialog(): void {
	const resolve = pendingResolve;
	pendingResolve = null;
	publish(emptyCollisionView());
	resolve?.(null);
}

export function resetCollisionDialog(): void {
	const resolve = pendingResolve;
	pendingResolve = null;
	publish(emptyCollisionView());
	resolve?.(null);
}

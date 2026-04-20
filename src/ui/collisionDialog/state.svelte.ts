import type { CollisionPolicy, PlannedOutput, ProcessingPreflightPlan } from '../../types/audio';

type CollisionDialogState = {
	isOpen: boolean;
	outputs: PlannedOutput[];
	title: string;
	body: string;
};

function createInitialState(): CollisionDialogState {
	return {
		isOpen: false,
		outputs: [],
		title: 'Resolve Existing File Conflicts',
		body: '',
	};
}

export const collisionDialogState = $state<CollisionDialogState>(createInitialState());

let pendingResolve: ((policy: CollisionPolicy | null) => void) | null = null;

function closeDialogState(): void {
	collisionDialogState.isOpen = false;
	collisionDialogState.outputs = [];
	collisionDialogState.title = 'Resolve Existing File Conflicts';
	collisionDialogState.body = '';
}

function buildBody(outputs: PlannedOutput[]): string {
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
	collisionDialogState.isOpen = true;
	collisionDialogState.outputs = outputs;
	collisionDialogState.body = buildBody(outputs);

	return new Promise<CollisionPolicy | null>((resolve) => {
		pendingResolve = resolve;
	});
}

export function chooseCollisionPolicy(policy: CollisionPolicy): void {
	const resolve = pendingResolve;
	pendingResolve = null;
	closeDialogState();
	resolve?.(policy);
}

export function cancelCollisionDialog(): void {
	const resolve = pendingResolve;
	pendingResolve = null;
	closeDialogState();
	resolve?.(null);
}

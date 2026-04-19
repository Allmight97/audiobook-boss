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
		title: 'Resolve Output Collisions',
		body: '',
	};
}

export const collisionDialogState = $state<CollisionDialogState>(createInitialState());

let pendingResolve: ((policy: CollisionPolicy | null) => void) | null = null;

function closeDialogState(): void {
	collisionDialogState.isOpen = false;
	collisionDialogState.outputs = [];
	collisionDialogState.title = 'Resolve Output Collisions';
	collisionDialogState.body = '';
}

function buildBody(outputs: PlannedOutput[]): string {
	const existingCount = outputs.filter(
		(output) => output.collision?.kind === 'existing_file',
	).length;
	const duplicateCount = outputs.filter(
		(output) => output.collision?.kind === 'batch_duplicate',
	).length;
	const previewCount = outputs.filter((output) => output.kind === 'preview').length;
	const parts: string[] = [];
	if (existingCount > 0) {
		parts.push(
			existingCount === 1
				? '1 existing output already occupies a destination path.'
				: `${existingCount} existing outputs already occupy destination paths.`,
		);
	}
	if (duplicateCount > 0) {
		parts.push(
			duplicateCount === 1
				? '1 output in this run collides with another planned output.'
				: `${duplicateCount} outputs in this run collide with other planned outputs.`,
		);
	}
	if (previewCount > 0) {
		parts.push(
			previewCount === 1
				? 'Preview artifact naming is included in this review.'
				: 'Preview artifact naming is included in this review.',
		);
	}
	parts.push('Choose one batch-wide policy before processing starts.');
	return parts.join(' ');
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

import type {
	ChildJobStatus,
	OperationKind,
	WorkOperationStatus,
	WorkProgressStage,
} from '../../lib/generated/tauri';

const STAGES: Record<WorkProgressStage, { readonly status: string; readonly activity: string }> = {
	pending: { status: 'Analyzing', activity: 'analyzing' },
	analyzing: { status: 'Analyzing', activity: 'analyzing' },
	converting: { status: 'Converting', activity: 'converting' },
	writing: { status: 'Writing Metadata', activity: 'writing' },
	cleaning: { status: 'Writing Metadata', activity: 'writing' },
	complete: { status: 'Completed', activity: 'running' },
	failed: { status: 'Failed', activity: 'running' },
	cancelled: { status: 'Cancelled', activity: 'running' },
};

const OPERATION_STATUSES: Record<WorkOperationStatus, string> = {
	accepted: 'Accepted',
	running: 'Running',
	cancelling: 'Cancelling',
	completed: 'Completed',
	cancelled: 'Cancelled',
	failed: 'Failed',
	mixed: 'Mixed result',
};

const CHILD_STATUSES: Record<ChildJobStatus, string> = {
	queued: 'Queued',
	running: 'Running',
	completed: 'Done',
	skipped: 'Skipped',
	cancelled: 'Cancelled',
	failed: 'Failed',
};

const OPERATION_KINDS: Record<OperationKind, string> = {
	processingBatch: 'Batch',
	metadataSave: 'Metadata',
};

export function stageStatusText(stage: WorkProgressStage): string {
	return STAGES[stage].status;
}

export function stageActivity(stage: WorkProgressStage): string {
	return STAGES[stage].activity;
}

export function operationStatusLabel(status: WorkOperationStatus): string {
	return OPERATION_STATUSES[status];
}

export function childStatusLabel(status: ChildJobStatus): string {
	return CHILD_STATUSES[status];
}

export function operationKindLabel(kind: OperationKind): string {
	return OPERATION_KINDS[kind];
}

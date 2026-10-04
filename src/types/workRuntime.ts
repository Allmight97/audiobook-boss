import type {
	ChildJobSnapshot as GeneratedChildJobSnapshot,
	ChildJobStatus as GeneratedChildJobStatus,
	OperationId as GeneratedOperationId,
	OperationKind as GeneratedOperationKind,
	OperationSnapshot as GeneratedOperationSnapshot,
	OperationTerminalSummary as GeneratedOperationTerminalSummary,
	ProgressSnapshot as GeneratedProgressSnapshot,
	ResourceLane as GeneratedResourceLane,
	WorkOperationStatus as GeneratedWorkOperationStatus,
	WorkOperationsSnapshot as GeneratedWorkOperationsSnapshot,
	WorkOperationsUpdate as GeneratedWorkOperationsUpdate,
	WorkProgressStage as GeneratedWorkProgressStage,
} from '../lib/generated/tauri';
import type { NullToOptionalDeep } from './ipc';

export type OperationId = GeneratedOperationId;
export type OperationKind = GeneratedOperationKind;
export type WorkOperationStatus = GeneratedWorkOperationStatus;
export type ChildJobStatus = GeneratedChildJobStatus;
export type WorkProgressStage = GeneratedWorkProgressStage;
export type ResourceLane = GeneratedResourceLane;

export type ProgressSnapshot = NullToOptionalDeep<GeneratedProgressSnapshot>;
export type ChildJobSnapshot = NullToOptionalDeep<GeneratedChildJobSnapshot>;
export type OperationTerminalSummary = NullToOptionalDeep<GeneratedOperationTerminalSummary>;
export type OperationSnapshot = Omit<NullToOptionalDeep<GeneratedOperationSnapshot>, 'children'> & {
	children: ChildJobSnapshot[];
};
export type WorkOperationsSnapshot = Omit<
	NullToOptionalDeep<GeneratedWorkOperationsSnapshot>,
	'operations'
> & {
	operations: OperationSnapshot[];
};
export type WorkOperationsUpdate = Omit<
	NullToOptionalDeep<GeneratedWorkOperationsUpdate>,
	'changed'
> & {
	changed: OperationSnapshot;
};

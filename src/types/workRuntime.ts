import type {
	ChildJobSnapshot as GeneratedChildJobSnapshot,
	OperationId as GeneratedOperationId,
	OperationSnapshot as GeneratedOperationSnapshot,
	WorkOperationsSnapshot as GeneratedWorkOperationsSnapshot,
	WorkOperationsUpdate as GeneratedWorkOperationsUpdate,
	WorkProgressStage as GeneratedWorkProgressStage,
} from '../lib/generated/tauri';
import type { NullToOptionalDeep } from './ipc';

export type OperationId = GeneratedOperationId;
export type WorkProgressStage = GeneratedWorkProgressStage;

export type ChildJobSnapshot = NullToOptionalDeep<GeneratedChildJobSnapshot>;
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

import type {
	ChildJobSnapshot as GeneratedChildJobSnapshot,
	ChildJobStatus as GeneratedChildJobStatus,
	OperationId as GeneratedOperationId,
	OperationKind as GeneratedOperationKind,
	OperationListSnapshot as GeneratedOperationListSnapshot,
	OperationSnapshot as GeneratedOperationSnapshot,
	OperationTerminalSummary as GeneratedOperationTerminalSummary,
	ProgressSnapshot as GeneratedProgressSnapshot,
	ResourceLane as GeneratedResourceLane,
	SubmitProcessingOperationRequest as GeneratedSubmitProcessingOperationRequest,
	WorkOperationStatus as GeneratedWorkOperationStatus,
	WorkProgressStage as GeneratedWorkProgressStage,
	WorkSubmissionAccepted as GeneratedWorkSubmissionAccepted,
} from '../lib/generated/tauri';
import type { ProcessPayload } from './audio';
import type { NullToOptionalDeep } from './ipc';
import type { MetadataIntentPatch } from './metadataIntent';

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
export type OperationListSnapshot = Omit<
	NullToOptionalDeep<GeneratedOperationListSnapshot>,
	'operations'
> & {
	operations: OperationSnapshot[];
};
export type WorkSubmissionAccepted = Omit<
	NullToOptionalDeep<GeneratedWorkSubmissionAccepted>,
	'snapshot'
> & {
	snapshot: OperationSnapshot;
};

export type MetadataIntentByPath = Record<string, MetadataIntentPatch>;

export type SubmitProcessingOperationRequest = Omit<
	NullToOptionalDeep<GeneratedSubmitProcessingOperationRequest>,
	'payload' | 'metadata'
> & {
	payload: ProcessPayload;
	metadataIntent?: MetadataIntentByPath | null;
	title?: string | null;
	previewSeconds?: number | null;
};

import type {
	EventStage as GeneratedEventStage,
	OperationKind as GeneratedOperationKind,
	ProgressEvent as GeneratedProgressEvent,
	QueueEvent as GeneratedQueueEvent,
} from '../lib/generated/tauri';
import type { NullToOptionalDeep } from './ipc';

/**
 * Frontend event contract for payloads that cross the Tauri runtime boundary.
 *
 * The specta-generated bindings in `src/lib/generated/tauri.ts` are the
 * canonical source for backend event shapes. This file re-exports those
 * payloads in UI-friendly form and adds the built-in Tauri file-drop events.
 */

export const EVENTS = {
	PROGRESS: 'processing-progress',
	QUEUE: 'processing-queue',
} as const;

export type EventStage = GeneratedEventStage;
export type OperationKind = GeneratedOperationKind;

/**
 * Exhaustive value-level access to the generated progress stages.
 *
 * This object must stay aligned with the Rust `EventStage` enum; if a stage is
 * added or removed in Rust, this declaration fails to type-check until the
 * frontend acknowledges the new variant.
 */
export const STAGES: { readonly [K in EventStage]: K } = {
	analyzing: 'analyzing',
	converting: 'converting',
	writing: 'writing',
	completed: 'completed',
	skipped: 'skipped',
	failed: 'failed',
	cancelled: 'cancelled',
} as const;

export const OPERATION_KINDS: { readonly [K in OperationKind]: K } = {
	processingMerge: 'processingMerge',
	processingBatch: 'processingBatch',
	metadataSave: 'metadataSave',
} as const;

export type ProcessingProgressEvent = NullToOptionalDeep<GeneratedProgressEvent>;
export type ProcessingQueueItem = NullToOptionalDeep<GeneratedQueueEvent>['items'][number];
export type ProcessingQueueEvent = NullToOptionalDeep<GeneratedQueueEvent>;

export interface TauriFileDropEvents {
	'tauri://drag-drop': { paths: string[]; position: { x: number; y: number } };
	'tauri://drag-enter': { paths: string[]; position: { x: number; y: number } };
	'tauri://drag-over': { position: { x: number; y: number } };
	'tauri://drag-leave': unknown;
}

export interface ApplicationEvents extends TauriFileDropEvents {
	[EVENTS.PROGRESS]: ProcessingProgressEvent;
	[EVENTS.QUEUE]: ProcessingQueueEvent;
}

export type EventName = keyof ApplicationEvents;
export type EventPayload<T extends EventName> = ApplicationEvents[T];

type DragDropPayload = TauriFileDropEvents['tauri://drag-drop'];

export function isFileDropEvent(event: unknown): event is DragDropPayload {
	const e = event as Partial<DragDropPayload>;
	return (
		typeof e === 'object' &&
		e !== null &&
		Array.isArray(e.paths) &&
		e.paths.every((item) => typeof item === 'string') &&
		typeof e.position === 'object' &&
		e.position !== null &&
		typeof e.position.x === 'number' &&
		typeof e.position.y === 'number'
	);
}

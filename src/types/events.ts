import type { OpenedAudioFilesEvent as GeneratedOpenedAudioFilesEvent } from '../lib/generated/tauri';
import type { NullToOptionalDeep } from './ipc';
import type { SettingsSnapshot } from './appSettings';
import type { SessionUpdate } from './session';
import type { WorkOperationsUpdate } from './workRuntime';

/**
 * Frontend event contract for payloads that cross the Tauri runtime boundary.
 *
 * The specta-generated bindings in `src/lib/generated/tauri.ts` are the
 * canonical source for backend event shapes. This file re-exports those
 * payloads in UI-friendly form and adds the built-in Tauri file-drop events.
 */

export const EVENTS = {
	OPENED_AUDIO_FILES: 'opened-audio-files',
	WORK_OPERATIONS_UPDATE: 'work-operations-update',
	SESSION_UPDATE: 'session-update',
	SETTINGS_UPDATE: 'settings-update',
} as const;

export type OpenedAudioFilesEvent = NullToOptionalDeep<GeneratedOpenedAudioFilesEvent>;
export type WorkOperationsUpdateEvent = WorkOperationsUpdate;
export type SessionUpdateEvent = SessionUpdate;
export type SettingsUpdateEvent = SettingsSnapshot;

export interface TauriFileDropEvents {
	'tauri://drag-drop': { paths: string[]; position: { x: number; y: number } };
	'tauri://drag-enter': { paths: string[]; position: { x: number; y: number } };
	'tauri://drag-over': { position: { x: number; y: number } };
	'tauri://drag-leave': unknown;
}

export interface ApplicationEvents extends TauriFileDropEvents {
	[EVENTS.OPENED_AUDIO_FILES]: OpenedAudioFilesEvent;
	[EVENTS.WORK_OPERATIONS_UPDATE]: WorkOperationsUpdateEvent;
	[EVENTS.SESSION_UPDATE]: SessionUpdateEvent;
	[EVENTS.SETTINGS_UPDATE]: SettingsUpdateEvent;
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

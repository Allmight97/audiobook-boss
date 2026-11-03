import type { SettingsSnapshot } from './appSettings';
import type { SessionUpdate } from './session';
import type { WorkOperationsUpdate } from './workRuntime';

export const EVENTS = {
	WORK_OPERATIONS_UPDATE: 'work-operations-update',
	SESSION_UPDATE: 'session-update',
	SETTINGS_UPDATE: 'settings-update',
} as const;

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
	[EVENTS.WORK_OPERATIONS_UPDATE]: WorkOperationsUpdateEvent;
	[EVENTS.SESSION_UPDATE]: SessionUpdateEvent;
	[EVENTS.SETTINGS_UPDATE]: SettingsUpdateEvent;
}

export type EventName = keyof ApplicationEvents;
export type EventPayload<T extends EventName> = ApplicationEvents[T];

type DragDropPayload = TauriFileDropEvents['tauri://drag-drop'];

export function isFileDropEvent(event: unknown): event is DragDropPayload {
	if (typeof event !== 'object' || event === null) return false;
	if (!('paths' in event) || !('position' in event)) return false;
	const { paths, position } = event;
	return (
		Array.isArray(paths) &&
		paths.every((item) => typeof item === 'string') &&
		typeof position === 'object' &&
		position !== null &&
		'x' in position &&
		'y' in position &&
		typeof position.x === 'number' &&
		typeof position.y === 'number'
	);
}

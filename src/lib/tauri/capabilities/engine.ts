import type { UnlistenFn } from '@tauri-apps/api/event';
import type { SettingsIntent, SettingsReply, SettingsSnapshot } from '../../../types/appSettings';
import { EVENTS } from '../../../types/events';
import type { SessionIntent, SessionReply, SessionUpdate } from '../../../types/session';
import type {
	OperationId,
	WorkOperationsSnapshot,
	WorkOperationsUpdate,
} from '../../../types/workRuntime';
import { tauriClient } from '../client';

export interface EngineCapability {
	attach: () => Promise<{
		client: number;
		session: SessionUpdate;
		settings: SettingsSnapshot;
		workOperations: WorkOperationsSnapshot;
	}>;
	sessionDispatch: (
		client: number,
		sequence: number,
		intent: SessionIntent,
	) => Promise<SessionReply>;
	/** Claims a finished preview's output and opens it, numbered with the session intents. */
	openPreviewOutput: (client: number, sequence: number, runId: string) => Promise<SessionReply>;
	settingsDispatch: (
		client: number,
		sequence: number,
		intent: SettingsIntent,
	) => Promise<SettingsReply>;
	listenSessionUpdates: (handler: (update: SessionUpdate) => void) => Promise<UnlistenFn>;
	/** Settings changed by something other than a settings intent. */
	listenSettingsUpdates: (handler: (snapshot: SettingsSnapshot) => void) => Promise<UnlistenFn>;
	listenWorkOperations: (handler: (update: WorkOperationsUpdate) => void) => Promise<UnlistenFn>;
	/** The Quit shortcut was pressed once; a second press inside `windowMs` quits. */
	listenQuitPressAgain: (handler: (windowMs: number) => void) => Promise<UnlistenFn>;
	/** Cancels the whole operation, or only the title named by `childJobId`. */
	cancelWorkOperation: (operationId: OperationId, childJobId?: string) => Promise<void>;
}

export const liveEngineCapability: EngineCapability = {
	attach: () => tauriClient.attachFrontend(),
	sessionDispatch: (client, sequence, intent) =>
		tauriClient.sessionDispatch(client, sequence, intent),
	openPreviewOutput: (client, sequence, runId) =>
		tauriClient.openPreviewOutput(client, sequence, runId),
	settingsDispatch: (client, sequence, intent) =>
		tauriClient.settingsDispatch(client, sequence, intent),
	listenSessionUpdates: (handler) =>
		tauriClient.listen(EVENTS.SESSION_UPDATE, (event) => {
			handler(event.payload);
		}),
	listenSettingsUpdates: (handler) =>
		tauriClient.listen(EVENTS.SETTINGS_UPDATE, (event) => {
			handler(event.payload);
		}),
	listenWorkOperations: (handler) =>
		tauriClient.listen(EVENTS.WORK_OPERATIONS_UPDATE, (event) => {
			handler(event.payload);
		}),
	listenQuitPressAgain: (handler) =>
		tauriClient.listen(EVENTS.QUIT_PRESS_AGAIN, (event) => {
			handler(event.payload.windowMs);
		}),
	cancelWorkOperation: (operationId, childJobId) =>
		tauriClient.cancelWorkOperation(operationId, childJobId),
};

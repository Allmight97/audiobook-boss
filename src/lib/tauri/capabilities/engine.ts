import type { UnlistenFn } from '@tauri-apps/api/event';
import type {
	FrontendAttachment,
	OperationId,
	SessionIntent,
	SessionReply,
	SessionUpdate,
	SettingsIntent,
	SettingsReply,
	SettingsSnapshot,
	WorkOperationsUpdate,
} from '../../generated/tauri';
import { tauriClient } from '../client';

export interface EngineCapability {
	attach: () => Promise<FrontendAttachment>;
	sessionDispatch: (
		client: number,
		sequence: number,
		intent: SessionIntent,
	) => Promise<SessionReply>;
	openPreviewOutput: (client: number, sequence: number, runId: string) => Promise<SessionReply>;
	settingsDispatch: (
		client: number,
		sequence: number,
		intent: SettingsIntent,
	) => Promise<SettingsReply>;
	listenSessionUpdates: (handler: (update: SessionUpdate) => void) => Promise<UnlistenFn>;
	listenSettingsUpdates: (handler: (snapshot: SettingsSnapshot) => void) => Promise<UnlistenFn>;
	listenWorkOperations: (handler: (update: WorkOperationsUpdate) => void) => Promise<UnlistenFn>;
	listenQuitPressAgain: (handler: (windowMs: number) => void) => Promise<UnlistenFn>;
	cancelWorkOperation: (operationId: OperationId, childJobId?: string) => Promise<void>;
}

export const liveEngineCapability: EngineCapability = {
	attach: () => tauriClient.commands.attachFrontend(),
	sessionDispatch: (client, sequence, intent) =>
		tauriClient.commands.sessionDispatch(client, sequence, intent),
	openPreviewOutput: (client, sequence, runId) =>
		tauriClient.commands.openPreviewOutput(client, sequence, runId),
	settingsDispatch: (client, sequence, intent) =>
		tauriClient.commands.settingsDispatch(client, sequence, intent),
	listenSessionUpdates: (handler) =>
		tauriClient.events.sessionUpdate.listen((event) => {
			handler(event.payload);
		}),
	listenSettingsUpdates: (handler) =>
		tauriClient.events.settingsUpdate.listen((event) => {
			handler(event.payload);
		}),
	listenWorkOperations: (handler) =>
		tauriClient.events.workOperationsUpdate.listen((event) => {
			handler(event.payload);
		}),
	listenQuitPressAgain: (handler) =>
		tauriClient.events.quitPressAgain.listen((event) => {
			handler(event.payload.windowMs);
		}),
	cancelWorkOperation: async (operationId, childJobId) => {
		await tauriClient.commands.cancelWorkOperation(operationId, childJobId ?? null);
	},
};

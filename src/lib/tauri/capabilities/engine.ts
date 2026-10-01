import type { UnlistenFn } from '@tauri-apps/api/event';
import type { SettingsIntent, SettingsReply, SettingsSnapshot } from '../../../types/appSettings';
import { EVENTS } from '../../../types/events';
import type { MetadataIntentPatch } from '../../../types/metadataIntent';
import type { SessionIntent, SessionReply, SessionUpdate } from '../../../types/session';
import { tauriClient } from '../client';

/** The engine's session and settings as the frontend reaches them. */
export interface EngineCapability {
	attach(): Promise<{ client: number; session: SessionUpdate; settings: SettingsSnapshot }>;
	sessionDispatch(client: number, sequence: number, intent: SessionIntent): Promise<SessionReply>;
	settingsDispatch(
		client: number,
		sequence: number,
		intent: SettingsIntent,
	): Promise<SettingsReply>;
	sessionCoverArt(): Promise<number[] | null>;
	sessionMetadataIntents(filePaths: string[]): Promise<Record<string, MetadataIntentPatch>>;
	listenSessionUpdates(handler: (update: SessionUpdate) => void): Promise<UnlistenFn>;
	/** Settings changed by something other than a settings intent. */
	listenSettingsUpdates(handler: (snapshot: SettingsSnapshot) => void): Promise<UnlistenFn>;
}

export const liveEngineCapability: EngineCapability = {
	attach: () => tauriClient.attachFrontend(),
	sessionDispatch: (client, sequence, intent) =>
		tauriClient.sessionDispatch(client, sequence, intent),
	settingsDispatch: (client, sequence, intent) =>
		tauriClient.settingsDispatch(client, sequence, intent),
	sessionCoverArt: () => tauriClient.sessionCoverArt(),
	sessionMetadataIntents: (filePaths) => tauriClient.sessionMetadataIntents(filePaths),
	listenSessionUpdates: (handler) =>
		tauriClient.listen(EVENTS.SESSION_UPDATE, (event) => {
			handler(event.payload);
		}),
	listenSettingsUpdates: (handler) =>
		tauriClient.listen(EVENTS.SETTINGS_UPDATE, (event) => {
			handler(event.payload);
		}),
};

import type { RuntimeSettingsCapabilities } from '../../../types/audio';
import { tauriClient } from '../client';

export interface SettingsCapability {
	getRuntimeSettingsCapabilities(): Promise<RuntimeSettingsCapabilities>;
}

export const liveSettingsCapability: SettingsCapability = {
	getRuntimeSettingsCapabilities: () => tauriClient.getRuntimeSettingsCapabilities(),
};

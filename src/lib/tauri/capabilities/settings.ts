import type {
	AppSettings,
	AppSettingsPatch,
	AppSettingsRecoveryPlan,
	AppSettingsRecoveryResult,
} from '../../../types/appSettings';
import type { RuntimeSettingsCapabilities } from '../../../types/audio';
import { tauriClient } from '../client';

export interface SettingsCapability {
	getAppSettings(): Promise<AppSettings>;
	getAppSettingsRecovery(): Promise<AppSettingsRecoveryPlan | null>;
	recoverAppSettings(expected: AppSettingsRecoveryPlan): Promise<AppSettingsRecoveryResult>;
	updateAppSettings(patch: AppSettingsPatch): Promise<AppSettings>;
	resetAppSettings(): Promise<AppSettings>;
	getMaxConcurrentJobs(): Promise<number>;
	setMaxConcurrentJobs(maxConcurrent: number | null): Promise<number>;
	getRuntimeSettingsCapabilities(): Promise<RuntimeSettingsCapabilities>;
}

export const liveSettingsCapability: SettingsCapability = {
	getAppSettings: () => tauriClient.getAppSettings(),
	getAppSettingsRecovery: () => tauriClient.getAppSettingsRecovery(),
	recoverAppSettings: (expected) => tauriClient.recoverAppSettings(expected),
	updateAppSettings: (patch) => tauriClient.updateAppSettings(patch),
	resetAppSettings: () => tauriClient.resetAppSettings(),
	getMaxConcurrentJobs: () => tauriClient.getMaxConcurrentJobs(),
	setMaxConcurrentJobs: (maxConcurrent) => tauriClient.setMaxConcurrentJobs(maxConcurrent),
	getRuntimeSettingsCapabilities: () => tauriClient.getRuntimeSettingsCapabilities(),
};

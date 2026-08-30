import type { AppSettings, AppSettingsPatch } from '../../../types/appSettings';
import type { RuntimeSettingsCapabilities } from '../../../types/audio';
import { tauriClient } from '../client';

export interface SettingsOpenFileOptions {
	readonly title?: string;
}

export interface SettingsCapability {
	getAppSettings(): Promise<AppSettings>;
	updateAppSettings(patch: AppSettingsPatch): Promise<AppSettings>;
	resetAppSettings(): Promise<AppSettings>;
	openFile(options?: SettingsOpenFileOptions): Promise<string | null>;
	getMaxConcurrentJobs(): Promise<number>;
	setMaxConcurrentJobs(maxConcurrent: number | null): Promise<number>;
	getRuntimeSettingsCapabilities(): Promise<RuntimeSettingsCapabilities>;
}

export const liveSettingsCapability: SettingsCapability = {
	getAppSettings: () => tauriClient.getAppSettings(),
	updateAppSettings: (patch) => tauriClient.updateAppSettings(patch),
	resetAppSettings: () => tauriClient.resetAppSettings(),
	openFile: (options) => tauriClient.openFile(options ? { title: options.title } : undefined),
	getMaxConcurrentJobs: () => tauriClient.getMaxConcurrentJobs(),
	setMaxConcurrentJobs: (maxConcurrent) => tauriClient.setMaxConcurrentJobs(maxConcurrent),
	getRuntimeSettingsCapabilities: () => tauriClient.getRuntimeSettingsCapabilities(),
};

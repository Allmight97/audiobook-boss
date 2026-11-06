import type {
	AcquisitionSnapshot as GeneratedAcquisitionJob,
	AcquisitionProgress as GeneratedAcquisitionProgress,
	ProviderId as GeneratedProviderId,
	RemoteRelease as GeneratedRemoteRelease,
	RemoteSourceAccountState as GeneratedRemoteSourceAccountState,
	RemoteSourceProviderCapabilities as GeneratedRemoteSourceProviderCapabilities,
	RemoteTitle as GeneratedRemoteTitle,
} from '../lib/generated/tauri';
import type { NullToOptionalDeep } from './ipc';

export type ProviderId = GeneratedProviderId;
export type RemoteSourceProviderCapabilities =
	NullToOptionalDeep<GeneratedRemoteSourceProviderCapabilities>;
export type RemoteSourceAccountState = NullToOptionalDeep<GeneratedRemoteSourceAccountState>;
export type RemoteTitle = NullToOptionalDeep<GeneratedRemoteTitle>;
export type AcquisitionJob = NullToOptionalDeep<GeneratedAcquisitionJob>;
export type AcquisitionProgress = NullToOptionalDeep<GeneratedAcquisitionProgress>;
export type RemoteRelease = NullToOptionalDeep<GeneratedRemoteRelease>;

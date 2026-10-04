import type {
	AcquisitionSnapshot as GeneratedAcquisitionJob,
	AcquisitionProgress as GeneratedAcquisitionProgress,
	ProviderId as GeneratedProviderId,
	RemoteIndexerConnectionTestResult as GeneratedRemoteIndexerConnectionTestResult,
	RemoteRelease as GeneratedRemoteRelease,
	RemoteSourceAccountState as GeneratedRemoteSourceAccountState,
	RemoteSourceProviderCapabilities as GeneratedRemoteSourceProviderCapabilities,
	RemoteTitle as GeneratedRemoteTitle,
	RemoteTitleAvailabilityStatus as GeneratedRemoteTitleAvailabilityStatus,
	SupplementalAsset as GeneratedSupplementalAsset,
} from '../lib/generated/tauri';
import type { NullToOptionalDeep } from './ipc';

export type ProviderId = GeneratedProviderId;
export type RemoteSourceProviderCapabilities =
	NullToOptionalDeep<GeneratedRemoteSourceProviderCapabilities>;
export type RemoteSourceAccountState = NullToOptionalDeep<GeneratedRemoteSourceAccountState>;
export type RemoteTitle = NullToOptionalDeep<GeneratedRemoteTitle>;
export type RemoteTitleAvailabilityStatus = GeneratedRemoteTitleAvailabilityStatus;
export type AcquisitionJob = NullToOptionalDeep<GeneratedAcquisitionJob>;
export type AcquisitionProgress = NullToOptionalDeep<GeneratedAcquisitionProgress>;
export type SupplementalAsset = NullToOptionalDeep<GeneratedSupplementalAsset>;
export type RemoteRelease = NullToOptionalDeep<GeneratedRemoteRelease>;
export type RemoteIndexerConnectionTestResult =
	NullToOptionalDeep<GeneratedRemoteIndexerConnectionTestResult>;

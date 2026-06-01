import type {
	AcquisitionJob as GeneratedAcquisitionJob,
	AcquisitionPlan as GeneratedAcquisitionPlan,
	ProviderId as GeneratedProviderId,
	RemoteAuthCompletionRequest as GeneratedRemoteAuthCompletionRequest,
	RemoteAuthStartResponse as GeneratedRemoteAuthStartResponse,
	RemoteLibraryResponse as GeneratedRemoteLibraryResponse,
	RemoteSourceAccountState as GeneratedRemoteSourceAccountState,
	RemoteSourceProviderCapabilities as GeneratedRemoteSourceProviderCapabilities,
	SupplementalAsset as GeneratedSupplementalAsset,
} from '../lib/generated/tauri';
import type { NullToOptionalDeep } from './ipc';

export type ProviderId = GeneratedProviderId;
export type RemoteSourceProviderCapabilities =
	NullToOptionalDeep<GeneratedRemoteSourceProviderCapabilities>;
export type RemoteSourceAccountState = NullToOptionalDeep<GeneratedRemoteSourceAccountState>;
export type RemoteAuthStartResponse = NullToOptionalDeep<GeneratedRemoteAuthStartResponse>;
export type RemoteAuthCompletionRequest = NullToOptionalDeep<GeneratedRemoteAuthCompletionRequest>;
export type RemoteLibraryResponse = NullToOptionalDeep<GeneratedRemoteLibraryResponse>;
export type AcquisitionPlan = NullToOptionalDeep<GeneratedAcquisitionPlan>;
export type AcquisitionJob = NullToOptionalDeep<GeneratedAcquisitionJob>;
export type SupplementalAsset = NullToOptionalDeep<GeneratedSupplementalAsset>;

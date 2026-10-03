import { tauriClient } from '../../lib/tauri/client';
import type { ProviderId } from '../../types/remoteSource';
import type { RemoteSourceWorkflowServices } from './workflow';

export function makeProductionRemoteSourceServices(): RemoteSourceWorkflowServices {
	return {
		listProviders: () => tauriClient.listRemoteSourceProviders(),
		getAccountState: (providerId: ProviderId) =>
			tauriClient.getRemoteSourceAccountState(providerId),
		startAuth: (providerId: ProviderId) => tauriClient.startRemoteSourceAuth(providerId),
		openAuthorizationUrl: (url) => tauriClient.openUrl(url),
		completeAuth: (providerId, responseUrlHandoffPath) =>
			tauriClient.completeRemoteSourceAuth({
				providerId,
				responseUrlHandoffPath,
			}),
		logout: (providerId: ProviderId) => tauriClient.logoutRemoteSourceAccount(providerId),
		loadLibrary: (providerId: ProviderId) => tauriClient.loadRemoteSourceLibrary(providerId),
	};
}

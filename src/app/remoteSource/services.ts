import { tauriClient } from '../../lib/tauri/client';
import type {
	ProviderId,
	RemoteReleaseGrabRequest,
	RemoteReleaseSearchRequest,
} from '../../types/remoteSource';
import { EVENTS } from '../../types/events';
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
		searchReleases: (request: RemoteReleaseSearchRequest) =>
			tauriClient.searchRemoteSourceReleases(request),
		grabRelease: (request: RemoteReleaseGrabRequest) =>
			tauriClient.grabRemoteSourceRelease(request),
		startAcquisition: (providerId, selections) =>
			tauriClient.startRemoteSourceAcquisition({
				providerId,
				selections: [...selections],
			}),
		getAcquisitionStatus: (jobId) => tauriClient.getRemoteSourceAcquisitionStatus(jobId),
		cancelAcquisition: (jobId) => tauriClient.cancelRemoteSourceAcquisition(jobId),
		listenAcquisitions: (handler) =>
			tauriClient.listen(EVENTS.ACQUISITION_UPDATE, ({ payload }) => handler(payload)),
	};
}

export function makeProductionIndexerConnectionServices() {
	return {
		getIndexerConnection: () => tauriClient.getRemoteSourceIndexerConnection(),
		updateIndexerConnection: (
			update: Parameters<typeof tauriClient.updateRemoteSourceIndexerConnection>[0],
		) => tauriClient.updateRemoteSourceIndexerConnection(update),
		testIndexerConnection: (
			update: Parameters<typeof tauriClient.updateRemoteSourceIndexerConnection>[0],
		) => tauriClient.testRemoteSourceIndexerConnection(update),
	};
}

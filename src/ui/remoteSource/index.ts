export type { CompanionAssetSummary } from '../../app/remoteSource';
export {
	companionSummaryForInputIds,
	hasSupplementalAssetsForInputId,
	subscribeRemoteSourceSupplementalAssets,
	purgeRemoteSourceSessionsForInputIds,
	registerRemoteSourceSupplementalAssets,
	releaseRemoteSourceSessionRetainers,
	retainRemoteSourceSessionsForInputIds,
	supplementalAssetsByInputIdForProcessing,
} from '../../app/remoteSource';

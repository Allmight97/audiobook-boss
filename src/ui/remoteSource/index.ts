export type { CompanionAssetSummary } from '../../app/remoteSource';
export {
	companionSummaryForInputIds,
	hasSupplementalAssetsForInputId,
	purgeRemoteSourceSessionsForInputIds,
	registerRemoteSourceSupplementalAssets,
	releaseRemoteSourceSessionRetainers,
	retainRemoteSourceSessionsForInputIds,
	supplementalAssetsByInputIdForProcessing,
} from '../../app/remoteSource';

export type { CompanionAssetSummary } from './sessionAssets.svelte';
export {
	companionSummaryForInputIds,
	hasSupplementalAssetsForInputId,
	purgeRemoteSourceSessionsForInputIds,
	releaseRemoteSourceSessionRetainers,
	retainRemoteSourceSessionsForInputIds,
	registerRemoteSourceSupplementalAssets,
	supplementalAssetsByInputIdForProcessing,
} from './sessionAssets.svelte';

export type { CompanionAssetSummary } from './sessionAssets.svelte';
export { openRemoteSourceAcquire } from './state.svelte';
export {
	companionSummaryForInputIds,
	hasSupplementalAssetsForInputId,
	purgeRemoteSourceSessionsForInputIds,
	releaseRemoteSourceSessionRetainers,
	retainRemoteSourceSessionsForInputIds,
	registerRemoteSourceSupplementalAssets,
	supplementalAssetsByInputIdForProcessing,
} from './sessionAssets.svelte';

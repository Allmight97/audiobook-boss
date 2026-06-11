import type { MetadataLookupWorkflowAction } from './metadataLookup/metadataLookupWorkflow';
import { clearMetadataLookupQueue, metadataLookupState } from './metadataLookup/state.svelte';

async function runLookupWorkflow(action: MetadataLookupWorkflowAction): Promise<void> {
	const { MetadataLookupWorkflowLive, runMetadataLookupWorkflow } = await import(
		'./metadataLookup/metadataLookupWorkflow'
	);
	return runMetadataLookupWorkflow(MetadataLookupWorkflowLive, action);
}

export async function applyMetadataLookupResult(index: number): Promise<void> {
	await runLookupWorkflow({ type: 'applyResult', index });
}

export async function searchMetadataLookup(): Promise<void> {
	await runLookupWorkflow({ type: 'search' });
}

export async function skipMetadataLookupQueueItem(): Promise<void> {
	await runLookupWorkflow({ type: 'skipQueueItem' });
}

export function closeMetadataLookup(): void {
	void runLookupWorkflow({ type: 'close' });
}

export function useManualMetadataEntryFromLookup(): void {
	void runLookupWorkflow({ type: 'manualEntry' });
}

export function openMetadataLookup(): void {
	void runLookupWorkflow({ type: 'open' });
}

export function initMetadataLookup(): void {
	metadataLookupState.isOpen = false;
	metadataLookupState.results = [];
	metadataLookupState.hasSearched = false;
	metadataLookupState.statusMessage = '';
	clearMetadataLookupQueue();
}

import type { AudioFile } from '../../types/audio';
import type { OnlineMetadataResult } from '../../types/metadata';
import { Data, Effect, type AppEffect, runAppEffect } from '../../lib/effect/appEffect';
import {
	buildQueueMetadataPatch,
	deriveQueryFromFile,
	getApplyMode,
	mapResultToMetadata,
	persistQueueMetadata,
	resetResults,
	restoreCoverArtForFile,
	selectedSources,
	updateApplyModeOptions,
	updateQueueContext,
	type QueueCoverState,
	type QueueItemState,
} from './metadataLookupWorkflowDomain';
import {
	MetadataLookupWorkflowServicesTag,
	type MetadataLookupWorkflowLayer,
	type MetadataLookupWorkflowServices,
	type MetadataLookupWorkflowServicesId,
} from './metadataLookupWorkflowServices';

export {
	MetadataLookupWorkflowServicesTag,
	makeMetadataLookupWorkflowServicesLayer,
	type MetadataLookupWorkflowLayer,
	type MetadataLookupWorkflowServices,
	type MetadataLookupWorkflowServicesId,
} from './metadataLookupWorkflowServices';

const METADATA_TITLE_INPUT_ID = 'meta-title';

type QueueAdvanceReason = 'applied' | 'skipped';

export type MetadataLookupWorkflowAction =
	| { type: 'applyResult'; index: number }
	| { type: 'close' }
	| { type: 'init' }
	| { type: 'manualEntry' }
	| { type: 'open' }
	| { type: 'search' }
	| { type: 'skipQueueItem' };

export class MetadataLookupWorkflowFailed extends Data.TaggedError('MetadataLookupWorkflowFailed')<{
	readonly message: string;
	readonly cause: unknown;
}> {}

function workflowFailure(message: string, cause: unknown): MetadataLookupWorkflowFailed {
	return new MetadataLookupWorkflowFailed({ message, cause });
}

function workflowSync<A>(
	evaluate: () => A,
	message: string,
): AppEffect<A, MetadataLookupWorkflowFailed> {
	return Effect.try({
		try: evaluate,
		catch: (cause) => workflowFailure(message, cause),
	});
}

function workflowPromise<A>(
	evaluate: () => PromiseLike<A>,
	message: string,
): AppEffect<A, MetadataLookupWorkflowFailed> {
	return Effect.tryPromise({
		try: evaluate,
		catch: (cause) => workflowFailure(message, cause),
	});
}

function refreshOutputForMetadataChange(services: MetadataLookupWorkflowServices): void {
	services.updateOutputPath('final');
	services.updateEstimatedSize();
}

function setStatus(
	services: MetadataLookupWorkflowServices,
	message: string,
	variant: 'error' | 'success' | 'info' = 'info',
): void {
	const state = services.getLookupState();
	state.statusMessage = message;
	state.statusVariant = variant;
}

function showModal(services: MetadataLookupWorkflowServices): void {
	services.getLookupState().isOpen = true;
}

function hideModal(services: MetadataLookupWorkflowServices): void {
	services.getLookupState().isOpen = false;
}

async function advanceQueue(
	services: MetadataLookupWorkflowServices,
	reason: QueueAdvanceReason,
): Promise<void> {
	const { queue, index } = services.getQueueState();
	if (queue.length === 0) return;

	if (index >= queue.length - 1) {
		restoreCoverArtForFile(services, queue[index]?.file ?? null);
		setStatus(services, 'Queue complete.', 'success');
		return;
	}

	services.clearCoverArt();
	services.setMetadataLookupQueueIndex(index + 1);
	updateQueueContext(services);

	const nextItem = services.getQueueState().queue[index + 1];
	if (nextItem) {
		await services.selectFile(
			nextItem.index,
			{ multi: false, range: false },
			{ skipPersistPrevious: true },
		);
		services.getLookupState().query = deriveQueryFromFile(services, nextItem.file);
	}

	resetResults(services);
	const message =
		reason === 'applied'
			? 'Metadata applied. Ready for next search.'
			: 'Skipped. Ready for next search.';
	setStatus(services, message, reason === 'applied' ? 'success' : 'info');
}

async function applyCoverArt(
	services: MetadataLookupWorkflowServices,
	result: OnlineMetadataResult,
): Promise<number[] | null> {
	if (!result.coverUrl) return null;
	try {
		const coverBytes = await services.loadCoverArtFromUrl(result.coverUrl);
		services.setCustomCoverArt(coverBytes);
		return coverBytes;
	} catch (error) {
		services.console.warn('Failed to load cover art from lookup:', error);
		setStatus(services, 'Cover art failed to load from source.', 'error');
		return null;
	}
}

async function applyResult(
	services: MetadataLookupWorkflowServices,
	result: OnlineMetadataResult,
): Promise<void> {
	const queue = services.getQueueState().queue;
	if (queue.length === 0) {
		setStatus(services, 'Select at least one file before applying metadata.', 'error');
		return;
	}

	const metadata = mapResultToMetadata(result);
	const mode = getApplyMode(services);

	const current = queue[services.getQueueState().index];
	if (current) {
		await services.selectFile(
			current.index,
			{ multi: false, range: false },
			{ skipPersistPrevious: true },
		);
	}

	services.applyMetadataToForm(metadata, { mode: 'single', markDirty: true });
	let queueCoverState: QueueCoverState = { intent: 'keep' };
	if (services.getLookupState().replaceCoverArt) {
		const coverBytes = await applyCoverArt(services, result);
		if (coverBytes && coverBytes.length > 0) {
			queueCoverState = { intent: 'replace', bytes: coverBytes };
		}
	}
	refreshOutputForMetadataChange(services);
	services.updateTagPreview();

	if (mode === 'queue') {
		if (current) {
			const queueState: QueueItemState = {
				metadataPatch: buildQueueMetadataPatch(services),
				cover: queueCoverState,
			};
			persistQueueMetadata(services, current.file, queueState);
		}
		await advanceQueue(services, 'applied');
		return;
	}

	setStatus(services, 'Metadata applied to form.', 'success');
}

async function runSearch(services: MetadataLookupWorkflowServices): Promise<void> {
	const state = services.getLookupState();
	const query = state.query.trim();
	if (!query) {
		setStatus(services, 'Enter a title, author, or ASIN to search.', 'error');
		return;
	}

	setStatus(services, 'Searching metadata sources…', 'info');

	try {
		const results = await services.searchOnlineMetadata({
			query,
			sources: selectedSources(services),
			limit: 8,
		});
		state.results = results;
		state.hasSearched = true;
		setStatus(services, `Found ${results.length} results.`, 'success');
	} catch (error) {
		services.console.error('Metadata lookup failed:', error);
		state.results = [];
		state.hasSearched = false;
		setStatus(services, 'Search failed. Check your query and try again.', 'error');
	}
}

function openWorkflow(services: MetadataLookupWorkflowServices): void {
	const selectedIndices = Array.from(services.getSelectedFileIndices()).sort((a, b) => a - b);
	const fileList = services.getCurrentFileList();
	const queue = selectedIndices
		.map((index) => {
			const file = fileList?.files[index];
			if (!file?.isValid) return null;
			return { file, index };
		})
		.filter((item): item is { file: AudioFile; index: number } => Boolean(item));
	services.setMetadataLookupQueue(queue);

	const state = services.getLookupState();
	if (services.getQueueState().queue.length === 0) {
		state.query = '';
		setStatus(services, 'Select a valid file to search metadata.', 'error');
	} else {
		state.query = deriveQueryFromFile(services, services.getQueueState().queue[0].file);
		setStatus(services, '', 'info');
	}

	updateQueueContext(services);
	updateApplyModeOptions(services);
	resetResults(services);
	state.replaceCoverArt = false;

	showModal(services);
}

function initWorkflow(services: MetadataLookupWorkflowServices): void {
	const state = services.getLookupState();
	state.isOpen = false;
	state.results = [];
	state.hasSearched = false;
	state.statusMessage = '';
	services.clearMetadataLookupQueue();
}

function useManualEntry(services: MetadataLookupWorkflowServices): void {
	hideModal(services);
	services.queueMicrotask(() => {
		services.focusElementById(METADATA_TITLE_INPUT_ID);
	});
}

function reportWorkflowFailure(
	services: MetadataLookupWorkflowServices,
	error: MetadataLookupWorkflowFailed,
): AppEffect<void> {
	return Effect.sync(() => {
		services.console.error(`Metadata lookup workflow failed: ${error.message}`, error.cause);
		setStatus(services, 'Metadata lookup failed. Check console and try again.', 'error');
	});
}

function metadataLookupWorkflowBody(
	action: MetadataLookupWorkflowAction,
): AppEffect<void, MetadataLookupWorkflowFailed, MetadataLookupWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* MetadataLookupWorkflowServicesTag;
		switch (action.type) {
			case 'applyResult': {
				yield* workflowPromise(async () => {
					const result = services.getLookupState().results[action.index];
					if (!result) return;
					await applyResult(services, result);
				}, 'Failed to apply metadata lookup result.');
				return;
			}
			case 'close':
				yield* workflowSync(() => hideModal(services), 'Failed to close metadata lookup.');
				return;
			case 'init':
				yield* workflowSync(() => initWorkflow(services), 'Failed to initialize metadata lookup.');
				return;
			case 'manualEntry':
				yield* workflowSync(
					() => useManualEntry(services),
					'Failed to switch to manual metadata entry.',
				);
				return;
			case 'open':
				yield* workflowSync(() => openWorkflow(services), 'Failed to open metadata lookup.');
				return;
			case 'search':
				yield* workflowPromise(
					() => runSearch(services),
					'Failed to search online metadata sources.',
				);
				return;
			case 'skipQueueItem':
				yield* workflowPromise(
					() => advanceQueue(services, 'skipped'),
					'Failed to skip metadata lookup queue item.',
				);
				return;
		}
	});
}

export function metadataLookupWorkflowExecution(
	action: MetadataLookupWorkflowAction,
): AppEffect<void, MetadataLookupWorkflowFailed, MetadataLookupWorkflowServicesId> {
	return metadataLookupWorkflowBody(action);
}

export function metadataLookupWorkflowProgram(
	action: MetadataLookupWorkflowAction,
): AppEffect<void, never, MetadataLookupWorkflowServicesId> {
	return metadataLookupWorkflowBody(action).pipe(
		Effect.catchAll((error) =>
			Effect.gen(function* () {
				const services = yield* MetadataLookupWorkflowServicesTag;
				yield* reportWorkflowFailure(services, error);
			}),
		),
	);
}

export function runMetadataLookupWorkflow(
	layer: MetadataLookupWorkflowLayer,
	action: MetadataLookupWorkflowAction,
): Promise<void> {
	return runAppEffect(metadataLookupWorkflowProgram(action).pipe(Effect.provide(layer)));
}

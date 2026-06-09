import type { AudioFile } from '../../types/audio';
import type { OnlineMetadataResult } from '../../types/metadata';
import {
	Data,
	Effect,
	type AppEffect,
	runAppEffect,
	workflowTryPromise,
	workflowTrySync,
} from '../../lib/effect/appEffect';
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
import { getCachedMetadataLookupCoverBytes } from './metadataLookupCoverPreview.svelte';

export {
	MetadataLookupWorkflowServicesTag,
	makeMetadataLookupWorkflowServicesLayer,
	type MetadataLookupWorkflowLayer,
	type MetadataLookupWorkflowServices,
	type MetadataLookupWorkflowServicesId,
} from './metadataLookupWorkflowServices';

const METADATA_TITLE_INPUT_ID = 'meta-title';

type QueueAdvanceReason = 'applied' | 'skipped';

type QueueAdvanceOptions = {
	readonly coverArtFailed?: boolean;
};

type SearchStatusOptions = {
	readonly successPrefix?: string;
	readonly successVariant?: 'error' | 'success' | 'info';
	readonly failurePrefix?: string;
};

type CoverArtApplyResult =
	| { readonly status: 'applied'; readonly bytes: number[] }
	| { readonly status: 'failed' }
	| { readonly status: 'notRequested' };

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
	return workflowTrySync(evaluate, message, workflowFailure);
}

function workflowPromise<A>(
	evaluate: () => PromiseLike<A>,
	message: string,
): AppEffect<A, MetadataLookupWorkflowFailed> {
	return workflowTryPromise(evaluate, message, workflowFailure);
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
	options: QueueAdvanceOptions = {},
): Promise<void> {
	const { queue, index } = services.getQueueState();
	if (queue.length === 0) return;

	if (index >= queue.length - 1) {
		restoreCoverArtForFile(services, queue[index]?.file ?? null);
		setStatus(
			services,
			options.coverArtFailed ? 'Queue complete, but cover art failed to load.' : 'Queue complete.',
			options.coverArtFailed ? 'error' : 'success',
		);
		return;
	}

	const nextIndex = index + 1;
	const nextItem = queue[nextIndex];

	services.refreshCoverArtDisplay();

	if (nextItem) {
		await services.selectFile(
			nextItem.index,
			{ multi: false, range: false },
			{ skipPersistPrevious: true },
		);
		services.setMetadataLookupQueueIndex(nextIndex);
		updateQueueContext(services);
		services.getLookupState().query = deriveQueryFromFile(services, nextItem.file);
	}

	resetResults(services);
	const prefix =
		reason === 'applied'
			? options.coverArtFailed
				? 'Metadata applied, but cover art failed to load. '
				: 'Metadata applied. '
			: 'Skipped. ';
	await runSearch(services, {
		successPrefix: prefix,
		successVariant: options.coverArtFailed ? 'error' : undefined,
		failurePrefix: prefix,
	});
}

async function applyCoverArt(
	services: MetadataLookupWorkflowServices,
	result: OnlineMetadataResult,
): Promise<CoverArtApplyResult> {
	if (!result.coverUrl) return { status: 'notRequested' };
	try {
		const cachedBytes = getCachedMetadataLookupCoverBytes(result.coverUrl);
		const coverBytes = cachedBytes ?? (await services.loadCoverArtFromUrl(result.coverUrl));
		services.setCustomCoverArt(coverBytes);
		return { status: 'applied', bytes: coverBytes };
	} catch (error) {
		services.console.warn('Failed to load cover art from lookup:', error);
		return { status: 'failed' };
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
	let coverArtFailed = false;
	if (services.getLookupState().replaceCoverArt) {
		const coverArtResult = await applyCoverArt(services, result);
		if (coverArtResult.status === 'applied' && coverArtResult.bytes.length > 0) {
			queueCoverState = { intent: 'replace', bytes: coverArtResult.bytes };
		} else if (coverArtResult.status === 'failed') {
			coverArtFailed = true;
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
		await advanceQueue(services, 'applied', { coverArtFailed });
		return;
	}

	setStatus(
		services,
		coverArtFailed
			? 'Metadata applied to form, but cover art failed to load.'
			: 'Metadata applied to form.',
		coverArtFailed ? 'error' : 'success',
	);
}

async function runSearch(
	services: MetadataLookupWorkflowServices,
	options: SearchStatusOptions = {},
): Promise<void> {
	const state = services.getLookupState();
	const query = state.query.trim();
	if (!query) {
		setStatus(services, 'Enter a title, author, or ASIN to search.', 'error');
		return;
	}

	setStatus(services, 'Searching metadata sources…', 'info');

	try {
		const response = await services.searchOnlineMetadata({
			query,
			sources: selectedSources(services),
			limit: 8,
		});
		const { results, diagnostics } = response;
		state.results = results;
		state.hasSearched = true;
		const resultStatus =
			diagnostics.length > 0
				? `Found ${results.length} results. Some lookup data was unavailable; showing available results.`
				: `Found ${results.length} results.`;
		setStatus(
			services,
			`${options.successPrefix ?? ''}${resultStatus}`,
			options.successVariant ?? (diagnostics.length > 0 ? 'info' : 'success'),
		);
	} catch (error) {
		services.console.error('Metadata lookup failed:', error);
		state.results = [];
		state.hasSearched = false;
		setStatus(
			services,
			`${options.failurePrefix ?? ''}Search failed. Check your query and try again.`,
			'error',
		);
	}
}

async function openWorkflow(services: MetadataLookupWorkflowServices): Promise<void> {
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
	if (services.getQueueState().queue.length > 0) {
		await runSearch(services);
	}
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
				yield* workflowPromise(() => openWorkflow(services), 'Failed to open metadata lookup.');
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

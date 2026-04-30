import { tauriClient } from '../lib/tauri/client';
import type { AudioFile } from '../types/audio';
import type { AudiobookMetadata, MetadataSource, OnlineMetadataResult } from '../types/metadata';
import type { MetadataIntentPatch } from '../types/metadataIntent';
import { applyMetadataDraftIntent, buildMetadataDraftIntent } from './metadataDraft';
import { applyMetadataToForm, readMetadataForm } from './metadataForm';
import { updateEstimatedSize, updateOutputPath } from './outputPanel';
import { updateTagPreview } from './tagPreview';
import { clearCoverArt, setCoverArt, setCustomCoverArt } from './coverArt';
import { getMetadataForFile, setMetadataForFile } from './metadataState';
import { getCurrentFileList } from './fileList';
import { selectFile } from './fileList/actions';
import { getSelectedFileIndices } from './fileList/state';
import {
	clearMetadataLookupQueue,
	metadataLookupQueueState,
	setMetadataLookupQueue,
	setMetadataLookupQueueIndex,
	metadataLookupState,
	type MetadataLookupApplyMode,
	type MetadataLookupStatusVariant,
} from './metadataLookup/state.svelte';
const METADATA_TITLE_INPUT_ID = 'meta-title';

type ApplyMode = MetadataLookupApplyMode;

type LookupQueueItem = {
	file: AudioFile;
	index: number;
};

type QueueCoverState = { intent: 'keep' } | { intent: 'replace'; bytes: number[] };

type QueueItemState = {
	metadataPatch: MetadataIntentPatch;
	cover: QueueCoverState;
};

function refreshOutputForMetadataChange(): void {
	updateOutputPath('final');
	updateEstimatedSize();
}

function setStatus(message: string, variant: MetadataLookupStatusVariant = 'info'): void {
	metadataLookupState.statusMessage = message;
	metadataLookupState.statusVariant = variant;
}

function showModal(): void {
	metadataLookupState.isOpen = true;
}

function hideModal(): void {
	metadataLookupState.isOpen = false;
}

function focusMetadataTitleInput(): void {
	const titleInput = document.getElementById(METADATA_TITLE_INPUT_ID);
	if (titleInput instanceof HTMLElement) {
		titleInput.focus();
	}
}

function formatFileName(path: string): string {
	return path.split(/[\\/]/).pop() ?? path;
}

function isTrackLikeTitle(value: string): boolean {
	return /^(chapter|track|disc|disk|episode)\s*\d+/i.test(value.trim());
}

function deriveQueryFromFile(file: AudioFile): string {
	const stored = getMetadataForFile(file.path) ?? {};
	const title = stored.title?.trim();
	const album = stored.album?.trim();
	const artist = stored.artist?.trim();
	const composer = stored.composer?.trim();

	const queryTitle = album && (!title || isTrackLikeTitle(title)) ? album : (title ?? album);

	const parts: string[] = [];
	if (queryTitle) parts.push(queryTitle);
	if (artist) {
		parts.push(artist);
	} else if (composer) {
		parts.push(composer);
	}
	if (parts.length > 0) return parts.join(' ');

	const rawName = formatFileName(file.path).replace(/\.[^.]+$/, '');
	const cleaned = rawName.replace(/[._-]+/g, ' ').trim();
	return cleaned.replace(/^\d+\s*/, '').trim();
}

function updateQueueContext(): void {
	const { queue, index } = metadataLookupQueueState;
	if (queue.length === 0) {
		metadataLookupState.queueContext = 'No files selected.';
		return;
	}

	const current = queue[index];
	const label = `${index + 1} of ${queue.length}`;
	metadataLookupState.queueContext = `${label} • ${formatFileName(current.file.path)}`;
}

function updateApplyModeOptions(): void {
	const multi = metadataLookupQueueState.queue.length > 1;
	metadataLookupState.isQueueMode = multi;
	metadataLookupState.applyMode = multi ? 'queue' : 'current';
	metadataLookupState.skipEnabled = multi;
}

function getApplyMode(): ApplyMode {
	return metadataLookupState.applyMode;
}

function resetResults(): void {
	metadataLookupState.results = [];
	metadataLookupState.hasSearched = false;
}

function buildQueueMetadataPatch(): MetadataIntentPatch {
	return buildMetadataDraftIntent(readMetadataForm({ mode: 'single', includeCoverArt: false }));
}

function persistQueueMetadata(file: AudioFile, state: QueueItemState): void {
	if (!file.isValid) return;
	const existing = getMetadataForFile(file.path) ?? {};
	const merged: Partial<AudiobookMetadata> = applyMetadataDraftIntent(
		existing,
		state.metadataPatch,
	);
	const intentPatch: MetadataIntentPatch = { ...state.metadataPatch };
	if (state.cover.intent === 'replace') {
		merged.cover_art = state.cover.bytes;
		intentPatch.cover_art = { op: 'set', value: state.cover.bytes };
	}

	setMetadataForFile(file.path, merged, {
		markPending: true,
		intentPatch,
	});
}

function restoreCoverArtForFile(file: AudioFile | null): void {
	clearCoverArt();
	if (!file?.isValid) return;
	const stored = getMetadataForFile(file.path);
	setCoverArt(stored?.cover_art ?? null);
}

async function advanceQueue(reason: 'applied' | 'skipped'): Promise<void> {
	const { queue, index } = metadataLookupQueueState;
	if (queue.length === 0) return;

	if (index >= queue.length - 1) {
		restoreCoverArtForFile(queue[index]?.file ?? null);
		setStatus('Queue complete.', 'success');
		return;
	}

	clearCoverArt();
	setMetadataLookupQueueIndex(index + 1);
	updateQueueContext();

	const nextItem = metadataLookupQueueState.queue[index + 1];
	if (nextItem) {
		await selectFile(nextItem.index, { multi: false, range: false }, { skipPersistPrevious: true });
		metadataLookupState.query = deriveQueryFromFile(nextItem.file);
	}

	resetResults();
	const message =
		reason === 'applied'
			? 'Metadata applied. Ready for next search.'
			: 'Skipped. Ready for next search.';
	setStatus(message, reason === 'applied' ? 'success' : 'info');
}

function mapResultToMetadata(result: OnlineMetadataResult): Partial<AudiobookMetadata> {
	const metadata: Partial<AudiobookMetadata> = {
		title: result.title,
	};

	if (result.authors.length > 0) {
		metadata.artist = result.authors.join(', ');
	}
	if (result.narrators.length > 0) {
		metadata.composer = result.narrators.join(', ');
	}
	if (result.series) {
		metadata.series = result.series;
	}
	if (result.seriesPart) {
		metadata.series_part = result.seriesPart;
	}
	if (result.subseries) {
		metadata.subseries = result.subseries;
	}
	if (result.subseriesPart) {
		metadata.subseries_part = result.subseriesPart;
	}
	if (result.description) {
		metadata.description = result.description;
	}
	if (result.publishedDate) {
		metadata.date = result.publishedDate;
	}
	metadata.album = result.title;

	return metadata;
}

async function applyCoverArt(result: OnlineMetadataResult): Promise<number[] | null> {
	if (!result.coverUrl) return null;
	try {
		const coverBytes = await tauriClient.loadCoverArtFromUrl(result.coverUrl);
		setCustomCoverArt(coverBytes);
		return coverBytes;
	} catch (error) {
		console.warn('Failed to load cover art from lookup:', error);
		setStatus('Cover art failed to load from source.', 'error');
		return null;
	}
}

async function applyResult(result: OnlineMetadataResult): Promise<void> {
	const queue = metadataLookupQueueState.queue;
	if (queue.length === 0) {
		setStatus('Select at least one file before applying metadata.', 'error');
		return;
	}

	const metadata = mapResultToMetadata(result);
	const mode = getApplyMode();

	const current = queue[metadataLookupQueueState.index];
	if (current) {
		await selectFile(current.index, { multi: false, range: false }, { skipPersistPrevious: true });
	}

	applyMetadataToForm(metadata, { mode: 'single', markDirty: true });
	let queueCoverState: QueueCoverState = { intent: 'keep' };
	if (metadataLookupState.replaceCoverArt) {
		const coverBytes = await applyCoverArt(result);
		if (coverBytes && coverBytes.length > 0) {
			queueCoverState = { intent: 'replace', bytes: coverBytes };
		}
	}
	refreshOutputForMetadataChange();
	updateTagPreview();

	if (mode === 'queue') {
		if (current) {
			const queueState: QueueItemState = {
				metadataPatch: buildQueueMetadataPatch(),
				cover: queueCoverState,
			};
			persistQueueMetadata(current.file, queueState);
		}
		await advanceQueue('applied');
		return;
	}

	setStatus('Metadata applied to form.', 'success');
}

async function runSearch(): Promise<void> {
	const query = metadataLookupState.query.trim();
	if (!query) {
		setStatus('Enter a title, author, or ASIN to search.', 'error');
		return;
	}

	// Convert UI source selection to backend sources
	const selectedSource = metadataLookupState.source;
	let sources: MetadataSource[] | null = null;

	if (selectedSource === 'auto') {
		sources = ['audnexus', 'openlibrary'];
	} else if (selectedSource) {
		sources = [selectedSource];
	}

	setStatus('Searching metadata sources…', 'info');

	try {
		const results = await tauriClient.searchOnlineMetadata({ query, sources, limit: 8 });
		metadataLookupState.results = results;
		metadataLookupState.hasSearched = true;
		setStatus(`Found ${results.length} results.`, 'success');
	} catch (error) {
		console.error('Metadata lookup failed:', error);
		metadataLookupState.results = [];
		metadataLookupState.hasSearched = false;
		setStatus('Search failed. Check your query and try again.', 'error');
	}
}

export async function applyMetadataLookupResult(index: number): Promise<void> {
	const result = metadataLookupState.results[index];
	if (!result) return;
	await applyResult(result);
}

export async function searchMetadataLookup(): Promise<void> {
	await runSearch();
}

export async function skipMetadataLookupQueueItem(): Promise<void> {
	await advanceQueue('skipped');
}

export function closeMetadataLookup(): void {
	hideModal();
}

export function useManualMetadataEntryFromLookup(): void {
	hideModal();
	queueMicrotask(() => {
		focusMetadataTitleInput();
	});
}

export function openMetadataLookup(): void {
	const selectedIndices = Array.from(getSelectedFileIndices()).sort((a, b) => a - b);
	const fileList = getCurrentFileList();
	const queue = selectedIndices
		.map((index) => {
			const file = fileList?.files[index];
			if (!file?.isValid) return null;
			return { file, index };
		})
		.filter((item): item is LookupQueueItem => Boolean(item));
	setMetadataLookupQueue(queue);

	if (metadataLookupQueueState.queue.length === 0) {
		metadataLookupState.query = '';
		setStatus('Select a valid file to search metadata.', 'error');
	} else {
		metadataLookupState.query = deriveQueryFromFile(metadataLookupQueueState.queue[0].file);
		setStatus('', 'info');
	}

	updateQueueContext();
	updateApplyModeOptions();
	resetResults();
	metadataLookupState.replaceCoverArt = false;

	showModal();
}

export function initMetadataLookup(): void {
	metadataLookupState.isOpen = false;
	metadataLookupState.results = [];
	metadataLookupState.hasSearched = false;
	metadataLookupState.statusMessage = '';
	clearMetadataLookupQueue();
}

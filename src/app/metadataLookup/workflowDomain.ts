import { pathBasename } from '../../lib/path/basename';
import type { AudioFile } from '../../types/audio';
import type { AudiobookMetadata, MetadataSource, OnlineMetadataResult } from '../../types/metadata';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { buildMetadataDraftIntent } from '../metadataSession';
import { clearMetadataLookupCoverPreviewCache } from './coverPreview';
import type { MetadataLookupWorkflowServices } from './workflow';

export type QueueCoverState = { intent: 'keep' } | { intent: 'replace'; bytes: number[] };

export type QueueItemState = {
	metadataPatch: MetadataIntentPatch;
	cover: QueueCoverState;
};

function formatFileName(path: string): string {
	return pathBasename(path, { fallback: 'path' });
}

function isTrackLikeTitle(value: string): boolean {
	return /^(chapter|track|disc|disk|episode)\s*\d+/i.test(value.trim());
}

/** Title criterion from the file's stored metadata: title (album when the
 * title is track-like), falling back to a cleaned filename. */
export function deriveTitleQueryFromFile(
	services: MetadataLookupWorkflowServices,
	file: AudioFile,
): string {
	const stored = services.getMetadataForFile(file.path) ?? {};
	const title = stored.title?.trim();
	const album = stored.album?.trim();

	const queryTitle = album && (!title || isTrackLikeTitle(title)) ? album : (title ?? album);
	if (queryTitle) return queryTitle;

	const rawName = formatFileName(file.path).replace(/\.[^.]+$/, '');
	const cleaned = rawName.replace(/[._-]+/g, ' ').trim();
	return cleaned.replace(/^\d+\s*/, '').trim();
}

/** Author criterion from the file's stored metadata: artist, falling back to
 * composer (narrator). Empty when the file carries neither. */
export function deriveAuthorQueryFromFile(
	services: MetadataLookupWorkflowServices,
	file: AudioFile,
): string {
	const stored = services.getMetadataForFile(file.path) ?? {};
	return stored.artist?.trim() || stored.composer?.trim() || '';
}

export function updateQueueContext(services: MetadataLookupWorkflowServices): void {
	const { queue, index } = services.getQueueState();
	const state = services.getLookupState();
	if (queue.length === 0) {
		state.queueContext = 'No files selected.';
		return;
	}

	const current = queue[index];
	const label = `${index + 1} of ${queue.length}`;
	state.queueContext = `${label} • ${formatFileName(current.file.path)}`;
}

export function updateApplyModeOptions(services: MetadataLookupWorkflowServices): void {
	const multi = services.getQueueState().queue.length > 1;
	const state = services.getLookupState();
	state.isQueueMode = multi;
	state.applyMode = multi ? 'queue' : 'current';
	state.skipEnabled = multi;
}

export function resetResults(services: MetadataLookupWorkflowServices): void {
	const state = services.getLookupState();
	state.results = [];
	state.hasSearched = false;
	clearMetadataLookupCoverPreviewCache();
}

export function buildQueueMetadataPatch(
	services: MetadataLookupWorkflowServices,
): MetadataIntentPatch {
	return buildMetadataDraftIntent(
		services.readMetadataForm({ mode: 'single', includeCoverArt: false }),
	);
}

export function persistQueueMetadata(
	services: MetadataLookupWorkflowServices,
	file: AudioFile,
	state: QueueItemState,
): void {
	if (!file.isValid) return;
	const intentPatch: MetadataIntentPatch = { ...state.metadataPatch };
	if (state.cover.intent === 'replace') {
		intentPatch.cover_art = { op: 'set', value: state.cover.bytes };
	}

	services.stageMetadataIntentPatch(file.path, intentPatch);
}

export function mapResultToMetadata(result: OnlineMetadataResult): Partial<AudiobookMetadata> {
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

export function selectedSources(services: MetadataLookupWorkflowServices): MetadataSource[] {
	const source = services.getLookupState().source;
	if (source === 'auto') {
		return ['audnexus', 'openlibrary'];
	}
	return [source];
}

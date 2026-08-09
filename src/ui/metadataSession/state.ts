import { SvelteMap } from 'svelte/reactivity';
import type { AudiobookMetadata } from '../../types/metadata';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import {
	applyMetadataDraftIntent,
	hasActionableMetadataDraftIntent,
	mergeMetadataDraftIntents,
} from './draft';

const metadataByFile = new SvelteMap<string, Partial<AudiobookMetadata>>();
const metadataIntentByFile = new Map<string, MetadataIntentPatch>();
const pendingSavePaths = new Set<string>();

const isNullish = (value: unknown): value is null | undefined => value == null;

function metadataValuesEqual(a: unknown, b: unknown): boolean {
	if (isNullish(a) && isNullish(b)) {
		return true;
	}
	if (Array.isArray(a) && Array.isArray(b)) {
		if (a.length !== b.length) {
			return false;
		}
		for (const [index, entry] of a.entries()) {
			const candidate = b[index];
			const entryIsObject = typeof entry === 'object' && entry !== null;
			const candidateIsObject = typeof candidate === 'object' && candidate !== null;
			if (!entryIsObject && !candidateIsObject) {
				if (isNullish(entry) && isNullish(candidate)) {
					continue;
				}
				if (entry !== candidate) {
					return false;
				}
				continue;
			}
			if (!metadataValuesEqual(entry, candidate)) {
				return false;
			}
		}
		return true;
	}
	return a === b;
}

function metadataEqualsNullish(
	a: Partial<AudiobookMetadata>,
	b: Partial<AudiobookMetadata>,
): boolean {
	const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
	for (const key of keys) {
		const aValue = a[key as keyof AudiobookMetadata];
		const bValue = b[key as keyof AudiobookMetadata];
		if (!metadataValuesEqual(aValue, bValue)) {
			return false;
		}
	}
	return true;
}

/**
 * Caches metadata truth from a backend read. Carries no pending-save
 * semantics; staging intent goes through `stageMetadataIntentPatch`.
 */
export function cacheMetadataForFile(filePath: string, metadata: Partial<AudiobookMetadata>): void {
	metadataByFile.set(filePath, metadata);
}

export function getMetadataForFile(filePath: string): Partial<AudiobookMetadata> | undefined {
	return metadataByFile.get(filePath);
}

export function isUsableMetadataCache(
	metadata: Partial<AudiobookMetadata> | undefined,
): metadata is Partial<AudiobookMetadata> {
	if (!metadata) return false;

	const populatedKeys = Object.entries(metadata).filter(([, value]) => value !== undefined);
	if (populatedKeys.length === 0) return false;

	// Cover-only cache entries come from autoUpdateCoverArtFromFirstValidFile and must not
	// short-circuit full metadata reads on selection or batch hydration.
	if (populatedKeys.length === 1 && populatedKeys[0]?.[0] === 'cover_art') {
		return false;
	}

	return true;
}

export type MetadataStageResult = 'staged' | 'unchanged' | 'noop';

/**
 * The single seam for creating pending metadata truth: merges the intent
 * patch into the cached metadata, records the merged intent, and marks the
 * file pending for save.
 *
 * - `'noop'`: the patch carries no actionable ops; nothing happens.
 * - `'unchanged'`: the cache already reflects the merge (the write would be
 *   redundant against disk truth); existing pending state is left intact.
 * - `'staged'`: cache, stored intent, and the pending marker all updated.
 */
export function stageMetadataIntentPatch(
	filePath: string,
	intentPatch: MetadataIntentPatch,
): MetadataStageResult {
	if (!hasActionableMetadataDraftIntent(intentPatch)) {
		return 'noop';
	}
	const existing = metadataByFile.get(filePath) ?? {};
	const merged = applyMetadataDraftIntent(existing, intentPatch);
	if (metadataEqualsNullish(existing, merged)) {
		return 'unchanged';
	}
	metadataByFile.set(filePath, merged);
	const existingIntent = metadataIntentByFile.get(filePath) ?? {};
	metadataIntentByFile.set(filePath, mergeMetadataDraftIntents(existingIntent, intentPatch));
	pendingSavePaths.add(filePath);
	return 'staged';
}

export function getMetadataIntentPatchForFile(filePath: string): MetadataIntentPatch | undefined {
	return metadataIntentByFile.get(filePath);
}

/**
 * Stored actionable intent for the given paths, keyed by path; `null` when
 * nothing actionable is stored. This is the read half of the staging seam —
 * processing payloads and save batches both drain from it.
 */
export function collectActionableMetadataIntent(
	filePaths: readonly string[],
): Record<string, MetadataIntentPatch> | null {
	const collected: Record<string, MetadataIntentPatch> = {};
	for (const filePath of filePaths) {
		const patch = metadataIntentByFile.get(filePath);
		if (patch && hasActionableMetadataDraftIntent(patch)) {
			collected[filePath] = patch;
		}
	}
	return Object.keys(collected).length > 0 ? collected : null;
}

/** Owner-internal: the save workflow drains these entries into a batch save. */
export function getPendingMetadataIntentEntries(): Array<[string, MetadataIntentPatch]> {
	return Array.from(pendingSavePaths)
		.map((filePath) => [filePath, metadataIntentByFile.get(filePath)] as const)
		.filter((entry): entry is [string, MetadataIntentPatch] => Boolean(entry[1]));
}

/** Owner-internal: successful saves clear per-file pending truth. */
export function clearPendingMetadataForFile(filePath: string): void {
	pendingSavePaths.delete(filePath);
	metadataIntentByFile.delete(filePath);
}

export function removeMetadataForFile(filePath: string): void {
	metadataByFile.delete(filePath);
	metadataIntentByFile.delete(filePath);
	pendingSavePaths.delete(filePath);
}

export function clearMetadataSession(): void {
	metadataByFile.clear();
	metadataIntentByFile.clear();
	pendingSavePaths.clear();
}

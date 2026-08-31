import type { AudiobookMetadata } from '../../types/metadata';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import {
	applyMetadataDraftIntent,
	hasActionableMetadataDraftIntent,
	mergeMetadataDraftIntents,
} from './draft';

export type CachedCoverDisplay =
	| { status: 'staged'; handleId: string; dataUrl: string }
	| { status: 'embedded'; dataUrl: string }
	| { status: 'cleared' };

const metadataByFile = new Map<string, Partial<AudiobookMetadata>>();
const metadataIntentByFile = new Map<string, MetadataIntentPatch>();
const coverDisplayByFile = new Map<string, CachedCoverDisplay>();
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

export function cacheMetadataForFile(filePath: string, metadata: Partial<AudiobookMetadata>): void {
	metadataByFile.set(filePath, metadata);
}

export function getMetadataForFile(filePath: string): Partial<AudiobookMetadata> | undefined {
	return metadataByFile.get(filePath);
}

export function cacheCoverDisplayForFile(
	filePath: string,
	display: CachedCoverDisplay | null,
): void {
	if (!display) {
		coverDisplayByFile.delete(filePath);
		return;
	}
	coverDisplayByFile.set(filePath, display);
}

export function getCoverDisplayForFile(filePath: string): CachedCoverDisplay | undefined {
	return coverDisplayByFile.get(filePath);
}

export function isUsableMetadataCache(
	metadata: Partial<AudiobookMetadata> | undefined,
): metadata is Partial<AudiobookMetadata> {
	if (!metadata) return false;

	const populatedKeys = Object.entries(metadata).filter(([, value]) => value !== undefined);
	if (populatedKeys.length === 0) return false;

	if (populatedKeys.length === 1 && populatedKeys[0]?.[0] === 'cover_art') {
		return false;
	}

	return true;
}

export type MetadataStageResult = 'staged' | 'unchanged' | 'noop';

export function stageMetadataIntentPatch(
	filePath: string,
	intentPatch: MetadataIntentPatch,
): MetadataStageResult {
	if (!hasActionableMetadataDraftIntent(intentPatch)) {
		return 'noop';
	}
	const existing = metadataByFile.get(filePath) ?? {};
	const merged = applyMetadataDraftIntent(existing, intentPatch);
	const existingIntent = metadataIntentByFile.get(filePath) ?? {};
	const incomingCover = coverIntentKey(intentPatch);
	const coverChanged = incomingCover !== 'noop' && incomingCover !== coverIntentKey(existingIntent);
	if (metadataEqualsNullish(existing, merged) && !coverChanged) {
		return 'unchanged';
	}
	metadataByFile.set(filePath, merged);
	metadataIntentByFile.set(filePath, mergeMetadataDraftIntents(existingIntent, intentPatch));
	pendingSavePaths.add(filePath);
	return 'staged';
}

function coverIntentKey(patch: MetadataIntentPatch): string {
	const cover = patch.cover_art;
	if (!cover || cover.op === 'noop') {
		return 'noop';
	}
	if (cover.op === 'clear') {
		return 'clear';
	}
	return `set:${cover.value}`;
}

export function getMetadataIntentPatchForFile(filePath: string): MetadataIntentPatch | undefined {
	return metadataIntentByFile.get(filePath);
}

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

export function getPendingMetadataIntentEntries(): Array<[string, MetadataIntentPatch]> {
	return Array.from(pendingSavePaths)
		.map((filePath) => [filePath, metadataIntentByFile.get(filePath)] as const)
		.filter((entry): entry is [string, MetadataIntentPatch] => Boolean(entry[1]));
}

export function clearPendingMetadataForFile(filePath: string): void {
	pendingSavePaths.delete(filePath);
	metadataIntentByFile.delete(filePath);
}

export function removeMetadataForFile(filePath: string): void {
	metadataByFile.delete(filePath);
	metadataIntentByFile.delete(filePath);
	coverDisplayByFile.delete(filePath);
	pendingSavePaths.delete(filePath);
}

export function clearMetadataSession(): void {
	metadataByFile.clear();
	metadataIntentByFile.clear();
	coverDisplayByFile.clear();
	pendingSavePaths.clear();
}

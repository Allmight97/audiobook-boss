import type { AudiobookMetadata } from '../types/metadata';
import type { MetadataIntentPatch } from '../types/metadataIntent';
import {
	buildMetadataDraftIntent,
	hasActionableMetadataDraftIntent,
	mergeMetadataDraftIntents,
} from './metadataDraft';

const metadataByFile = new Map<string, Partial<AudiobookMetadata>>();
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

type SetMetadataOptions = {
	markPending?: boolean;
	intentPatch?: MetadataIntentPatch;
};

export function setMetadataForFile(
	filePath: string,
	metadata: Partial<AudiobookMetadata>,
	options?: SetMetadataOptions,
): void {
	metadataByFile.set(filePath, metadata);
	if (options?.markPending) {
		const nextPatch = options.intentPatch ?? buildMetadataDraftIntent(metadata);
		if (hasActionableMetadataDraftIntent(nextPatch)) {
			const existing = metadataIntentByFile.get(filePath) ?? {};
			metadataIntentByFile.set(filePath, mergeMetadataDraftIntents(existing, nextPatch));
		}
		pendingSavePaths.add(filePath);
	}
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

export function getAllMetadata(): Record<string, Partial<AudiobookMetadata>> {
	return Object.fromEntries(metadataByFile.entries());
}

export function getMetadataIntentPatchForFile(filePath: string): MetadataIntentPatch | undefined {
	return metadataIntentByFile.get(filePath);
}

export function getAllMetadataIntentPatches(): Record<string, MetadataIntentPatch> {
	return Object.fromEntries(metadataIntentByFile.entries());
}

export function metadataEqualsNullish(
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

export function getPendingMetadataEntries(): Array<[string, Partial<AudiobookMetadata>]> {
	return Array.from(pendingSavePaths)
		.map((filePath) => [filePath, metadataByFile.get(filePath)] as const)
		.filter((entry): entry is [string, Partial<AudiobookMetadata>] => Boolean(entry[1]));
}

export function getPendingMetadataIntentEntries(): Array<[string, MetadataIntentPatch]> {
	return Array.from(pendingSavePaths)
		.map((filePath) => [filePath, metadataIntentByFile.get(filePath)] as const)
		.filter((entry): entry is [string, MetadataIntentPatch] => Boolean(entry[1]));
}

export function hasPendingMetadataChanges(): boolean {
	return pendingSavePaths.size > 0;
}

export function clearPendingMetadataForFile(filePath: string): void {
	pendingSavePaths.delete(filePath);
	metadataIntentByFile.delete(filePath);
}

export function clearPendingMetadataForFiles(filePaths: string[]): void {
	filePaths.forEach((filePath) => {
		pendingSavePaths.delete(filePath);
		metadataIntentByFile.delete(filePath);
	});
}

export function removeMetadataForFile(filePath: string): void {
	metadataByFile.delete(filePath);
	metadataIntentByFile.delete(filePath);
	pendingSavePaths.delete(filePath);
}

export function clearMetadataState(): void {
	metadataByFile.clear();
	metadataIntentByFile.clear();
	pendingSavePaths.clear();
}

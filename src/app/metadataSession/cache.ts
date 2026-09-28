import type { AudiobookMetadata } from '../../types/metadata';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import {
	applyMetadataIntentPatch,
	hasActionableMetadataIntentPatch,
	mergeMetadataIntentPatches,
} from '../../types/metadataIntent';

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

function isUsableMetadataCache(
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

type MetadataStageResult = 'staged' | 'unchanged' | 'empty';

/**
 * Per-file session metadata: the tags last read from each file and the
 * changes the user asked for. What callers read is derived from both, so the
 * form can never show a value that save or processing will not send.
 */
export function createMetadataCache() {
	const sourceByFile = new Map<string, Partial<AudiobookMetadata>>();
	const intentByFile = new Map<string, MetadataIntentPatch>();

	function effectiveMetadata(filePath: string): Partial<AudiobookMetadata> | undefined {
		const source = sourceByFile.get(filePath);
		const intent = intentByFile.get(filePath);
		return intent ? applyMetadataIntentPatch(source ?? {}, intent) : source;
	}

	function removeMetadataForFile(filePath: string): void {
		sourceByFile.delete(filePath);
		intentByFile.delete(filePath);
	}

	return {
		/** Records tags as read from the file; pending changes still apply on top. */
		recordSourceMetadata(filePath: string, metadata: Partial<AudiobookMetadata>): void {
			sourceByFile.set(filePath, metadata);
		},
		/** Whether this session holds a usable read of the file's own tags. */
		hasSourceMetadata(filePath: string): boolean {
			return isUsableMetadataCache(sourceByFile.get(filePath));
		},
		/** The file's tags with this session's pending changes applied. */
		getMetadataForFile: effectiveMetadata,
		stageMetadataIntentPatch(
			filePath: string,
			intentPatch: MetadataIntentPatch,
		): MetadataStageResult {
			if (!hasActionableMetadataIntentPatch(intentPatch)) {
				return 'empty';
			}
			const current = effectiveMetadata(filePath) ?? {};
			if (metadataEqualsNullish(current, applyMetadataIntentPatch(current, intentPatch))) {
				return 'unchanged';
			}
			const pending = intentByFile.get(filePath) ?? {};
			intentByFile.set(filePath, mergeMetadataIntentPatches(pending, intentPatch));
			return 'staged';
		},
		getMetadataIntentPatchForFile(filePath: string): MetadataIntentPatch | undefined {
			return intentByFile.get(filePath);
		},
		collectActionableMetadataIntent(
			filePaths: readonly string[],
		): Record<string, MetadataIntentPatch> | null {
			const collected: Record<string, MetadataIntentPatch> = {};
			for (const filePath of filePaths) {
				const patch = intentByFile.get(filePath);
				if (patch && hasActionableMetadataIntentPatch(patch)) {
					collected[filePath] = patch;
				}
			}
			return Object.keys(collected).length > 0 ? collected : null;
		},
		getPendingMetadataIntentEntries(): Array<[string, MetadataIntentPatch]> {
			return Array.from(intentByFile.entries());
		},
		/**
		 * Accepts `saved` as written to the file: it folds into the recorded
		 * source tags and stops being pending. A change staged after `saved` was
		 * submitted keeps the whole pending patch for the next save.
		 */
		commitSavedIntent(filePath: string, saved: MetadataIntentPatch): void {
			if (intentByFile.get(filePath) !== saved) return;
			const source = sourceByFile.get(filePath);
			if (source) sourceByFile.set(filePath, applyMetadataIntentPatch(source, saved));
			intentByFile.delete(filePath);
		},
		removeMetadataForFile,
		dropRemovedPaths(livePaths: ReadonlySet<string>): void {
			const known = new Set([...sourceByFile.keys(), ...intentByFile.keys()]);
			for (const filePath of known) {
				if (!livePaths.has(filePath)) {
					removeMetadataForFile(filePath);
				}
			}
		},
		clear(): void {
			sourceByFile.clear();
			intentByFile.clear();
		},
	};
}

export type MetadataCache = ReturnType<typeof createMetadataCache>;

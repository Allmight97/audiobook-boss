import type { AudiobookMetadata } from '../types/metadata';

const metadataByFile = new Map<string, Partial<AudiobookMetadata>>();
const pendingSavePaths = new Set<string>();

const isNullish = (value: unknown): value is null | undefined => value == null;

function hasMeaningfulMetadataEntry(key: string, value: unknown): boolean {
	if (isNullish(value)) {
		return false;
	}
	if (typeof value === 'string') {
		return value.trim().length > 0;
	}
	if (Array.isArray(value)) {
		// cover_art=[] is intentional and means "remove cover art"
		if (key === 'cover_art') {
			return true;
		}
		return value.length > 0;
	}
	return true;
}

function metadataValuesEqual(a: unknown, b: unknown): boolean {
	if (isNullish(a) && isNullish(b)) {
		return true;
	}
	if (Array.isArray(a) && Array.isArray(b)) {
		if (a.length !== b.length) {
			return false;
		}
		return a.every((entry, index) => metadataValuesEqual(entry, b[index]));
	}
	return a === b;
}

type SetMetadataOptions = {
	markPending?: boolean;
};

export function setMetadataForFile(
	filePath: string,
	metadata: Partial<AudiobookMetadata>,
	options?: SetMetadataOptions,
): void {
	metadataByFile.set(filePath, metadata);
	if (options?.markPending) {
		pendingSavePaths.add(filePath);
	}
}

export function getMetadataForFile(filePath: string): Partial<AudiobookMetadata> | undefined {
	return metadataByFile.get(filePath);
}

export function getAllMetadata(): Record<string, Partial<AudiobookMetadata>> {
	return Object.fromEntries(metadataByFile.entries());
}

export function hasMeaningfulMetadata(metadata: Partial<AudiobookMetadata>): boolean {
	return Object.entries(metadata).some(([key, value]) => hasMeaningfulMetadataEntry(key, value));
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

export function hasPendingMetadataChanges(): boolean {
	return pendingSavePaths.size > 0;
}

export function clearPendingMetadataForFile(filePath: string): void {
	pendingSavePaths.delete(filePath);
}

export function clearPendingMetadataForFiles(filePaths: string[]): void {
	filePaths.forEach((filePath) => pendingSavePaths.delete(filePath));
}

export function removeMetadataForFile(filePath: string): void {
	metadataByFile.delete(filePath);
	pendingSavePaths.delete(filePath);
}

export function clearMetadataState(): void {
	metadataByFile.clear();
	pendingSavePaths.clear();
}

import type { AudiobookMetadata } from "../types/metadata";

const metadataByFile = new Map<string, Partial<AudiobookMetadata>>();
const pendingSavePaths = new Set<string>();

type SetMetadataOptions = {
  markPending?: boolean;
};

export function setMetadataForFile(
  filePath: string,
  metadata: Partial<AudiobookMetadata>,
  options?: SetMetadataOptions
): void {
  metadataByFile.set(filePath, metadata);
  if (options?.markPending) {
    pendingSavePaths.add(filePath);
  }
}

export function getMetadataForFile(
  filePath: string
): Partial<AudiobookMetadata> | undefined {
  return metadataByFile.get(filePath);
}

export function getAllMetadata(): Record<string, Partial<AudiobookMetadata>> {
  return Object.fromEntries(metadataByFile.entries());
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

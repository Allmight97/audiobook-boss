import type { AudiobookMetadata } from "../types/metadata";

const metadataByFile = new Map<string, Partial<AudiobookMetadata>>();

export function setMetadataForFile(
  filePath: string,
  metadata: Partial<AudiobookMetadata>
): void {
  metadataByFile.set(filePath, metadata);
}

export function getMetadataForFile(
  filePath: string
): Partial<AudiobookMetadata> | undefined {
  return metadataByFile.get(filePath);
}

export function getAllMetadata(): Record<string, Partial<AudiobookMetadata>> {
  return Object.fromEntries(metadataByFile.entries());
}

export function removeMetadataForFile(filePath: string): void {
  metadataByFile.delete(filePath);
}

export function clearMetadataState(): void {
  metadataByFile.clear();
}

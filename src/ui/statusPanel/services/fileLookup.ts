import type { FileListInfo } from "../../../types/audio";

export function findFilePathByName(
  fileList: FileListInfo | null,
  filename: string
): string | null {
  if (!fileList) return null;
  const match = fileList.files.find((file) => {
    const base = file.path.split(/[\\/]/).pop() || "";
    return base === filename;
  });
  return match?.path ?? null;
}

export function findFilePathByIndex(
  fileList: FileListInfo | null,
  index: number
): string | null {
  if (!fileList) return null;
  if (!Number.isInteger(index)) return null;
  if (index < 0 || index >= fileList.files.length) return null;
  return fileList.files[index]?.path ?? null;
}

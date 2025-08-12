import { FileListInfo } from '../../types/audio';

// Core state variables - keep direct exports for backward compatibility
export let currentFileList: FileListInfo | null = null;
export let selectedFileIndex: number = -1;

// Internal state
let sortAscending: boolean = true;

// Minimal setters for actions module
export function setCurrentFileList(fileList: FileListInfo | null): void {
    currentFileList = fileList;
}

export function setSelectedIndex(index: number): void {
    selectedFileIndex = index;
}

export function getSortAscending(): boolean {
    return sortAscending;
}

export function setSortAscending(ascending: boolean): void {
    sortAscending = ascending;
}

// State validation utilities
export function hasFiles(): boolean {
    return currentFileList !== null && currentFileList.files.length > 0;
}

export function isValidIndex(index: number): boolean {
    if (!currentFileList) return false;
    return index >= 0 && index < currentFileList.files.length;
}

export function getFileCount(): number {
    return currentFileList?.files.length || 0;
}

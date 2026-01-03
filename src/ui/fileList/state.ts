import { FileListInfo } from '../../types/audio';

// Core state variables - keep direct exports for backward compatibility
export let currentFileList: FileListInfo | null = null;
export let selectedFileIndex: number = -1;
// New: Track multiple selections
const selectedFileIndices = new Set<number>();

// Internal state
let sortAscending: boolean = true;
let orderLocked: boolean = false;

// Minimal setters for actions module
export function setCurrentFileList(fileList: FileListInfo | null): void {
    currentFileList = fileList;
}

export function setSelectedIndex(index: number): void {
    selectedFileIndex = index;
}

// Multi-select accessors
export function getSelectedFileIndices(): Set<number> {
    return selectedFileIndices;
}

export function setSelectedFileIndices(indices: Set<number> | number[]): void {
    selectedFileIndices.clear();
    const arr = Array.isArray(indices) ? indices : Array.from(indices);
    arr.forEach(i => selectedFileIndices.add(i));
}

export function addToSelectedIndices(index: number): void {
    selectedFileIndices.add(index);
}

export function removeFromSelectedIndices(index: number): void {
    selectedFileIndices.delete(index);
}

export function clearSelectedIndices(): void {
    selectedFileIndices.clear();
}

export function getSortAscending(): boolean {
    return sortAscending;
}

export function setSortAscending(ascending: boolean): void {
    sortAscending = ascending;
}

export function setOrderLocked(locked: boolean): void {
    orderLocked = locked;
}

export function isOrderLocked(): boolean {
    return orderLocked;
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

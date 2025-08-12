import { AudioFile, FileListInfo, formatFileSize } from '../../types/audio';
import { invoke } from '@tauri-apps/api/core';
import { onFileListChange } from '../outputPanel';
import { setCoverArt } from '../coverArt';
import type { AudiobookMetadata } from '../../types/metadata';
import { 
    currentFileList, 
    selectedFileIndex, 
    setCurrentFileList, 
    setSelectedIndex, 
    getSortAscending, 
    setSortAscending 
} from './state';
import { 
    createFileListItem,
    updateFileListDOM,
    updateTotalStats,
    updateSelection,
    updateSortButtonText,
    updateButtonVisibility,
    showEmptyState 
} from './dom';
import { initFileListEvents } from './events';

export function displayFileList(fileListInfo: FileListInfo): void {
    setCurrentFileList(fileListInfo);
    const container = document.querySelector('.file-list-placeholder');
    if (!container) return;

    container.innerHTML = '';
    container.className = 'file-list-container';

    if (fileListInfo.files.length === 0) {
        container.innerHTML = '<p class="text-gray-500">No files loaded</p>';
        return;
    }

    fileListInfo.files.forEach((file, index) => {
        const fileItem = createFileListItem(file, index);
        container.appendChild(fileItem);
    });

    updateTotalStats();
    initFileListEvents();
    onFileListChange();
    
    // Centralized button updates
    updateButtonVisibility();
    updateSortButtonText(getSortAscending());
}

export function selectFile(index: number): void {
    if (!currentFileList || index < 0 || index >= currentFileList.files.length) return;

    setSelectedIndex(index);
    updateSelection();
    updateFileProperties(currentFileList.files[index]);
}

export function removeFile(index: number): void {
    if (!currentFileList || index < 0 || index >= currentFileList.files.length) return;

    currentFileList.files.splice(index, 1);
    currentFileList.validCount = currentFileList.files.filter(f => f.isValid).length;
    currentFileList.invalidCount = currentFileList.files.length - currentFileList.validCount;
    
    recalculateTotals();
    updateFileListDOM();
    
    if (selectedFileIndex === index) {
        setSelectedIndex(-1);
        clearFileProperties();
    } else if (selectedFileIndex > index) {
        setSelectedIndex(selectedFileIndex - 1);
    }
    
    onFileListChange();
}

export function recalculateTotals(): void {
    if (!currentFileList) return;

    const validFiles = currentFileList.files.filter(f => f.isValid && f.duration && f.size);
    currentFileList.totalDuration = validFiles.reduce((sum, f) => sum + (f.duration || 0), 0);
    currentFileList.totalSize = validFiles.reduce((sum, f) => sum + (f.size || 0), 0);
}

export function updateFileProperties(file: AudioFile): void {
    const bitrateEl = document.getElementById('prop-bitrate');
    const sampleRateEl = document.getElementById('prop-samplerate');
    const channelsEl = document.getElementById('prop-channels');
    const fileSizeEl = document.getElementById('prop-filesize');

    if (file.isValid) {
        // Display technical audio properties
        if (bitrateEl) bitrateEl.textContent = file.bitrate ? `${file.bitrate} kb/s` : 'N/A';
        if (sampleRateEl) sampleRateEl.textContent = file.sampleRate ? `${file.sampleRate} Hz` : 'N/A';
        if (channelsEl) channelsEl.textContent = file.channels ? `${file.channels} ch` : 'N/A';
        if (fileSizeEl) fileSizeEl.textContent = file.size ? formatFileSize(file.size) : 'N/A';
        
        // Still load metadata for the metadata form
        loadFileMetadata(file.path);
    } else {
        // File is invalid, show dashes
        if (bitrateEl) bitrateEl.textContent = '---';
        if (sampleRateEl) sampleRateEl.textContent = '---';
        if (channelsEl) channelsEl.textContent = '---';
        if (fileSizeEl) fileSizeEl.textContent = '---';
    }
}

async function loadFileMetadata(filePath: string): Promise<void> {
    try {
        const metadata = await invoke<AudiobookMetadata>('read_audio_metadata', { filePath: filePath });
        populateMetadataForm(metadata);
    } catch (error) {
        console.warn('Failed to load metadata:', error);
    }
}

function populateMetadataForm(metadata: AudiobookMetadata): void {
    const titleEl = document.getElementById('meta-title') as HTMLInputElement;
    const authorEl = document.getElementById('meta-author') as HTMLInputElement;
    const albumEl = document.getElementById('meta-album') as HTMLInputElement;
    const narratorEl = document.getElementById('meta-narrator') as HTMLInputElement;
    const yearEl = document.getElementById('meta-year') as HTMLInputElement;
    const genreEl = document.getElementById('meta-genre') as HTMLInputElement;
    const descriptionEl = document.getElementById('meta-description') as HTMLTextAreaElement;

    if (titleEl && metadata.title) titleEl.value = metadata.title;
    if (authorEl && metadata.author) authorEl.value = metadata.author;
    if (albumEl && metadata.album) albumEl.value = metadata.album;
    if (narratorEl && metadata.narrator) narratorEl.value = metadata.narrator;
    if (yearEl && metadata.year) yearEl.value = metadata.year.toString();
    if (genreEl && metadata.genre) genreEl.value = metadata.genre;
    if (descriptionEl && metadata.description) descriptionEl.value = metadata.description;

    // Handle cover art display - use the new cover art module
    setCoverArt(metadata.cover_art || null);
}

// Move file up in the list
export function moveFileUp(index: number): void {
    if (!currentFileList || index <= 0 || index >= currentFileList.files.length) return;
    
    // Swap with previous file
    const temp = currentFileList.files[index];
    currentFileList.files[index] = currentFileList.files[index - 1];
    currentFileList.files[index - 1] = temp;
    
    // Update selected index if needed
    if (selectedFileIndex === index) {
        setSelectedIndex(index - 1);
    } else if (selectedFileIndex === index - 1) {
        setSelectedIndex(index);
    }
    
    updateFileListDOM();
    onFileListChange();
}

// Move file down in the list
export function moveFileDown(index: number): void {
    if (!currentFileList || index < 0 || index >= currentFileList.files.length - 1) return;
    
    // Swap with next file
    const temp = currentFileList.files[index];
    currentFileList.files[index] = currentFileList.files[index + 1];
    currentFileList.files[index + 1] = temp;
    
    // Update selected index if needed
    if (selectedFileIndex === index) {
        setSelectedIndex(index + 1);
    } else if (selectedFileIndex === index + 1) {
        setSelectedIndex(index);
    }
    
    updateFileListDOM();
    onFileListChange();
}

export function clearFileProperties(): void {
    const bitrateEl = document.getElementById('prop-bitrate');
    const sampleRateEl = document.getElementById('prop-samplerate');
    const channelsEl = document.getElementById('prop-channels');
    const fileSizeEl = document.getElementById('prop-filesize');

    if (bitrateEl) bitrateEl.textContent = '---';
    if (sampleRateEl) sampleRateEl.textContent = '---';
    if (channelsEl) channelsEl.textContent = '---';
    if (fileSizeEl) fileSizeEl.textContent = '---';

    // Clear metadata form
    const titleEl = document.getElementById('meta-title') as HTMLInputElement;
    const authorEl = document.getElementById('meta-author') as HTMLInputElement;
    const albumEl = document.getElementById('meta-album') as HTMLInputElement;
    const narratorEl = document.getElementById('meta-narrator') as HTMLInputElement;
    const yearEl = document.getElementById('meta-year') as HTMLInputElement;
    const genreEl = document.getElementById('meta-genre') as HTMLInputElement;
    const descriptionEl = document.getElementById('meta-description') as HTMLTextAreaElement;

    if (titleEl) titleEl.value = '';
    if (authorEl) authorEl.value = '';
    if (albumEl) albumEl.value = '';
    if (narratorEl) narratorEl.value = '';
    if (yearEl) yearEl.value = '';
    if (genreEl) genreEl.value = '';
    if (descriptionEl) descriptionEl.value = '';

    // Clear cover art display
    setCoverArt(null);
}

export function toggleFileSort(): void {
    if (!currentFileList || currentFileList.files.length <= 1) return;
    
    setSortAscending(!getSortAscending());
    
    // Sort files by name
    currentFileList.files.sort((a, b) => {
        const nameA = a.path.split(/[\\\/]/).pop() || a.path;
        const nameB = b.path.split(/[\\\/]/).pop() || b.path;
        
        if (getSortAscending()) {
            return nameA.localeCompare(nameB);
        } else {
            return nameB.localeCompare(nameA);
        }
    });
    
    // Reset selected index as files have been reordered
    setSelectedIndex(-1);
    clearFileProperties();
    
    // Update the sort button text
    updateSortButtonText(getSortAscending());
    
    // Ensure button visibility is updated after reordering
    updateButtonVisibility();
    
    updateFileListDOM();
    onFileListChange();
}

export function clearAllFiles(): void {
    if (!currentFileList) return;
    
    currentFileList.files = [];
    currentFileList.validCount = 0;
    currentFileList.invalidCount = 0;
    currentFileList.totalDuration = 0;
    currentFileList.totalSize = 0;
    
    showEmptyState();
    
    setSelectedIndex(-1);
    clearFileProperties();
    updateTotalStats();
    updateButtonVisibility();
    onFileListChange();
}

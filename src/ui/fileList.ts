import { AudioFile, FileListInfo, formatDuration, formatFileSize } from '../types/audio';
import { invoke } from '@tauri-apps/api/core';
import { onFileListChange } from './outputPanel';
import { setCoverArt } from './coverArt';

let currentFileList: FileListInfo | null = null;
let selectedFileIndex: number = -1;
let draggedIndex: number = -1;

export function displayFileList(fileListInfo: FileListInfo): void {
    currentFileList = fileListInfo;
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
}

function createFileListItem(file: AudioFile, index: number): HTMLElement {
    const item = document.createElement('div');
    item.className = `file-list-item ${file.isValid ? 'valid' : 'invalid'}`;
    item.draggable = true;
    item.dataset.index = index.toString();

    const fileName = file.path.split('/').pop() || file.path;
    const statusIcon = file.isValid ? '✓' : '✗';
    const statusClass = file.isValid ? 'text-green-500' : 'text-red-500';

    item.innerHTML = `
        <div class="file-item-content">
            <div class="file-status ${statusClass}">${statusIcon}</div>
            <div class="file-info">
                <div class="file-name">${fileName}</div>
                <div class="file-details">
                    ${file.isValid && file.duration && file.size ? 
                        `${formatDuration(file.duration)} • ${formatFileSize(file.size)} • ${file.format}` :
                        `Error: ${file.error || 'Invalid file'}`
                    }
                </div>
            </div>
            <button class="remove-file-btn" data-index="${index}">×</button>
        </div>
    `;

    // Note: Event handlers are now attached via event delegation in initFileListEvents()
    return item;
}

function selectFile(index: number): void {
    if (!currentFileList || index < 0 || index >= currentFileList.files.length) return;

    selectedFileIndex = index;
    updateSelection();
    updateFileProperties(currentFileList.files[index]);
}

function updateSelection(): void {
    const items = document.querySelectorAll('.file-list-item');
    items.forEach((item, index) => {
        item.classList.toggle('selected', index === selectedFileIndex);
    });
}

function removeFile(index: number): void {
    if (!currentFileList || index < 0 || index >= currentFileList.files.length) return;

    currentFileList.files.splice(index, 1);
    currentFileList.validCount = currentFileList.files.filter(f => f.isValid).length;
    currentFileList.invalidCount = currentFileList.files.length - currentFileList.validCount;
    
    recalculateTotals();
    updateFileListDOM();
    
    if (selectedFileIndex === index) {
        selectedFileIndex = -1;
        clearFileProperties();
    } else if (selectedFileIndex > index) {
        selectedFileIndex--;
    }
    
    onFileListChange();
}

function recalculateTotals(): void {
    if (!currentFileList) return;

    const validFiles = currentFileList.files.filter(f => f.isValid && f.duration && f.size);
    currentFileList.totalDuration = validFiles.reduce((sum, f) => sum + (f.duration || 0), 0);
    currentFileList.totalSize = validFiles.reduce((sum, f) => sum + (f.size || 0), 0);
}

function updateTotalStats(): void {
    if (!currentFileList) return;

    const totalSizeEl = document.getElementById('prop-combinedsize');
    if (totalSizeEl) totalSizeEl.textContent = formatFileSize(currentFileList.totalSize);
}

function updateFileProperties(file: AudioFile): void {
    const bitrateEl = document.getElementById('prop-bitrate');
    const sampleRateEl = document.getElementById('prop-samplerate');
    const channelsEl = document.getElementById('prop-channels');
    const fileSizeEl = document.getElementById('prop-filesize');

    if (file.isValid) {
        // Display technical audio properties
        if (bitrateEl) bitrateEl.textContent = file.bitrate ? `${file.bitrate} kbps` : 'N/A';
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
        const metadata = await invoke('read_audio_metadata', { filePath: filePath });
        populateMetadataForm(metadata);
    } catch (error) {
        console.warn('Failed to load metadata:', error);
    }
}

function populateMetadataForm(metadata: any): void {
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


function clearFileProperties(): void {
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

function initFileListEvents(): void {
    const container = document.querySelector('.file-list-container');
    if (!container) return;

    // Remove any existing event listeners to prevent duplicates
    container.removeEventListener('click', handleFileListClick);
    container.removeEventListener('dragstart', handleDragStart as EventListener);
    container.removeEventListener('dragover', handleDragOver as EventListener);
    container.removeEventListener('drop', handleDrop as EventListener);
    container.removeEventListener('dragend', handleDragEnd as EventListener);

    // Add event delegation handlers
    container.addEventListener('click', handleFileListClick);
    container.addEventListener('dragstart', handleDragStart as EventListener);
    container.addEventListener('dragover', handleDragOver as EventListener);
    container.addEventListener('drop', handleDrop as EventListener);
    container.addEventListener('dragend', handleDragEnd as EventListener);
}

function handleFileListClick(e: Event): void {
    const target = e.target as HTMLElement;
    
    // Handle remove button clicks
    if (target.classList.contains('remove-file-btn')) {
        e.stopPropagation();
        e.preventDefault();
        const index = parseInt(target.dataset.index || '-1');
        if (index >= 0) {
            console.log('Remove button clicked for index:', index);
            removeFile(index);
        }
        return;
    }
    
    // Handle file item selection
    const fileItem = target.closest('.file-list-item') as HTMLElement;
    if (fileItem) {
        const index = parseInt(fileItem.dataset.index || '-1');
        if (index >= 0) selectFile(index);
    }
}

function handleDragStart(event: Event): void {
    const dragEvent = event as DragEvent;
    const target = dragEvent.target as HTMLElement;
    const item = target.closest('.file-list-item') as HTMLElement;
    if (!item) return;

    draggedIndex = parseInt(item.dataset.index || '-1');
    item.classList.add('dragging');
}

function handleDragOver(event: Event): void {
    const dragEvent = event as DragEvent;
    dragEvent.preventDefault();
    const target = dragEvent.target as HTMLElement;
    const item = target.closest('.file-list-item') as HTMLElement;
    if (!item || draggedIndex === -1) return;

    const targetIndex = parseInt(item.dataset.index || '-1');
    if (targetIndex === draggedIndex) return;

    // Clear previous drag-over classes
    document.querySelectorAll('.file-list-item').forEach(el => el.classList.remove('drag-over'));
    item.classList.add('drag-over');
}

function handleDrop(event: Event): void {
    const dragEvent = event as DragEvent;
    dragEvent.preventDefault();
    const target = dragEvent.target as HTMLElement;
    const item = target.closest('.file-list-item') as HTMLElement;
    if (!item || !currentFileList || draggedIndex === -1) return;

    const targetIndex = parseInt(item.dataset.index || '-1');
    if (targetIndex === draggedIndex) return;

    const draggedFile = currentFileList.files[draggedIndex];
    currentFileList.files.splice(draggedIndex, 1);
    currentFileList.files.splice(targetIndex, 0, draggedFile);

    updateFileListDOM();
    onFileListChange();
}

function handleDragEnd(): void {
    draggedIndex = -1;
    document.querySelectorAll('.file-list-item').forEach(item => {
        item.classList.remove('dragging', 'drag-over');
    });
}

function updateFileListDOM(): void {
    if (!currentFileList) return;
    
    const container = document.querySelector('.file-list-container');
    if (!container) return;

    // If no files, show placeholder
    if (currentFileList.files.length === 0) {
        container.innerHTML = '<p class="text-gray-500">No files loaded</p>';
        container.className = 'file-list-placeholder';
        return;
    }

    // Ensure container has correct class
    container.className = 'file-list-container';
    
    // Remove excess items
    const existingItems = container.querySelectorAll('.file-list-item');
    for (let i = currentFileList.files.length; i < existingItems.length; i++) {
        existingItems[i].remove();
    }
    
    // Update or create items
    currentFileList.files.forEach((file, index) => {
        const existingItem = existingItems[index] as HTMLElement;
        if (existingItem) {
            updateFileListItem(existingItem, file, index);
        } else {
            const newItem = createFileListItem(file, index);
            container.appendChild(newItem);
        }
    });
    
    updateTotalStats();
    updateSelection();
    initFileListEvents();
}

function updateFileListItem(item: HTMLElement, file: AudioFile, index: number): void {
    item.className = `file-list-item ${file.isValid ? 'valid' : 'invalid'}`;
    item.dataset.index = index.toString();
    
    const fileName = file.path.split('/').pop() || file.path;
    const statusIcon = file.isValid ? '✓' : '✗';
    const statusClass = file.isValid ? 'text-green-500' : 'text-red-500';
    
    item.innerHTML = `
        <div class="file-item-content">
            <div class="file-status ${statusClass}">${statusIcon}</div>
            <div class="file-info">
                <div class="file-name">${fileName}</div>
                <div class="file-details">
                    ${file.isValid && file.duration && file.size ? 
                        `${formatDuration(file.duration)} • ${formatFileSize(file.size)} • ${file.format}` :
                        `Error: ${file.error || 'Invalid file'}`
                    }
                </div>
            </div>
            <button class="remove-file-btn" data-index="${index}">×</button>
        </div>
    `;
}

export function clearAllFiles(): void {
    if (!currentFileList) return;
    
    currentFileList.files = [];
    currentFileList.validCount = 0;
    currentFileList.invalidCount = 0;
    currentFileList.totalDuration = 0;
    currentFileList.totalSize = 0;
    
    const container = document.querySelector('.file-list-container');
    if (container) {
        container.innerHTML = '<p class="text-gray-500">No files loaded</p>';
        container.className = 'file-list-placeholder';
    }
    
    selectedFileIndex = -1;
    clearFileProperties();
    updateTotalStats();
    onFileListChange();
}

export { currentFileList, selectedFileIndex };

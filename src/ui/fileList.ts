import { AudioFile, FileListInfo, formatDuration, formatFileSize } from '../types/audio';
import { invoke } from '@tauri-apps/api/core';
import { onFileListChange } from './outputPanel';
import { setCoverArt } from './coverArt';

let currentFileList: FileListInfo | null = null;
let selectedFileIndex: number = -1;
// let draggedIndex: number = -1; // Removed - using arrow buttons instead
let sortAscending: boolean = true;

// Initialize sort button when module loads
document.addEventListener('DOMContentLoaded', () => {
    const sortBtn = document.getElementById('sort-toggle-btn');
    if (sortBtn) {
        sortBtn.addEventListener('click', toggleFileSort);
    }
});

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
    
    // Update sort button visibility and text
    const sortBtn = document.getElementById('sort-toggle-btn');
    if (sortBtn) {
        sortBtn.style.display = fileListInfo.files.length > 1 ? 'block' : 'none';
        sortBtn.textContent = sortAscending ? 'Sort: A-Z' : 'Sort: Z-A';
    }
}

function createFileListItem(file: AudioFile, index: number): HTMLElement {
    const item = document.createElement('div');
    item.className = `file-list-item ${file.isValid ? 'valid' : 'invalid'}`;
    // Phase 1: Remove draggable functionality
    // item.draggable = true;
    item.dataset.index = index.toString();

    const fileName = file.path.split('/').pop() || file.path;
    const statusIcon = file.isValid ? '✓' : '✗';
    const statusClass = file.isValid ? 'text-green-500' : 'text-red-500';
    
    const isFirst = index === 0;
    const isLast = currentFileList ? index === currentFileList.files.length - 1 : false;

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
            <button class="move-up-btn" data-index="${index}" ${isFirst ? 'disabled' : ''}>▲</button>
            <button class="move-down-btn" data-index="${index}" ${isLast ? 'disabled' : ''}>▼</button>
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


// Phase 3: Move file up in the list
function moveFileUp(index: number): void {
    if (!currentFileList || index <= 0 || index >= currentFileList.files.length) return;
    
    // Swap with previous file
    const temp = currentFileList.files[index];
    currentFileList.files[index] = currentFileList.files[index - 1];
    currentFileList.files[index - 1] = temp;
    
    // Update selected index if needed
    if (selectedFileIndex === index) {
        selectedFileIndex = index - 1;
    } else if (selectedFileIndex === index - 1) {
        selectedFileIndex = index;
    }
    
    updateFileListDOM();
    onFileListChange();
}

// Phase 3: Move file down in the list
function moveFileDown(index: number): void {
    if (!currentFileList || index < 0 || index >= currentFileList.files.length - 1) return;
    
    // Swap with next file
    const temp = currentFileList.files[index];
    currentFileList.files[index] = currentFileList.files[index + 1];
    currentFileList.files[index + 1] = temp;
    
    // Update selected index if needed
    if (selectedFileIndex === index) {
        selectedFileIndex = index + 1;
    } else if (selectedFileIndex === index + 1) {
        selectedFileIndex = index;
    }
    
    updateFileListDOM();
    onFileListChange();
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
    // Phase 1: Comment out drag-related event listeners
    // container.removeEventListener('dragstart', handleDragStart as EventListener);
    // container.removeEventListener('dragover', handleDragOver as EventListener);
    // container.removeEventListener('drop', handleDrop as EventListener);
    // container.removeEventListener('dragend', handleDragEnd as EventListener);

    // Add event delegation handlers
    container.addEventListener('click', handleFileListClick);
    // Phase 1: Comment out drag-related event listeners
    // container.addEventListener('dragstart', handleDragStart as EventListener);
    // container.addEventListener('dragover', handleDragOver as EventListener);
    // container.addEventListener('drop', handleDrop as EventListener);
    // container.addEventListener('dragend', handleDragEnd as EventListener);
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
    
    // Phase 4: Handle move up button clicks
    if (target.classList.contains('move-up-btn')) {
        e.stopPropagation();
        e.preventDefault();
        const index = parseInt(target.dataset.index || '-1');
        if (index > 0) {
            moveFileUp(index);
        }
        return;
    }
    
    // Phase 4: Handle move down button clicks
    if (target.classList.contains('move-down-btn')) {
        e.stopPropagation();
        e.preventDefault();
        const index = parseInt(target.dataset.index || '-1');
        if (index >= 0 && currentFileList && index < currentFileList.files.length - 1) {
            moveFileDown(index);
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

// Drag-and-drop handlers commented out - replaced with arrow buttons
/*
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
*/

function updateFileListDOM(): void {
    if (!currentFileList) return;
    
    const container = document.querySelector('.file-list-container');
    if (!container) return;

    // If no files, show placeholder
    if (currentFileList.files.length === 0) {
        container.innerHTML = '<p class="text-gray-500">No files loaded</p>';
        container.className = 'file-list-placeholder';
        
        // Hide sort button when no files
        const sortBtn = document.getElementById('sort-toggle-btn');
        if (sortBtn) {
            sortBtn.style.display = 'none';
        }
        
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
    
    // Update sort button visibility
    const sortBtn = document.getElementById('sort-toggle-btn');
    if (sortBtn) {
        sortBtn.style.display = currentFileList.files.length > 1 ? 'block' : 'none';
    }
    
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
    
    const isFirst = index === 0;
    const isLast = currentFileList ? index === currentFileList.files.length - 1 : false;
    
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
            <button class="move-up-btn" data-index="${index}" ${isFirst ? 'disabled' : ''}>▲</button>
            <button class="move-down-btn" data-index="${index}" ${isLast ? 'disabled' : ''}>▼</button>
            <button class="remove-file-btn" data-index="${index}">×</button>
        </div>
    `;
}

// Phase 4: Toggle file sort order
export function toggleFileSort(): void {
    if (!currentFileList || currentFileList.files.length <= 1) return;
    
    sortAscending = !sortAscending;
    
    // Sort files by name
    currentFileList.files.sort((a, b) => {
        const nameA = a.path.split('/').pop() || a.path;
        const nameB = b.path.split('/').pop() || b.path;
        
        if (sortAscending) {
            return nameA.localeCompare(nameB);
        } else {
            return nameB.localeCompare(nameA);
        }
    });
    
    // Reset selected index as files have been reordered
    selectedFileIndex = -1;
    clearFileProperties();
    
    // Update the sort button text
    const sortBtn = document.getElementById('sort-toggle-btn');
    if (sortBtn) {
        sortBtn.textContent = sortAscending ? 'Sort: A-Z' : 'Sort: Z-A';
    }
    
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
    
    const container = document.querySelector('.file-list-container');
    if (container) {
        container.innerHTML = '<p class="text-gray-500">No files loaded</p>';
        container.className = 'file-list-placeholder';
    }
    
    // Hide sort button when no files
    const sortBtn = document.getElementById('sort-toggle-btn');
    if (sortBtn) {
        sortBtn.style.display = 'none';
    }
    
    selectedFileIndex = -1;
    clearFileProperties();
    updateTotalStats();
    onFileListChange();
}

export { currentFileList, selectedFileIndex };

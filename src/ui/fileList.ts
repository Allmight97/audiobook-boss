import { AudioFile, FileListInfo, formatDuration, formatFileSize } from '../types/audio';
import { invoke } from '@tauri-apps/api/core';

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
    initDragReorder();
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

    item.addEventListener('click', () => selectFile(index));
    item.querySelector('.remove-file-btn')?.addEventListener('click', (e) => {
        e.stopPropagation();
        removeFile(index);
    });

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
    displayFileList(currentFileList);
    
    if (selectedFileIndex === index) {
        selectedFileIndex = -1;
        clearFileProperties();
    } else if (selectedFileIndex > index) {
        selectedFileIndex--;
    }
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

    if (bitrateEl) bitrateEl.textContent = file.isValid ? 'N/A' : '---';
    if (sampleRateEl) sampleRateEl.textContent = file.isValid ? 'N/A' : '---';
    if (channelsEl) channelsEl.textContent = file.isValid ? 'N/A' : '---';
    if (fileSizeEl) fileSizeEl.textContent = file.isValid && file.size ? formatFileSize(file.size) : '---';

    if (file.isValid) {
        loadFileMetadata(file.path);
    }
}

async function loadFileMetadata(filePath: string): Promise<void> {
    try {
        const metadata = await invoke('read_audio_metadata', { filePath });
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

    if (titleEl && metadata.title) titleEl.value = metadata.title;
    if (authorEl && metadata.author) authorEl.value = metadata.author;
    if (albumEl && metadata.album) albumEl.value = metadata.album;
    if (narratorEl && metadata.narrator) narratorEl.value = metadata.narrator;
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
}

function initDragReorder(): void {
    const container = document.querySelector('.file-list-container');
    if (!container) return;

    container.addEventListener('dragstart', handleDragStart as EventListener);
    container.addEventListener('dragover', handleDragOver as EventListener);
    container.addEventListener('drop', handleDrop as EventListener);
    container.addEventListener('dragend', handleDragEnd as EventListener);
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

    displayFileList(currentFileList);
}

function handleDragEnd(): void {
    draggedIndex = -1;
    document.querySelectorAll('.file-list-item').forEach(item => {
        item.classList.remove('dragging', 'drag-over');
    });
}

export { currentFileList, selectedFileIndex };
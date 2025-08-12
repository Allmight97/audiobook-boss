import { AudioFile, formatDuration, formatFileSize } from '../../types/audio';
import { currentFileList, selectedFileIndex } from './state';

// Cached DOM elements (stable roots only)
let sortButton: HTMLElement | null = null;
let clearButton: HTMLElement | null = null;

// Initialize cached DOM elements
export function initDOMCache(): void {
    sortButton = document.getElementById('sort-toggle-btn');
    clearButton = document.getElementById('clear-files-btn');
}

// Get container element (may change between placeholder/list)
function getContainer(): Element | null {
    const placeholder = document.querySelector('.file-list-placeholder');
    const listContainer = document.querySelector('.file-list-container');
    return listContainer || placeholder;
}

export function createFileListItem(file: AudioFile, index: number): HTMLElement {
    const item = document.createElement('div');
    item.className = `file-list-item ${file.isValid ? 'valid' : 'invalid'}`;
    item.dataset.index = index.toString();

    const fileName = file.path.split(/[\\\/]/).pop() || file.path;
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

    return item;
}

export function updateFileListItem(item: HTMLElement, file: AudioFile, index: number): void {
    item.className = `file-list-item ${file.isValid ? 'valid' : 'invalid'}`;
    item.dataset.index = index.toString();
    
    const fileName = file.path.split(/[\\\/]/).pop() || file.path;
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

export function updateFileListDOM(): void {
    if (!currentFileList) return;
    
    const container = getContainer();
    if (!container) return;

    // If no files, show placeholder
    if (currentFileList.files.length === 0) {
        container.innerHTML = '<p class="text-gray-500">No files loaded</p>';
        container.className = 'file-list-placeholder';
        
        // Hide sort button when no files
        if (sortButton) {
            sortButton.style.display = 'none';
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
    
    updateButtonVisibility();
    updateTotalStats();
    updateSelection();
}

export function updateButtonVisibility(): void {
    if (!currentFileList) return;
    
    // Update sort button visibility
    if (sortButton) {
        sortButton.style.display = currentFileList.files.length > 1 ? 'block' : 'none';
    }
    if (clearButton) {
        clearButton.style.display = currentFileList.files.length > 0 ? 'block' : 'none';
    }
}

export function updateTotalStats(): void {
    if (!currentFileList) return;

    const totalSizeEl = document.getElementById('prop-combinedsize');
    if (totalSizeEl) totalSizeEl.textContent = formatFileSize(currentFileList.totalSize);
}

export function updateSelection(): void {
    const items = document.querySelectorAll('.file-list-item');
    items.forEach((item, index) => {
        item.classList.toggle('selected', index === selectedFileIndex);
    });
}

export function updateSortButtonText(ascending: boolean): void {
    if (sortButton) {
        sortButton.textContent = ascending ? 'Sort: A-Z' : 'Sort: Z-A';
    }
}

export function clearContainer(): void {
    const container = getContainer();
    if (container) {
        container.innerHTML = '<p class="text-gray-500">No files loaded</p>';
        container.className = 'file-list-placeholder';
    }
}

export function showEmptyState(): void {
    clearContainer();
    
    // Hide buttons when no files
    if (sortButton) {
        sortButton.style.display = 'none';
    }
    if (clearButton) {
        clearButton.style.display = 'none';
    }
}

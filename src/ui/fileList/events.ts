import { currentFileList } from './state';
import { selectFile, removeFile, moveFileUp, moveFileDown } from './actions';

export function initFileListEvents(): void {
    const container = document.querySelector('.file-list-container');
    if (!container) return;

    // Remove any existing event listeners to prevent duplicates
    container.removeEventListener('click', handleFileListClick);
    
    // Add event delegation handlers
    container.addEventListener('click', handleFileListClick);
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
    
    // Handle move up button clicks
    if (target.classList.contains('move-up-btn')) {
        e.stopPropagation();
        e.preventDefault();
        const index = parseInt(target.dataset.index || '-1');
        if (index > 0) {
            moveFileUp(index);
        }
        return;
    }
    
    // Handle move down button clicks
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

// Initialize event handlers for sort and clear buttons on DOM load
export function initDOMEventHandlers(): void {
    // This will be called from the main fileList.ts module during DOMContentLoaded
    // Button event handlers are set up in the main module to maintain initialization order
}

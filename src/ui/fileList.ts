import { initDOMCache } from './fileList/dom';
import { 
    toggleFileSort,
    clearAllFiles
} from './fileList/actions';

// Initialize sort button when module loads
document.addEventListener('DOMContentLoaded', () => {
    initDOMCache(); // Initialize DOM caching
    const sortBtn = document.getElementById('sort-toggle-btn');
    if (sortBtn) {
        sortBtn.addEventListener('click', toggleFileSort);
    }
    const clearBtn = document.getElementById('clear-files-btn') as HTMLButtonElement | null;
    if (clearBtn) {
        clearBtn.addEventListener('click', () => clearAllFiles());
    }
});

// All functions moved to actions.ts - re-export for compatibility
export { 
    displayFileList, 
    toggleFileSort, 
    clearAllFiles 
} from './fileList/actions';

// Re-export state variables from the new state module  
export { currentFileList, selectedFileIndex } from './fileList/state';

// Public API aggregator for fileList module
// This preserves import paths during the split process

import { initDOMCache } from './dom';
import { toggleFileSort, clearAllFiles } from './actions';

export { 
    displayFileList, 
    toggleFileSort, 
    clearAllFiles,
    moveFileUp,
    moveFileDown,
    setFileOrderLocked
} from './actions';

// Re-export state from state module  
export { 
    currentFileList, 
    selectedFileIndex 
} from './state';

// Initialize module when loaded
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

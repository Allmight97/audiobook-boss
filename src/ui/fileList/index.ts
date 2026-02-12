// Public API aggregator for fileList module
// This preserves import paths during the split process

import { initDOMCache } from './dom';
import { toggleFileSort, clearAllFiles } from './actions';

function bindFileListControls(): void {
	const sortBtn = document.getElementById('sort-toggle-btn') as HTMLButtonElement | null;
	if (sortBtn && sortBtn.dataset.fileListBound !== '1') {
		sortBtn.addEventListener('click', toggleFileSort);
		sortBtn.dataset.fileListBound = '1';
	}

	const clearBtn = document.getElementById('clear-files-btn') as HTMLButtonElement | null;
	if (clearBtn && clearBtn.dataset.fileListBound !== '1') {
		clearBtn.addEventListener('click', () => clearAllFiles());
		clearBtn.dataset.fileListBound = '1';
	}
}

export function refreshFileListControlBindings(): void {
	initDOMCache(); // Refresh button cache in case controls mounted after initial load
	bindFileListControls();
}

export {
	displayFileList,
	toggleFileSort,
	clearAllFiles,
	moveFileUp,
	moveFileDown,
	setFileOrderLocked,
} from './actions';

// Re-export state from state module
export {
	getCurrentFileList,
	getSelectedFileIndex,
} from './state';

// Initialize module when loaded
document.addEventListener('DOMContentLoaded', () => {
	refreshFileListControlBindings();
});

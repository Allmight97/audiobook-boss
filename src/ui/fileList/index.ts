// Public API aggregator for fileList module
// This preserves import paths during the split process

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

// Temporary aggregator pointing to original fileList.ts during migration
// This preserves import paths during the split process

export { 
    displayFileList, 
    toggleFileSort, 
    clearAllFiles 
} from '../fileList';

// Re-export state from new state module  
export { 
    currentFileList, 
    selectedFileIndex 
} from './state';

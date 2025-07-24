import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { FileListInfo } from '../types/audio';
import { displayFileList } from './fileList';

let dragDropArea: HTMLElement | null = null;

export function initFileImport(): void {
    dragDropArea = document.querySelector('.drag-drop-area');
    if (!dragDropArea) return;

    setupDragDropHandlers();
    setupClickToSelect();
}

function setupDragDropHandlers(): void {
    if (!dragDropArea) return;

    // Listen for the Tauri file drop event
    listen<string[]>('tauri://file-drop', async (event) => {
        dragDropArea?.classList.remove('drag-over');
        await handleFileDrop(event.payload);
    });

    listen('tauri://file-drop-hover', () => {
        dragDropArea?.classList.add('drag-over');
    });

    listen('tauri://file-drop-cancelled', () => {
        dragDropArea?.classList.remove('drag-over');
    });
}

function setupClickToSelect(): void {
    if (!dragDropArea) return;

    dragDropArea.addEventListener('click', handleClickToSelect);
}

async function handleFileDrop(paths: string[]): Promise<void> {
    const supportedPaths = filterSupportedFiles(paths);
    if (supportedPaths.length === 0) {
        showError('No supported audio files dropped. Please use MP3, M4A, M4B, or AAC files.');
        return;
    }
    await processFilePaths(supportedPaths);
}

async function handleClickToSelect(): Promise<void> {
    try {
        const selected = await open({
            multiple: true,
            directory: false,
            filters: [{
                name: 'Audio Files',
                extensions: ['mp3', 'm4a', 'm4b', 'aac']
            }]
        });
        
        if (Array.isArray(selected) && selected.length > 0) {
            await processFilePaths(selected);
        } else if (typeof selected === 'string') {
            await processFilePaths([selected]);
        }
    } catch (error) {
        showError(`Failed to open file dialog: ${error}`);
    }
}

function filterSupportedFiles(paths: string[]): string[] {
    const supportedFormats = ['.mp3', '.m4a', '.m4b', '.aac'];
    return paths.filter(path => 
        supportedFormats.some(format => 
            path.toLowerCase().endsWith(format)
        )
    );
}

async function processFilePaths(filePaths: string[]): Promise<void> {
    if (filePaths.length === 0) return;


    try {
        const fileListInfo: FileListInfo = await invoke('analyze_audio_files', { 
            filePaths: filePaths 
        });
        displayFileList(fileListInfo);
        clearError();
    } catch (error) {
        showError(`Failed to analyze files: ${error}`);
    }
}

function showError(message: string): void {
    const errorElement = document.getElementById('file-import-error');
    if (errorElement) {
        errorElement.textContent = message;
        errorElement.style.display = 'block';
    }
}

function clearError(): void {
    const errorElement = document.getElementById('file-import-error');
    if (errorElement) {
        errorElement.style.display = 'none';
    }
}

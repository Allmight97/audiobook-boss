/**
 * StatusPanel business logic and state management
 * 
 * This module contains the core StatusPanel class with event handling,
 * processing coordination, and state management.
 */

import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { ProcessingProgressEvent, EVENTS, STAGES } from '../../types/events';
import { currentFileList } from '../fileList';
import { getCurrentAudioSettings } from '../outputPanel';
import { AudiobookMetadata } from '../../types/metadata';
import * as dom from './dom';

interface ProcessingStatus {
    stage: 'idle' | 'analyzing' | 'converting' | 'merging' | 'writing' | 'completed' | 'cancelled' | 'failed';
    percentage: number;
    message: string;
    currentFile?: string;
    etaSeconds?: number;
}

export class StatusPanel {
    private cancelUnlisten?: () => void;
    private isProcessing: boolean = false;
    private currentStatus: ProcessingStatus;

    constructor() {
        this.currentStatus = {
            stage: 'idle',
            percentage: 0,
            message: 'Ready to process audiobook'
        };
        
        this.initializeElements();
        this.setupEventHandlers();
    }

    private initializeElements(): void {
        const elements = dom.initializeElements();
        if (!elements) {
            console.error('StatusPanel: Required DOM elements not found');
            return;
        }

        // Set initial UI state
        this.updateUI();
        
        // Initialize art thumbnail to placeholder
        dom.resetArtThumbnail();
    }

    private setupEventHandlers(): void {
        const processButton = dom.getProcessButton();
        if (processButton) {
            processButton.addEventListener('click', this.handleProcessButtonClick.bind(this));
        }
    }

    private async handleProcessButtonClick(): Promise<void> {
        if (this.isProcessing) {
            // Cancel processing
            await this.handleCancel();
        } else {
            // Start processing
            await this.startProcessing();
        }
    }

    public async startProcessing(): Promise<void> {
        try {
            console.log('StatusPanel: Starting processing...');
            console.log('Current file list:', currentFileList);
            
            // Validate inputs
            if (!currentFileList || !currentFileList.files || currentFileList.files.length === 0) {
                console.log('StatusPanel: No files found');
                dom.showError('No audio files selected. Please add files to process.');
                return;
            }

            if (currentFileList.validCount === 0) {
                console.log('StatusPanel: No valid files found');
                dom.showError('No valid audio files found. Please check your files and try again.');
                return;
            }

            console.log('StatusPanel: Files validated, getting audio settings...');
            
            // Get audio settings
            let settings;
            try {
                settings = getCurrentAudioSettings();
                console.log('StatusPanel: Audio settings retrieved:', settings);
            } catch (error) {
                console.log('StatusPanel: Settings validation failed:', error);
                dom.showError(`Settings validation failed: ${error}`);
                return;
            }

            // Update UI to processing state
            this.isProcessing = true;
            this.updateStatus({
                stage: 'analyzing',
                percentage: 0,
                message: 'Starting processing...'
            });

            // Update art thumbnail with current file's cover art
            await this.updateArtThumbnail();

            // Start listening for progress events
            await this.startProgressListener();

            // Get file paths for processing
            const filePaths = currentFileList.files
                .filter(file => file.isValid)
                .map(file => file.path);

            // Get metadata from the form (basic implementation)
            const metadata = this.getCurrentMetadata();

            // Call backend processing command
            const result = await invoke('process_audiobook_files', {
                filePaths,
                settings,
                metadata: Object.keys(metadata).length > 0 ? metadata : null
            });

            console.log('Processing completed successfully:', result);

        } catch (error) {
            console.error('Processing failed:', error);
            dom.showError(`Processing failed: ${error}`);
            this.resetToIdle();
        }
    }

    private async startProgressListener(): Promise<void> {
        if (this.cancelUnlisten) {
            this.cancelUnlisten();
        }

        this.cancelUnlisten = await listen(EVENTS.PROGRESS, (event) => {
            const progress = event.payload as ProcessingProgressEvent;
            this.updateProgress(progress);
        });
    }

    public updateProgress(event: ProcessingProgressEvent): void {
        const status: ProcessingStatus = {
            stage: event.stage,
            percentage: Math.round(event.percentage * 10) / 10, // Round to 1 decimal place
            message: event.message,
            currentFile: event.current_file,
            etaSeconds: event.eta_seconds
        };

        this.updateStatus(status);

        // Handle completion or failure
        if (status.stage === STAGES.completed) {
            setTimeout(() => {
                this.resetToIdle();
                dom.showSuccess('Audiobook created successfully!');
            }, 2000); // Show success for 2 seconds
        } else if (status.stage === STAGES.failed) {
            this.resetToIdle();
            dom.showError(status.message);
        } else if (status.stage === STAGES.cancelled) {
            this.resetToIdle();
            dom.showInfo('Processing was cancelled.');
        }
    }

    private updateStatus(status: ProcessingStatus): void {
        this.currentStatus = status;
        this.updateUI();
    }

    private updateUI(): void {
        // Update progress bar and percentage
        dom.updateProgressBar(this.currentStatus.percentage);
        dom.updatePercentageText(this.currentStatus.percentage);

        // Update status text
        const statusDisplay = this.getStatusDisplayText();
        dom.updateStatusText(statusDisplay);

        // Update step text
        dom.updateStepText(`Current Step: ${this.currentStatus.message}`);

        // Update process button
        dom.updateProcessButton(this.isProcessing);
    }

    private getStatusDisplayText(): string {
        switch (this.currentStatus.stage) {
            case 'idle':
                return 'Idle';
            case 'analyzing':
                return 'Analyzing';
            case 'converting':
                return 'Converting';
            case 'merging':
                return 'Merging';
            case 'writing':
                return 'Writing Metadata';
            case 'completed':
                return 'Completed';
            case 'cancelled':
                return 'Cancelled';
            case 'failed':
                return 'Failed';
            default:
                return 'Processing';
        }
    }

    private async handleCancel(): Promise<void> {
        try {
            await invoke('cancel_processing');
            this.updateStatus({
                stage: 'cancelled',
                percentage: this.currentStatus.percentage,
                message: 'Cancellation requested...'
            });
        } catch (error) {
            console.error('Failed to cancel processing:', error);
            dom.showError('Failed to cancel processing. Please try again.');
        }
    }

    private resetToIdle(): void {
        this.isProcessing = false;
        
        if (this.cancelUnlisten) {
            this.cancelUnlisten();
            this.cancelUnlisten = undefined;
        }

        this.updateStatus({
            stage: 'idle',
            percentage: 0,
            message: 'Ready to process audiobook'
        });

        // Reset art thumbnail to placeholder
        dom.resetArtThumbnail();
    }

    private convertBytesToDataUrl(bytes: number[]): string {
        // Convert number array to Uint8Array
        const uint8Array = new Uint8Array(bytes);
        
        // Detect image format from magic bytes
        let mimeType = 'image/jpeg'; // default fallback
        if (uint8Array.length >= 4) {
            // PNG: 89 50 4E 47
            if (uint8Array[0] === 0x89 && uint8Array[1] === 0x50 && uint8Array[2] === 0x4E && uint8Array[3] === 0x47) {
                mimeType = 'image/png';
            }
            // JPEG: FF D8 FF
            else if (uint8Array[0] === 0xFF && uint8Array[1] === 0xD8 && uint8Array[2] === 0xFF) {
                mimeType = 'image/jpeg';
            }
            // WebP: 52 49 46 46 ... 57 45 42 50
            else if (uint8Array[0] === 0x52 && uint8Array[1] === 0x49 && uint8Array[2] === 0x46 && uint8Array[3] === 0x46) {
                mimeType = 'image/webp';
            }
        }
        
        // Convert to base64
        let binary = '';
        uint8Array.forEach(byte => {
            binary += String.fromCharCode(byte);
        });
        const base64 = btoa(binary);
        
        return `data:${mimeType};base64,${base64}`;
    }

    private async updateArtThumbnail(): Promise<void> {
        if (!currentFileList || !currentFileList.files.length) {
            dom.resetArtThumbnail();
            return;
        }

        // Get the first valid file for cover art
        const firstValidFile = currentFileList.files.find(f => f.isValid);
        if (!firstValidFile) {
            dom.resetArtThumbnail();
            return;
        }

        try {
            // Load metadata with cover art
            const metadata = await invoke<AudiobookMetadata>('read_audio_metadata', { 
                filePath: firstValidFile.path 
            });
            
            // Check for cover art data (backend returns as cover_art field with number array)
            if (metadata.cover_art && metadata.cover_art.length > 0) {
                const dataUrl = this.convertBytesToDataUrl(metadata.cover_art);
                dom.displayCoverArt(dataUrl);
            } else {
                dom.resetArtThumbnail();
            }
        } catch (error) {
            console.warn('Failed to load cover art for thumbnail:', error);
            dom.resetArtThumbnail();
        }
    }

    private getCurrentMetadata(): Partial<AudiobookMetadata> {
        // Basic metadata extraction from DOM elements
        const getElementValue = (id: string): string => {
            const element = document.getElementById(id) as HTMLInputElement;
            return element?.value?.trim() || '';
        };

        const metadata: Partial<AudiobookMetadata> = {};
        
        const title = getElementValue('meta-title');
        const author = getElementValue('meta-author');
        const album = getElementValue('meta-album');
        const narrator = getElementValue('meta-narrator');
        const year = getElementValue('meta-year');
        const genre = getElementValue('meta-genre');
        const series = getElementValue('meta-series');
        const description = getElementValue('meta-description');

        if (title) metadata.title = title;
        if (author) metadata.author = author;
        if (album) metadata.album = album;
        if (narrator) metadata.narrator = narrator;
        if (year) {
            const yearNum = parseInt(year);
            if (!isNaN(yearNum)) metadata.year = yearNum;
        }
        if (genre) metadata.genre = genre;
        if (series) metadata.series = series;
        if (description) metadata.description = description;

        return metadata;
    }

    // Public method to check if processing is active
    public get isCurrentlyProcessing(): boolean {
        return this.isProcessing;
    }

    // Public method to get current status
    public getCurrentStatus(): ProcessingStatus {
        return { ...this.currentStatus };
    }
}

// Export a singleton instance
let statusPanelInstance: StatusPanel | null = null;

export function initStatusPanel(): StatusPanel {
    if (!statusPanelInstance) {
        statusPanelInstance = new StatusPanel();
    }
    return statusPanelInstance;
}

export function getStatusPanel(): StatusPanel | null {
    return statusPanelInstance;
}

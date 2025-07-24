import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { currentFileList } from './fileList';
import { getCurrentAudioSettings } from './outputPanel';

interface ProgressEvent {
    stage: string;
    percentage: number;
    message: string;
    current_file?: string;
    eta_seconds?: number;
}

interface ProcessingStatus {
    stage: 'idle' | 'analyzing' | 'converting' | 'merging' | 'writing' | 'completed' | 'cancelled' | 'failed';
    percentage: number;
    message: string;
    currentFile?: string;
    etaSeconds?: number;
}

export class StatusPanel {
    private progressBar!: HTMLElement;
    private percentageElement!: HTMLElement;
    private statusText!: HTMLElement;
    private stepText!: HTMLElement;
    private processButton!: HTMLButtonElement;
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
        // Get DOM elements from the existing HTML structure
        this.progressBar = document.getElementById('progress-bar') as HTMLElement;
        this.percentageElement = document.getElementById('percentage-processed') as HTMLElement;
        this.statusText = document.getElementById('status-text') as HTMLElement;
        this.stepText = document.getElementById('step-text') as HTMLElement;
        this.processButton = document.getElementById('process-button') as HTMLButtonElement;

        if (!this.progressBar || !this.percentageElement || !this.statusText || 
            !this.stepText || !this.processButton) {
            console.error('StatusPanel: Required DOM elements not found');
            return;
        }

        // Set initial state
        this.updateUI();
    }

    private setupEventHandlers(): void {
        if (this.processButton) {
            this.processButton.addEventListener('click', this.handleProcessButtonClick.bind(this));
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
                this.showError('No audio files selected. Please add files to process.');
                return;
            }

            if (currentFileList.validCount === 0) {
                console.log('StatusPanel: No valid files found');
                this.showError('No valid audio files found. Please check your files and try again.');
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
                this.showError(`Settings validation failed: ${error}`);
                return;
            }

            // Update UI to processing state
            this.isProcessing = true;
            this.updateStatus({
                stage: 'analyzing',
                percentage: 0,
                message: 'Starting processing...'
            });

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
            this.showError(`Processing failed: ${error}`);
            this.resetToIdle();
        }
    }

    private async startProgressListener(): Promise<void> {
        if (this.cancelUnlisten) {
            this.cancelUnlisten();
        }

        this.cancelUnlisten = await listen('processing-progress', (event) => {
            const progress = event.payload as ProgressEvent;
            this.updateProgress(progress);
        });
    }

    public updateProgress(event: ProgressEvent): void {
        const status: ProcessingStatus = {
            stage: event.stage as ProcessingStatus['stage'],
            percentage: Math.round(event.percentage * 10) / 10, // Round to 1 decimal place
            message: event.message,
            currentFile: event.current_file,
            etaSeconds: event.eta_seconds
        };

        this.updateStatus(status);

        // Handle completion or failure
        if (status.stage === 'completed') {
            setTimeout(() => {
                this.resetToIdle();
                this.showSuccess('Audiobook created successfully!');
            }, 2000); // Show success for 2 seconds
        } else if (status.stage === 'failed') {
            this.resetToIdle();
            this.showError(status.message);
        } else if (status.stage === 'cancelled') {
            this.resetToIdle();
            this.showInfo('Processing was cancelled.');
        }
    }

    private updateStatus(status: ProcessingStatus): void {
        this.currentStatus = status;
        this.updateUI();
    }

    private updateUI(): void {
        // Update progress bar
        if (this.progressBar) {
            this.progressBar.style.width = `${this.currentStatus.percentage}%`;
        }

        // Update percentage display
        if (this.percentageElement) {
            this.percentageElement.textContent = `${this.currentStatus.percentage.toFixed(1)}%`;
        }

        // Update status text
        if (this.statusText) {
            const statusDisplay = this.getStatusDisplayText();
            this.statusText.textContent = statusDisplay;
        }

        // Update step text
        if (this.stepText) {
            this.stepText.textContent = `Current Step: ${this.currentStatus.message}`;
        }

        // Update process button
        this.updateProcessButton();
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

    private updateProcessButton(): void {
        if (!this.processButton) return;

        if (this.isProcessing) {
            this.processButton.textContent = 'Cancel Processing';
            this.processButton.className = 'button-secondary';
        } else {
            this.processButton.textContent = 'Process Audiobook';
            this.processButton.className = 'button-primary';
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
            this.showError('Failed to cancel processing. Please try again.');
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
    }

    private getCurrentMetadata(): any {
        // Basic metadata extraction from DOM elements
        const getElementValue = (id: string): string => {
            const element = document.getElementById(id) as HTMLInputElement;
            return element?.value?.trim() || '';
        };

        const metadata: any = {};
        
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

    private showError(message: string): void {
        console.error('StatusPanel Error:', message);
        // For now, just log. Could add visual error display in the future
        if (this.stepText) {
            this.stepText.textContent = `Error: ${message}`;
            this.stepText.style.color = 'var(--text-error, #ef4444)';
        }
    }

    private showSuccess(message: string): void {
        console.log('StatusPanel Success:', message);
        if (this.stepText) {
            this.stepText.textContent = message;
            this.stepText.style.color = 'var(--text-success, #10b981)';
        }
    }

    private showInfo(message: string): void {
        console.log('StatusPanel Info:', message);
        if (this.stepText) {
            this.stepText.textContent = message;
            this.stepText.style.color = 'var(--text-primary)';
        }
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

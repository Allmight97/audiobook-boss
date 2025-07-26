import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

// Global state for currently loaded cover art
let currentCoverArt: number[] | null = null;

/**
 * Initializes the cover art functionality
 * Sets up event handlers for the Load Cover Art button
 */
export function initCoverArt(): void {
    const loadButton = document.getElementById('load-cover-art') as HTMLButtonElement;
    const clearButton = document.getElementById('clear-cover-art') as HTMLButtonElement;
    
    if (loadButton) {
        loadButton.addEventListener('click', handleLoadCoverArt);
    }
    
    if (clearButton) {
        clearButton.addEventListener('click', handleClearCoverArt);
    }
    
    // Update button visibility based on initial state
    updateClearButtonVisibility();
}

/**
 * Handles the Clear Cover Art button click
 * Clears the current cover art and updates UI
 */
function handleClearCoverArt(): void {
    clearCoverArt();
    updateClearButtonVisibility();
    console.log('Cover art cleared');
}

/**
 * Updates the visibility of the Clear button based on cover art state
 */
function updateClearButtonVisibility(): void {
    const clearButton = document.getElementById('clear-cover-art') as HTMLButtonElement;
    if (clearButton) {
        clearButton.style.display = currentCoverArt ? 'block' : 'none';
    }
}

/**
 * Handles the Load Cover Art button click
 * Opens file dialog and loads selected image
 */
async function handleLoadCoverArt(): Promise<void> {
    try {
        // Open file dialog for image selection
        const selectedFile = await open({
            multiple: false,
            directory: false,
            title: 'Select Cover Art Image',
            filters: [{
                name: 'Image Files',
                extensions: ['jpg', 'jpeg', 'png', 'webp']
            }]
        });

        if (!selectedFile || typeof selectedFile !== 'string') {
            return; // User cancelled
        }

        // Load image data from backend
        const imageData = await invoke<number[]>('load_cover_art_file', { 
            filePath: selectedFile 
        });

        // Update global state
        currentCoverArt = imageData;

        // Display the loaded cover art
        displayCoverArt(imageData);

        // Update metadata form if needed
        updateMetadataWithCoverArt(imageData);
        
        // Update Clear button visibility
        updateClearButtonVisibility();

        console.log('Cover art loaded successfully:', selectedFile);

    } catch (error) {
        console.error('Failed to load cover art:', error);
        showCoverArtError(error instanceof Error ? error.message : 'Unknown error');
    }
}

/**
 * Displays cover art in the UI
 * Updates both the main cover art area and any thumbnails
 */
function displayCoverArt(coverArtBytes: number[] | null): void {
    const coverImg = document.getElementById('cover-art-img') as HTMLImageElement;
    const placeholderText = document.querySelector('.cover-art-area .placeholder-text') as HTMLElement;
    
    if (!coverImg || !placeholderText) {
        console.warn('Cover art display elements not found');
        return;
    }

    if (coverArtBytes && coverArtBytes.length > 0) {
        // Convert byte array to Uint8Array and then to base64
        const uint8Array = new Uint8Array(coverArtBytes);
        const base64String = btoa(String.fromCharCode(...uint8Array));
        
        // Determine image format based on file signature
        let mimeType = 'image/jpeg'; // default
        if (coverArtBytes.length >= 8) {
            if (coverArtBytes[0] === 0x89 && coverArtBytes[1] === 0x50) {
                mimeType = 'image/png';
            } else if (coverArtBytes.length >= 12 && 
                      coverArtBytes[0] === 0x52 && coverArtBytes[1] === 0x49) {
                mimeType = 'image/webp';
            }
        }
        
        // Create data URL
        const dataUrl = `data:${mimeType};base64,${base64String}`;
        
        // Show image and hide placeholder
        coverImg.src = dataUrl;
        coverImg.classList.remove('hidden');
        placeholderText.style.display = 'none';
    } else {
        // No cover art - show placeholder and hide image
        coverImg.classList.add('hidden');
        coverImg.src = '';
        placeholderText.style.display = 'block';
    }
}

/**
 * Updates metadata form with cover art data
 * Ensures cover art is included in metadata operations
 */
function updateMetadataWithCoverArt(coverArtBytes: number[]): void {
    // Store in a way that can be accessed by metadata operations
    // This will be used when processing or saving metadata
    (window as any).currentCoverArt = coverArtBytes;
}

/**
 * Shows an error message for cover art operations
 */
function showCoverArtError(message: string): void {
    // For now, just log to console
    // Could add a visual error display in the future
    console.error('Cover Art Error:', message);
    
    // Could show a temporary error message in the UI
    const errorDiv = document.createElement('div');
    errorDiv.className = 'error-message';
    errorDiv.textContent = `Failed to load cover art: ${message}`;
    errorDiv.style.cssText = `
        position: fixed;
        top: 20px;
        right: 20px;
        background: #ef4444;
        color: white;
        padding: 12px;
        border-radius: 4px;
        z-index: 1000;
        max-width: 300px;
    `;
    
    document.body.appendChild(errorDiv);
    
    // Remove after 5 seconds
    setTimeout(() => {
        if (errorDiv.parentNode) {
            errorDiv.parentNode.removeChild(errorDiv);
        }
    }, 5000);
}

/**
 * Gets the currently loaded cover art
 * Returns null if no cover art is loaded
 */
export function getCurrentCoverArt(): number[] | null {
    return currentCoverArt;
}

/**
 * Sets cover art data (used by other modules)
 * Updates display and state
 */
export function setCoverArt(coverArtBytes: number[] | null): void {
    currentCoverArt = coverArtBytes;
    displayCoverArt(coverArtBytes);
}

/**
 * Clears the current cover art
 * Resets display to placeholder state
 */
export function clearCoverArt(): void {
    currentCoverArt = null;
    displayCoverArt(null);
    delete (window as any).currentCoverArt;
}
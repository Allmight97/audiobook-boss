import { bridge } from "../lib/bridge";
import { isFileDropEvent, EventPayload } from "../types/events";

// Global state for currently loaded cover art
let currentCoverArt: number[] | null = null;
// Tracks whether the user manually loaded custom cover art (preserved across file selection)
let hasCustomCoverArt: boolean = false;
// Tracks whether the user explicitly requested cover art removal in this session
let coverArtRemovalRequested: boolean = false;

/**
 * Initializes the cover art functionality
 */
export function initCoverArt(): void {
    const coverArtArea = document.getElementById("cover-art-area");
    // const clearButton = document.getElementById("cover-art-clear-btn");

    if (coverArtArea) {
        // Click Handler (Load or Clear via delegation)
        coverArtArea.addEventListener("click", (e) => {
            const target = e.target as HTMLElement;
            // If clicked the clear button
            if (target.closest(".cover-art-clear-btn") || target.id === "cover-art-clear-btn") {
                e.stopPropagation();
                handleClearCoverArt();
                return;
            }
            // Otherwise, load cover art
            handleLoadCoverArt();
        });

        // Drag Visuals
        coverArtArea.addEventListener("dragover", (e) => {
            e.preventDefault();
            coverArtArea.classList.add("drag-over");
        });
        coverArtArea.addEventListener("dragleave", (e) => {
            e.preventDefault();
            coverArtArea.classList.remove("drag-over");
        });
    }

    // Handle Global Drag & Drop (Tauri Event) for Cover Art
    bridge.listen<EventPayload<"tauri://drag-drop">>(
        "tauri://drag-drop",
        async (event) => {
            if (!isFileDropEvent(event.payload)) return;
            const { position, paths } = event.payload;

            const area = document.getElementById("cover-art-area");
            if (!area) return;

            // Remove drag visual
            area.classList.remove("drag-over");

            // Check bounds
            const rect = area.getBoundingClientRect();
            if (
                position.x >= rect.left &&
                position.x <= rect.right &&
                position.y >= rect.top &&
                position.y <= rect.bottom
            ) {
                // Filter for image files
                const imageFile = paths.find((p) =>
                    /\.(jpg|jpeg|png|webp)$/i.test(p)
                );

                if (imageFile) {
                    await loadCoverArtFile(imageFile);
                }
            }
        }
    );

    // Initial Visibility Check
    updateClearButtonVisibility();
}

/**
 * Handles the Clear Cover Art action
 */
function handleClearCoverArt(): void {
    clearCoverArt({ markRemoval: true });
    console.log("Cover art cleared");
}

/**
 * Updates the visibility of the Clear button (now handled via CSS hover, but kept for logic sync)
 * and toggles .has-image class on the container
 */
function updateClearButtonVisibility(): void {
    const coverArtArea = document.getElementById("cover-art-area");
    const clearButton = document.getElementById("cover-art-clear-btn");

    if (coverArtArea) {
        if (currentCoverArt) {
            coverArtArea.classList.add("has-image");
        } else {
            coverArtArea.classList.remove("has-image");
        }
    }

    // Legacy button support (if exists) + overlay button state if needed
    if (clearButton) {
        // Overlay button display is handled by CSS (.cover-art-area.has-image:hover)
        // but we can enforce logic here if needed.
    }
}

/**
 * Handles the Click-to-Load action
 */
async function handleLoadCoverArt(): Promise<void> {
    try {
        const selectedFile = await bridge.open({
            multiple: false,
            directory: false,
            title: "Select Cover Art Image",
            filters: [
                {
                    name: "Image Files",
                    extensions: ["jpg", "jpeg", "png", "webp"],
                },
            ],
        });

        if (!selectedFile || typeof selectedFile !== "string") {
            return; // User cancelled
        }

        await loadCoverArtFile(selectedFile);
    } catch (error) {
        console.error("Failed to open file dialog:", error);
    }
}

/**
 * Loads cover art from a specific file path
 */
async function loadCoverArtFile(filePath: string): Promise<void> {
    try {
        const imageData = await bridge.invoke<number[]>("load_cover_art_file", {
            filePath: filePath,
        });

        currentCoverArt = imageData;
        hasCustomCoverArt = true;
        coverArtRemovalRequested = false;

        displayCoverArt(imageData);
        updateMetadataWithCoverArt(imageData);
        updateClearButtonVisibility();

        console.log("Cover art loaded:", filePath);
    } catch (error) {
        console.error("Failed to load cover art file:", error);
        showCoverArtError(error instanceof Error ? error.message : "Unknown error");
    }
}

/**
 * Displays cover art in the UI
 */
export function displayCoverArt(coverArtBytes: number[] | null): void {
    const coverImg = document.getElementById("cover-art-img") as HTMLImageElement;
    const placeholderText = document.querySelector(
        ".cover-art-area .placeholder-text"
    ) as HTMLElement;
    const coverArtArea = document.getElementById("cover-art-area");

    if (!coverImg) return;

    if (coverArtBytes && coverArtBytes.length > 0) {
        const uint8Array = new Uint8Array(coverArtBytes);
        const base64String = btoa(String.fromCharCode(...uint8Array));

        // Simple mime detection
        let mimeType = "image/jpeg";
        if (coverArtBytes.length >= 8) {
            if (coverArtBytes[0] === 0x89 && coverArtBytes[1] === 0x50) mimeType = "image/png";
            else if (coverArtBytes.length >= 4 && coverArtBytes[0] === 0x52) mimeType = "image/webp";
        }

        const dataUrl = `data:${mimeType};base64,${base64String}`;
        coverImg.src = dataUrl;
        coverImg.classList.remove("hidden");
        if (placeholderText) placeholderText.style.display = "none";
        if (coverArtArea) coverArtArea.classList.add("has-image");
    } else {
        coverImg.classList.add("hidden");
        coverImg.src = "";
        if (placeholderText) placeholderText.style.display = "block";
        if (coverArtArea) coverArtArea.classList.remove("has-image");
    }
    updateClearButtonVisibility();
}

/**
 * Updates global state for metadata operations
 */
function updateMetadataWithCoverArt(coverArtBytes: number[]): void {
    (window as any).currentCoverArt = coverArtBytes;
}

function showCoverArtError(message: string): void {
    console.error("Cover Art Error:", message);
    // (Optional: visual toast)
}

// Global Exports
export function getCurrentCoverArt(): number[] | null {
    return currentCoverArt;
}

export function getHasCustomCoverArt(): boolean {
    return hasCustomCoverArt;
}

export function isCoverArtRemovalRequested(): boolean {
    return coverArtRemovalRequested;
}

export function setCoverArt(coverArtBytes: number[] | null): void {
    currentCoverArt = coverArtBytes;
    if (coverArtBytes && coverArtBytes.length > 0) {
        coverArtRemovalRequested = false;
    }
    displayCoverArt(coverArtBytes);
    updateClearButtonVisibility();
}

export function clearCoverArt(options?: { markRemoval?: boolean }): void {
    const markRemoval = options?.markRemoval ?? false;
    currentCoverArt = null;
    displayCoverArt(null);
    delete (window as any).currentCoverArt;
    coverArtRemovalRequested = markRemoval;
    hasCustomCoverArt = false;
    updateClearButtonVisibility();
}

import { bridge } from "../lib/bridge";
import { isFileDropEvent, EventPayload } from "../types/events";

// Global state for currently loaded cover art
let currentCoverArt: number[] | null = null;
// Tracks whether the user manually loaded custom cover art (preserved across file selection)
let hasCustomCoverArt: boolean = false;
// Tracks whether the user explicitly requested cover art removal in this session
let coverArtRemovalRequested: boolean = false;
let coverArtMessageTimeoutId: number | null = null;
let isCoverArtAreaHovered: boolean = false;

/**
 * Initializes the cover art functionality
 */
export function initCoverArt(): void {
    const coverArtArea = document.getElementById("cover-art-area");
    const coverArtUrlInput = document.getElementById(
        "cover-art-url-input"
    ) as HTMLInputElement | null;
    const coverArtUrlButton = document.getElementById(
        "cover-art-url-load-btn"
    ) as HTMLButtonElement | null;

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

        coverArtArea.addEventListener("mouseenter", () => {
            isCoverArtAreaHovered = true;
        });
        coverArtArea.addEventListener("mouseleave", () => {
            isCoverArtAreaHovered = false;
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

    if (coverArtUrlInput) {
        coverArtUrlInput.addEventListener("keydown", (event) => {
            if (event.key === "Enter") {
                event.preventDefault();
                void handleLoadCoverArtFromInput(coverArtUrlInput);
            }
        });
        coverArtUrlInput.addEventListener("paste", (event) => {
            const pasted = getUrlFromClipboard(event);
            if (!pasted) return;
            coverArtUrlInput.value = pasted;
        });
    }

    if (coverArtUrlButton) {
        coverArtUrlButton.addEventListener("click", () => {
            if (!coverArtUrlInput) return;
            void handleLoadCoverArtFromInput(coverArtUrlInput);
        });
    }

    document.addEventListener("paste", (event) => {
        const target = event.target as HTMLElement | null;
        if (target && isTextInput(target) && target.id !== "cover-art-url-input") {
            return;
        }
        if (!isCoverArtAreaHovered) return;
        const pastedUrl = getUrlFromClipboard(event);
        if (!pastedUrl) return;
        event.preventDefault();
        void loadCoverArtFromUrl(pastedUrl);
    });

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

async function handleLoadCoverArtFromInput(
    input: HTMLInputElement
): Promise<void> {
    const raw = input.value.trim();
    if (!raw) {
        showCoverArtMessage("Paste an image URL first.", "error");
        return;
    }

    const parsed = parseCoverArtUrl(raw);
    if (!parsed) {
        showCoverArtMessage("Invalid URL format.", "error");
        return;
    }

    if (parsed.protocol !== "https:") {
        showCoverArtMessage("Only HTTPS URLs are supported.", "error");
        return;
    }

    input.value = parsed.toString();
    await loadCoverArtFromUrl(parsed.toString());
}

/**
 * Loads cover art from a specific file path
 */
async function loadCoverArtFile(filePath: string): Promise<void> {
    try {
        const imageData = await bridge.invoke<number[]>("load_cover_art_file", {
            filePath: filePath,
        });

        applyLoadedCoverArt(imageData);

        console.log("Cover art loaded:", filePath);
    } catch (error) {
        console.error("Failed to load cover art file:", error);
        showCoverArtError(formatCoverArtError(error, "Unknown error"));
    }
}

async function loadCoverArtFromUrl(url: string): Promise<void> {
    try {
        setCoverArtLoading(true);
        clearCoverArtMessage();
        const imageData = await bridge.invoke<number[]>("load_cover_art_from_url", {
            url: url,
        });

        applyLoadedCoverArt(imageData);
        showCoverArtMessage("Cover art loaded from URL.", "success");
    } catch (error) {
        console.error("Failed to load cover art URL:", error);
        showCoverArtError(formatCoverArtError(error, "Unable to load image."));
    } finally {
        setCoverArtLoading(false);
    }
}

function applyLoadedCoverArt(imageData: number[]): void {
    currentCoverArt = imageData;
    hasCustomCoverArt = true;
    coverArtRemovalRequested = false;

    displayCoverArt(imageData);
    updateMetadataWithCoverArt(imageData);
    updateClearButtonVisibility();
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

        // MIME type detection from magic bytes
        let mimeType = "image/jpeg";
        if (coverArtBytes.length >= 12) {
            if (coverArtBytes[0] === 0x89 && coverArtBytes[1] === 0x50) {
                mimeType = "image/png";
            } else if (
                coverArtBytes[0] === 0x52 && coverArtBytes[1] === 0x49 && coverArtBytes[2] === 0x46 && coverArtBytes[3] === 0x46 && // RIFF
                coverArtBytes[8] === 0x57 && coverArtBytes[9] === 0x45 && coverArtBytes[10] === 0x42 && coverArtBytes[11] === 0x50   // WEBP
            ) {
                mimeType = "image/webp";
            }
        } else if (coverArtBytes.length >= 2 && coverArtBytes[0] === 0x89 && coverArtBytes[1] === 0x50) {
            mimeType = "image/png";
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
    showCoverArtMessage(message, "error");
}

function showCoverArtMessage(message: string, variant: "error" | "success"): void {
    const messageEl = document.getElementById("cover-art-url-message");
    if (!messageEl) return;

    messageEl.textContent = message;
    messageEl.classList.toggle("is-error", variant === "error");
    messageEl.classList.toggle("is-success", variant === "success");
    messageEl.classList.add("visible");

    if (coverArtMessageTimeoutId !== null) {
        window.clearTimeout(coverArtMessageTimeoutId);
    }
    coverArtMessageTimeoutId = window.setTimeout(() => {
        messageEl.classList.remove("visible");
        messageEl.textContent = "";
    }, 4000);
}

function clearCoverArtMessage(): void {
    const messageEl = document.getElementById("cover-art-url-message");
    if (!messageEl) return;
    messageEl.classList.remove("visible", "is-error", "is-success");
    messageEl.textContent = "";
    if (coverArtMessageTimeoutId !== null) {
        window.clearTimeout(coverArtMessageTimeoutId);
        coverArtMessageTimeoutId = null;
    }
}

function formatCoverArtError(error: unknown, fallback: string): string {
    if (error instanceof Error) return error.message;
    if (typeof error === "string") return error;
    return fallback;
}

function setCoverArtLoading(isLoading: boolean): void {
    const coverArtArea = document.getElementById("cover-art-area");
    const coverArtUrlInput = document.getElementById(
        "cover-art-url-input"
    ) as HTMLInputElement | null;
    const coverArtUrlButton = document.getElementById(
        "cover-art-url-load-btn"
    ) as HTMLButtonElement | null;

    if (coverArtArea) {
        coverArtArea.classList.toggle("loading", isLoading);
    }
    if (coverArtUrlInput) {
        coverArtUrlInput.disabled = isLoading;
    }
    if (coverArtUrlButton) {
        coverArtUrlButton.disabled = isLoading;
    }
}

function parseCoverArtUrl(raw: string): URL | null {
    try {
        return new URL(raw);
    } catch {
        return null;
    }
}

function isTextInput(target: HTMLElement): boolean {
    const tagName = target.tagName.toLowerCase();
    return tagName === "input" || tagName === "textarea";
}

function getUrlFromClipboard(event: ClipboardEvent): string | null {
    const raw = event.clipboardData?.getData("text")?.trim();
    if (!raw) return null;
    const parsed = parseCoverArtUrl(raw);
    return parsed ? parsed.toString() : null;
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
    const coverArtUrlInput = document.getElementById(
        "cover-art-url-input"
    ) as HTMLInputElement | null;
    if (coverArtUrlInput) {
        coverArtUrlInput.value = "";
    }
    clearCoverArtMessage();
}

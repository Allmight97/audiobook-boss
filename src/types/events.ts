/**
 * Frontend Event Contract Documentation for Phase 0
 * 
 * This file documents the complete event contract between the Rust backend
 * and TypeScript frontend as it currently exists. This serves as documentation
 * for Phase 0 and helps prevent breaking changes during refactoring.
 * 
 * Created: Phase 0 - Event Contract Documentation
 * Purpose: Preserve exact behavior during refactoring
 */

// ============================================================================
// PROCESSING EVENTS (Backend → Frontend)
// ============================================================================

/**
 * Progress event emitted by Rust backend during audio processing
 * 
 * Source: src-tauri/src/audio/processor.rs (ProgressEvent struct)
 * Handler: src/ui/statusPanel.ts (listen('processing-progress'))
 * 
 * Emitted during:
 * - File analysis phase
 * - Audio conversion and merging
 * - Metadata writing
 * - Process completion/failure/cancellation
 */
export interface ProcessingProgressEvent {
    /** Processing stage identifier */
    stage: 'analyzing' | 'converting' | 'merging' | 'writing' | 'completed' | 'failed' | 'cancelled';
    
    /** Progress percentage (0.0 to 100.0) */
    percentage: number;
    
    /** Human-readable status message */
    message: string;
    
    /** Currently processing file (optional) */
    current_file?: string;
    
    /** Estimated time remaining in seconds (optional) */
    eta_seconds?: number;
}

// ============================================================================
// TAURI BUILT-IN EVENTS (Tauri Framework → Frontend)
// ============================================================================

/**
 * File drop events from Tauri's built-in file drop functionality
 * 
 * Source: Tauri framework built-in events
 * Handler: src/ui/fileImport.ts
 */
export interface TauriFileDropEvents {
    /** Files dropped onto the application window */
    'tauri://file-drop': string[];
    
    /** User is hovering files over the drop area */
    'tauri://file-drop-hover': undefined;
    
    /** User cancelled the file drop operation */
    'tauri://file-drop-cancelled': undefined;
}

// ============================================================================
// COMPLETE EVENT CONTRACT
// ============================================================================

/**
 * Complete event contract for the audiobook processing application
 * 
 * This interface represents all events that flow between backend and frontend.
 * Any changes to this contract during refactoring should be carefully reviewed
 * to ensure backward compatibility.
 */
export interface ApplicationEvents extends TauriFileDropEvents {
    /** Progress updates during audiobook processing */
    'processing-progress': ProcessingProgressEvent;
}

// ============================================================================
// EVENT FLOW DOCUMENTATION
// ============================================================================

/**
 * EVENT FLOW DOCUMENTATION
 * 
 * 1. FILE DROP EVENTS:
 *    - User drags files over window → 'tauri://file-drop-hover'
 *    - User drops files → 'tauri://file-drop'
 *    - User cancels drop → 'tauri://file-drop-cancelled'
 *    
 * 2. PROCESSING EVENTS:
 *    - User clicks "Process Audiobook" → invoke('process_audiobook_files')
 *    - Backend emits progress → 'processing-progress' events
 *    - Frontend updates UI based on stage and percentage
 *    - Process completes → final 'processing-progress' with stage='completed'
 *    
 * 3. CANCELLATION FLOW:
 *    - User clicks "Cancel" → invoke('cancel_processing')
 *    - Backend sets cancellation flag
 *    - Backend emits 'processing-progress' with stage='cancelled'
 *    - Frontend resets to idle state
 */

// ============================================================================
// EVENT LISTENERS AND HANDLERS
// ============================================================================

/**
 * CURRENT EVENT LISTENER LOCATIONS:
 * 
 * File: src/ui/fileImport.ts
 * - listen('tauri://file-drop') → handleFileDrop()
 * - listen('tauri://file-drop-hover') → add drag-over CSS class
 * - listen('tauri://file-drop-cancelled') → remove drag-over CSS class
 * 
 * File: src/ui/statusPanel.ts  
 * - listen('processing-progress') → updateProgress() → updateStatus() → updateUI()
 * 
 * PROCESSING STATES (frontend):
 * - isProcessing: boolean flag in StatusPanel
 * - currentStatus: ProcessingStatus with stage, percentage, message, etc.
 * 
 * PROCESSING STAGES (backend ProcessingStage enum):
 * - Analyzing: File validation and preparation
 * - Converting: Audio conversion and merging with FFmpeg
 * - Merging: (legacy, now part of Converting)
 * - WritingMetadata: Adding metadata to final file
 * - Completed: Success state
 * - Failed: Error state with error message
 */

// ============================================================================
// EVENT PAYLOAD DETAILS
// ============================================================================

/**
 * PROCESSING PROGRESS EVENT DETAILS:
 * 
 * Stage Mapping (backend → frontend):
 * - ProcessingStage::Analyzing → "analyzing"
 * - ProcessingStage::Converting → "converting" 
 * - ProcessingStage::Merging → "merging" (legacy)
 * - ProcessingStage::WritingMetadata → "writing"
 * - ProcessingStage::Completed → "completed"
 * - ProcessingStage::Failed(_) → "failed"
 * 
 * Percentage Ranges:
 * - 0-10%: Initial validation and setup
 * - 10-80%: Audio conversion (mapped from FFmpeg progress)
 * - 80-95%: Metadata writing
 * - 95-98%: File moving and cleanup
 * - 100%: Completion
 * 
 * Message Examples:
 * - "Validating input files..."
 * - "Creating temporary workspace..."
 * - "Starting audio conversion..."
 * - "Converting and merging audio files..."
 * - "Writing metadata..."
 * - "Moving to final location..."
 * - "Cleaning up temporary files..."
 * - "Processing completed successfully!"
 */

// ============================================================================
// TYPE EXPORTS FOR RUNTIME USE
// ============================================================================

export type EventName = keyof ApplicationEvents;
export type EventPayload<T extends EventName> = ApplicationEvents[T];

/**
 * Type guard for processing progress events
 */
export function isProcessingProgressEvent(
    event: unknown
): event is ProcessingProgressEvent {
    const e = event as ProcessingProgressEvent;
    return (
        typeof e === 'object' &&
        e !== null &&
        typeof e.stage === 'string' &&
        typeof e.percentage === 'number' &&
        typeof e.message === 'string' &&
        ['analyzing', 'converting', 'merging', 'writing', 'completed', 'failed', 'cancelled'].includes(e.stage)
    );
}

/**
 * Type guard for file drop events
 */
export function isFileDropEvent(event: unknown): event is string[] {
    return Array.isArray(event) && event.every(item => typeof item === 'string');
}
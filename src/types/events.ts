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

import type {
	ProgressEvent as GeneratedProgressEvent,
	QueueEvent as GeneratedQueueEvent,
} from '../lib/generated/tauri';
import type { NullToOptionalDeep } from './ipc';

// ============================================================================
// EVENT CONSTANTS (P0.5.3)
// ============================================================================

/** Event name constants to prevent string drift */
export const EVENTS = {
	PROGRESS: 'processing-progress',
	QUEUE: 'processing-queue',
} as const;

/** Stage name constants to prevent string drift */
export const STAGES = {
	analyzing: 'analyzing',
	converting: 'converting',
	writing: 'writing',
	completed: 'completed',
	failed: 'failed',
	cancelled: 'cancelled',
} as const;

// ============================================================================
// PROCESSING EVENTS (Backend → Frontend)
// ============================================================================

/**
 * Progress event emitted by Rust backend during audio processing
 *
 * Source: src-tauri/src/audio/progress/mod.rs (ProgressEvent struct)
 * Handler: src/ui/statusPanel/events.ts (listen(EVENTS.PROGRESS))
 *
 * Emitted during:
 * - File analysis phase
 * - Audio conversion and merging
 * - Metadata writing
 * - Process completion/failure/cancellation
 */
type GeneratedProgressEventForUi = NullToOptionalDeep<GeneratedProgressEvent>;

export type ProcessingProgressEvent = Omit<GeneratedProgressEventForUi, 'stage'> & {
	/** Processing stage identifier */
	stage: keyof typeof STAGES;
};

export type ProcessingQueueItem = NullToOptionalDeep<GeneratedQueueEvent>['items'][number];

/**
 * Queue snapshot event emitted by Rust backend during batch processing
 *
 * Source: src-tauri/src/commands/audio_processing.rs
 * Handler: src/ui/statusPanel/events.ts (listen(EVENTS.QUEUE))
 */
export type ProcessingQueueEvent = NullToOptionalDeep<GeneratedQueueEvent>;

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
	/**
	 * Files dropped onto the application window
	 * Payload: { paths: string[], position: { x: number, y: number } }
	 */
	'tauri://drag-drop': { paths: string[]; position: { x: number; y: number } };

	/**
	 * User is hovering files over the drop area (drag enter)
	 * Payload: { paths: string[], position: { x: number, y: number } }
	 */
	'tauri://drag-enter': { paths: string[]; position: { x: number; y: number } };

	/**
	 * User is hovering files over the drop area (drag over)
	 * Payload: { position: { x: number, y: number } }
	 */
	'tauri://drag-over': { position: { x: number; y: number } };

	/**
	 * User cancelled the file drop operation (drag leave)
	 * Payload: null (due to known bug) or { type: 'leave' }
	 */
	'tauri://drag-leave': unknown;
}

// ============================================================================
// COMPLETE EVENT CONTRACT
// ============================================================================

/**
 * Complete event contract for the audiobook processing application
 *
 * This interface represents all events that flow between backend and frontend.
 * Any changes to this contract during refactoring should be carefully reviewed
 * to preserve backend/frontend event contract stability.
 */
export interface ApplicationEvents extends TauriFileDropEvents {
	/** Progress updates during audiobook processing */
	[EVENTS.PROGRESS]: ProcessingProgressEvent;
	/** Batch queue snapshot */
	[EVENTS.QUEUE]: ProcessingQueueEvent;
}

// ============================================================================
// EVENT FLOW DOCUMENTATION
// ============================================================================

/**
 * EVENT FLOW DOCUMENTATION
 *
 * 1. FILE DROP EVENTS:
 *    - User drags files over window → 'tauri://drag-enter' / 'tauri://drag-over'
 *    - User drops files → 'tauri://drag-drop'
 *    - User cancels drop (leaves window) → 'tauri://drag-leave'
 *
 * 2. PROCESSING EVENTS:
 *    - User clicks "Process Audiobook" → invoke('process_audiobook_files')
 *    - Backend emits progress → EVENTS.PROGRESS events
 *    - Frontend updates UI based on stage and percentage
 *    - Process completes → final EVENTS.PROGRESS with stage=STAGES.completed
 *
 * 3. CANCELLATION FLOW:
 *    - User clicks "Cancel" → invoke('cancel_processing')
 *    - Backend sets cancellation flag
 *    - Backend emits EVENTS.PROGRESS with stage=STAGES.cancelled
 *    - Frontend resets to idle state
 */

// ============================================================================
// EVENT LISTENERS AND HANDLERS
// ============================================================================

/**
 * CURRENT EVENT LISTENER LOCATIONS:
 *
 * File: src/ui/fileImport.ts
 * - listen('tauri://drag-drop') → handleFileDrop()
 * - listen('tauri://drag-enter') → add drag-over CSS class
 * - listen('tauri://drag-leave') → remove drag-over CSS class
 *
 * File: src/ui/statusPanel/events.ts
 * - listen(EVENTS.PROGRESS) → updateProgress() → updateStatus() → updateUI()
 * - listen(EVENTS.QUEUE) → handleQueueSnapshot() → queue/aggregate UI updates
 *
 * PROCESSING STATES (frontend):
 * - isProcessing: boolean flag in StatusPanel
 * - currentStatus: ProcessingStatus with stage, percentage, message, etc.
 *
 * PROCESSING STAGES (backend ProcessingStage enum):
 * - Analyzing: File validation and preparation
 * - Converting: Audio conversion and merging with FFmpeg
 * - Merging: Historical label retained for backward-compat parsing; backend now emits Converting
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
 * - ProcessingStage::WritingMetadata → "writing"
 * - ProcessingStage::Completed → "completed"
 * - ProcessingStage::Failed(_) → "failed"
 *
 * Percentage Ranges (matches `audio::constants`):
 * - 0-10%: Initial validation and setup
 * - 10-80%: Audio conversion (mapped from encoder timestamps and total duration)
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
export function isProcessingProgressEvent(event: unknown): event is ProcessingProgressEvent {
	const e = event as ProcessingProgressEvent;
	return (
		typeof e === 'object' &&
		e !== null &&
		typeof e.stage === 'string' &&
		typeof e.percentage === 'number' &&
		typeof e.message === 'string' &&
		Object.values(STAGES).includes(e.stage as keyof typeof STAGES)
	);
}

/**
 * Type guard for file drop events
 */
type DragDropPayload = TauriFileDropEvents['tauri://drag-drop'];

export function isFileDropEvent(event: unknown): event is DragDropPayload {
	const e = event as Partial<DragDropPayload>;
	return (
		typeof e === 'object' &&
		e !== null &&
		Array.isArray(e.paths) &&
		e.paths.every((item) => typeof item === 'string') &&
		typeof e.position === 'object' &&
		e.position !== null &&
		typeof e.position.x === 'number' &&
		typeof e.position.y === 'number'
	);
}

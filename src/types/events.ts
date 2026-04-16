/**
 * Frontend event contract for the Rust → TypeScript IPC seam.
 *
 * This module is the frontend-side index for events that flow from the Rust
 * backend (via tauri-specta) and from the Tauri runtime itself. Payload
 * shapes come from the specta-generated bindings in `src/lib/generated/tauri.ts`
 * — the canonical source of truth — and are re-exported here in UI-friendly
 * form (e.g. `null` collapsed to optional via `NullToOptionalDeep`).
 *
 * When stages or payload fields evolve, change the Rust types first, run
 * `bun run bindings:generate`, then update consumers in this file.
 */

import type {
	EventStage as GeneratedEventStage,
	ProgressEvent as GeneratedProgressEvent,
	QueueEvent as GeneratedQueueEvent,
} from '../lib/generated/tauri';
import type { NullToOptionalDeep } from './ipc';

// ============================================================================
// EVENT CONSTANTS
// ============================================================================

/** Event name constants to prevent string drift */
export const EVENTS = {
	PROGRESS: 'processing-progress',
	QUEUE: 'processing-queue',
} as const;

/**
 * Processing stage identifiers emitted on the `processing-progress` event.
 *
 * The canonical source of these values is the Rust `EventStage` enum
 * (`src-tauri/src/audio/progress/mod.rs`). The type alias below is re-exported
 * from the specta-generated bindings so adding a stage in Rust is a compile
 * error here until this file (and `STAGES`) is updated.
 */
export type EventStage = GeneratedEventStage;

/**
 * Readable, value-level access to stage identifiers.
 *
 * Typed as `{ [K in EventStage]: K }` so that this object must exhaustively
 * cover every variant of the generated `EventStage` union. If Rust adds a
 * variant, this declaration will fail to type-check.
 */
export const STAGES: { readonly [K in EventStage]: K } = {
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
 * Progress event emitted by Rust backend during audio processing.
 *
 * Source: `src-tauri/src/audio/progress/mod.rs` (`ProgressEvent` struct)
 * Handler: `src/ui/statusPanel/events.ts` (listen(EVENTS.PROGRESS))
 *
 * Emitted during:
 * - File analysis phase
 * - Audio conversion and merging
 * - Metadata writing
 * - Process completion/failure/cancellation
 *
 * Stage is a proper `EventStage` union (not a raw string) because the Rust
 * side now uses an enum that specta serializes as a snake_case string literal.
 */
export type ProcessingProgressEvent = NullToOptionalDeep<GeneratedProgressEvent>;

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
 * Complete event contract for the audiobook processing application.
 *
 * Combines Rust → frontend events (typed against the specta-generated
 * payloads, with the bindings drift gate guarding against silent breakage)
 * and Tauri runtime file-drop events.
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
 * PROCESSING STAGES: see the `EventStage` type (re-exported from
 * `src/lib/generated/tauri.ts`, sourced from the Rust `EventStage` enum in
 * `src-tauri/src/audio/progress/mod.rs`). Adding a stage is a single-source
 * change in Rust; specta regenerates the TS union automatically.
 */

// ============================================================================
// EVENT PAYLOAD DETAILS
// ============================================================================

/**
 * PROCESSING PROGRESS EVENT DETAILS:
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
 * Type guard for processing progress events.
 *
 * Validates the runtime shape of an arbitrary value so it can be narrowed to
 * `ProcessingProgressEvent`. The `stage` field is checked against the known
 * `EventStage` variants (exhaustively enumerated by `STAGES`).
 *
 * Note: the live event path in `src/lib/tauri/client.ts` is already narrowed
 * via the typed `normalizeProgressEvent` adapter, so this guard is intended
 * for ad-hoc/raw-listener consumers (e.g. tooling, smoke checks, or any
 * future use that bypasses `tauriClient`).
 */
const EVENT_STAGE_VALUES = Object.values(STAGES) as readonly EventStage[];

export function isProcessingProgressEvent(event: unknown): event is ProcessingProgressEvent {
	if (typeof event !== 'object' || event === null) {
		return false;
	}
	const e = event as Partial<ProcessingProgressEvent>;
	return (
		typeof e.percentage === 'number' &&
		typeof e.message === 'string' &&
		typeof e.stage === 'string' &&
		EVENT_STAGE_VALUES.includes(e.stage as EventStage)
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

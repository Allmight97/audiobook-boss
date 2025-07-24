# Implementation Plan: Progress & Status Interface

## Overview
Task 17 requires implementing a real-time progress panel that displays processing status, progress bars, stage indicators, and provides user control via a cancel button. Based on my analysis, I can see that:

1. **The HTML structure already has a status panel** in place (bottom of index.html)
2. **Backend progress infrastructure exists** with ProcessingProgress, ProcessingStage, and ProgressReporter
3. **The processor module needs enhancement** to emit Tauri events for real-time updates
4. **No cancellation mechanism exists yet** in the backend

## Current State Analysis

### Existing HTML Status Panel
The HTML already contains:
- Progress bar with percentage display
- Status text area
- Step text for current processing stage
- Process Audiobook button
- Art thumbnail placeholder

### Backend Infrastructure
- `ProcessingProgress` struct with stage, progress, current_file, files_completed, total_files, eta_seconds
- `ProcessingStage` enum: Analyzing, Converting, Merging, WritingMetadata, Completed, Failed
- `ProgressReporter` class that tracks progress and calculates percentages
- FFmpeg progress parsing capabilities

### Missing Components
1. **Event emission** from backend to frontend
2. **Cancel command** and cancellation state tracking
3. **Frontend TypeScript module** to manage the status panel
4. **Event listeners** for progress updates
5. **UI state management** during processing

## Implementation Plan

### Phase 1: Backend Enhancements

#### 1.1 Add Progress State Management
Create a shared state structure in the backend to track processing state and allow cancellation:

```rust
// In src-tauri/src/lib.rs or a new state module
pub struct ProcessingState {
    is_processing: Arc<Mutex<bool>>,
    is_cancelled: Arc<Mutex<bool>>,
    progress: Arc<Mutex<Option<ProcessingProgress>>>,
}
```

#### 1.2 Add Cancel Command
```rust
// In src-tauri/src/commands/mod.rs
#[tauri::command]
pub fn cancel_processing(state: tauri::State<ProcessingState>) -> Result<()> {
    let mut is_cancelled = state.is_cancelled.lock().unwrap();
    *is_cancelled = true;
    Ok(())
}
```

#### 1.3 Modify process_audiobook Command
Enhance the existing command to:
- Accept a Tauri window handle for event emission
- Check cancellation state periodically
- Emit progress events

```rust
#[tauri::command]
pub async fn process_audiobook_files(
    window: tauri::Window,
    state: tauri::State<'_, ProcessingState>,
    file_paths: Vec<String>,
    settings: AudioSettings,
    metadata: Option<AudiobookMetadata>
) -> Result<String> {
    // Set processing state
    // Create progress callback that emits events
    // Check cancellation during processing
}
```

#### 1.4 Progress Event Structure
```rust
#[derive(Clone, Serialize)]
struct ProgressEvent {
    stage: String,
    percentage: f32,
    message: String,
    current_file: Option<String>,
    eta_seconds: Option<f64>,
}
```

### Phase 2: Frontend Implementation

#### 2.1 Create statusPanel.ts Module
```typescript
// src/ui/statusPanel.ts
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

interface ProcessingStatus {
    stage: 'idle' | 'analyzing' | 'converting' | 'merging' | 'writing' | 'completed' | 'cancelled' | 'failed';
    percentage: number;
    message: string;
    currentFile?: string;
    etaSeconds?: number;
}

export class StatusPanel {
    private progressBar: HTMLElement;
    private percentageElement: HTMLElement;
    private statusText: HTMLElement;
    private stepText: HTMLElement;
    private processButton: HTMLButtonElement;
    private cancelUnlisten?: Function;
    private isProcessing: boolean = false;

    constructor() {
        this.initializeElements();
        this.setupEventHandlers();
    }

    private initializeElements(): void {
        // Get DOM elements
        // Set up initial state
    }

    private setupEventHandlers(): void {
        // Process button click handler
        // Set up progress event listener
    }

    public async startProcessing(): Promise<void> {
        // Validate inputs
        // Update UI to processing state
        // Listen for progress events
        // Call backend process command
    }

    public updateProgress(event: ProgressEvent): void {
        // Update progress bar
        // Update status text
        // Update step text
        // Handle completion/cancellation
    }

    private showCancelButton(): void {
        // Change Process button to Cancel
    }

    private async handleCancel(): Promise<void> {
        // Call cancel command
        // Update UI
    }

    private resetToIdle(): void {
        // Reset UI to idle state
        // Re-enable controls
    }
}
```

#### 2.2 Integration with main.ts
```typescript
// Add to src/main.ts
import { StatusPanel } from './ui/statusPanel';

let statusPanel: StatusPanel;

document.addEventListener('DOMContentLoaded', () => {
    // ... existing code ...
    statusPanel = new StatusPanel();
});

// Add test command
(window as any).testCommands.cancelProcessing = () => invoke('cancel_processing');
```

### Phase 3: Progress Event Flow

#### 3.1 Backend Event Emission Points
1. **File Analysis Stage**: Emit when analyzing each file
2. **Conversion Stage**: Parse FFmpeg output and emit percentage updates
3. **Merging Stage**: Track merge progress
4. **Metadata Writing**: Quick stage with fixed progress
5. **Completion/Error**: Final status emission

#### 3.2 Frontend Event Handling
```typescript
// Listen for progress events
const unlisten = await listen('processing-progress', (event) => {
    const progress = event.payload as ProgressEvent;
    this.updateProgress(progress);
});
```

### Phase 4: UI State Management

#### 4.1 Processing States
- **Idle**: Enable all controls, show "Process Audiobook" button
- **Processing**: Disable controls, show progress, enable "Cancel" button
- **Completed**: Show success message, auto-reset after delay
- **Cancelled**: Show cancellation message, reset immediately
- **Failed**: Show error message, enable retry

#### 4.2 Visual Feedback
- Progress bar with animated stripes (already styled in CSS)
- Stage-specific icons or colors
- ETA display when available
- Current file display during multi-file processing

### Phase 5: Error Handling & Edge Cases

1. **Handle backend crashes**: Timeout mechanism to detect stalled processing
2. **Network interruptions**: Graceful handling of event listener failures
3. **Invalid state transitions**: Prevent double-processing, handle rapid clicks
4. **Memory cleanup**: Properly unlisten events when processing completes

### Implementation Order

1. **Backend First** (2-3 hours):
   - Add ProcessingState to app state
   - Implement cancel command
   - Add event emission to processor
   - Test with console commands

2. **Frontend Structure** (1-2 hours):
   - Create statusPanel.ts
   - Wire up existing DOM elements
   - Basic state management

3. **Event Integration** (1-2 hours):
   - Connect event listeners
   - Test progress updates
   - Implement cancellation

4. **Polish & Testing** (1 hour):
   - Fine-tune progress calculations
   - Add animations/transitions
   - Test edge cases

### Testing Strategy

1. **Manual Testing**:
   ```javascript
   // Test processing
   window.testCommands.processAudiobook(filePaths, settings, metadata);
   
   // Test cancellation
   window.testCommands.cancelProcessing();
   
   // Simulate progress events (for testing UI)
   window.testCommands.simulateProgress = async () => {
       // Emit fake events
   };
   ```

2. **Test Scenarios**:
   - Process single small file
   - Process multiple large files
   - Cancel at different stages
   - Handle errors gracefully

### Success Criteria

✅ Real-time progress updates (< 100ms latency)
✅ Accurate percentage calculation based on file processing
✅ Clear stage indicators with descriptive messages
✅ Working cancel functionality that stops FFmpeg process
✅ Graceful error handling with user-friendly messages
✅ Smooth UI transitions between states
✅ No memory leaks from event listeners
✅ Accessible (keyboard navigation for cancel, screen reader support)

This implementation leverages the existing HTML structure and backend progress tracking while adding the missing event emission and cancellation features. The modular design ensures clean separation of concerns and maintainability.
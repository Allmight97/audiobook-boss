# StatusPanel Module Structure

This module provides status panel functionality for audiobook processing with a clean separation of concerns.

## Files Overview

### `/index.ts`
Public API aggregator that re-exports the main interfaces:
- `StatusPanel` class
- `initStatusPanel()` function 
- `getStatusPanel()` function

### `/logic.ts`
Core business logic and state management:
- Event handling for processing progress
- State coordination and status updates
- Backend communication (invoke calls)
- Cover art processing and metadata extraction
- Public API implementation

### `/dom.ts` 
DOM manipulation and UI update helpers:
- Element caching and lookup
- Progress bar and text updates
- Cover art display/reset
- Button state management
- Error/success message styling

### `/src/ui/statusPanel.ts` (Aggregator Shim)
Re-exports everything from the modular implementation to maintain import compatibility:
```typescript
export * from './statusPanel/index';
```

## Architecture

The split follows a clean separation pattern:
- **Logic**: Handles events, state, and business rules
- **DOM**: Pure view layer with no business logic
- **Index**: Public interface contract
- **Shim**: Backwards compatibility

This design allows for easier testing, maintenance, and future enhancements while preserving the existing public API contract.

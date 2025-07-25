# Load Cover Art Feature Test Plan

## Manual Testing Steps

### 1. Backend Testing (via console)
```javascript
// Test the backend command directly
window.testCommands.loadCoverArtFile('/path/to/test/image.jpg')
```

### 2. Frontend Testing 
1. Click the "Load Cover Art" button in the Metadata & Output panel
2. Select an image file (jpg, png, webp) 
3. Verify cover art displays in the metadata panel
4. Check that cover art is included in metadata operations

### 3. Integration Testing
1. Load audio files
2. Load cover art
3. Verify cover art shows in progress panel thumbnail during processing
4. Verify final M4B file includes cover art

### 4. Error Testing
1. Try to load non-image files (should show error)
2. Try to load corrupted image files
3. Try to cancel file dialog (should handle gracefully)

## Expected Behavior

### Success Cases
- Cover art displays in metadata panel
- File dialog opens with image file filters
- Image is properly validated and loaded
- Cover art is included in metadata operations
- Status panel thumbnail shows cover art during processing

### Error Cases  
- Invalid file formats show user-friendly error messages
- File access errors are handled gracefully
- UI remains functional after errors

## Files Modified
- `/src-tauri/src/commands/mod.rs` - Added `load_cover_art_file` command
- `/src-tauri/src/lib.rs` - Registered new command
- `/src/ui/coverArt.ts` - New cover art handling module
- `/src/main.ts` - Integrated cover art initialization and test functions
- `/src/ui/outputPanel.ts` - Include cover art in metadata operations
- `/src/ui/fileList.ts` - Use new cover art module for display
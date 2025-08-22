// Debug script to test browse button functionality
console.log('=== Browse Button Debug Script ===');

// Check if DOM elements exist
const browseButton = document.getElementById('output-dir-browse');
const outputDirText = document.getElementById('output-dir-text');

console.log('Browse button element:', browseButton);
console.log('Output dir text element:', outputDirText);

if (browseButton) {
    console.log('Browse button found!');
    console.log('Button innerHTML:', browseButton.innerHTML);
    console.log('Button classes:', browseButton.className);
    console.log('Button disabled:', browseButton.disabled);
    
    // Check event listeners
    const listeners = getEventListeners ? getEventListeners(browseButton) : 'getEventListeners not available';
    console.log('Event listeners:', listeners);
    
    // Test click manually
    console.log('Testing manual click...');
    browseButton.click();
} else {
    console.error('Browse button NOT found!');
}

// Check if @tauri-apps/plugin-dialog is available
try {
    const dialog = window.__TAURI_PLUGIN_DIALOG__;
    console.log('Tauri dialog plugin available:', !!dialog);
    
    // Try to access the open function directly
    import('@tauri-apps/plugin-dialog').then(module => {
        console.log('Dialog module imported successfully:', module);
        console.log('Open function available:', typeof module.open);
    }).catch(err => {
        console.error('Failed to import dialog module:', err);
    });
} catch (error) {
    console.error('Error accessing Tauri dialog plugin:', error);
}

// Check if outputPanel module is loaded
if (window.testCommands) {
    console.log('Test commands available');
    try {
        const settings = window.testCommands.getCurrentAudioSettings();
        console.log('Current audio settings:', settings);
    } catch (err) {
        console.log('Error getting audio settings (expected if no directory selected):', err.message);
    }
} else {
    console.log('Test commands not available');
}
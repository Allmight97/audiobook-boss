import { initFrontendErrorLogBridge } from './frontendLogBridge';

// Side-effect bootstrap: evaluated before the App.svelte dependency tree so
// first-render failures reach the bridge (see src/main.ts ordering contract).
initFrontendErrorLogBridge();

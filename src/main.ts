import './styles.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { initFrontendErrorLogBridge } from './lib/frontendLogBridge';

// Before mount: failures during the first render or module evaluation of the
// component tree are exactly the blank-screen class the bridge must capture.
initFrontendErrorLogBridge();

const target = document.getElementById('app');
if (!target) {
	throw new Error('App root #app not found');
}

mount(App, { target });

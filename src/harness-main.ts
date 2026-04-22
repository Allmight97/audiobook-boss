import './styles.css';
import { mount } from 'svelte';
import HarnessApp from './HarnessApp.svelte';
import { installHarnessRuntime } from './harness/runtime';

const target = document.getElementById('app');
if (!target) {
	throw new Error('Harness root #app not found');
}

installHarnessRuntime();
mount(HarnessApp, { target });

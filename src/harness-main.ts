import './styles.css';
import { mount } from 'svelte';
import HarnessApp from './HarnessApp.svelte';

const target = document.getElementById('app');
if (!target) {
	throw new Error('Harness root #app not found');
}

mount(HarnessApp, { target });

import './styles.css';
import './lib/frontendLogBridge.install';
import { mount } from 'svelte';
import App from './App.svelte';

const target = document.getElementById('app');
if (!target) {
	throw new Error('App root #app not found');
}

mount(App, { target });

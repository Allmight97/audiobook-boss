import '../styles.css';
import { mount } from 'svelte';
import DirectionV3Shell from './DirectionV3Shell.svelte';

const target = document.getElementById('prototype');
if (!target) {
	throw new Error('Prototype root #prototype not found');
}

mount(DirectionV3Shell, { target });

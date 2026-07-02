import '../styles.css';
import { mount } from 'svelte';
import LabIsland from './LabIsland.svelte';

const target = document.getElementById('lab');
if (!target) {
	throw new Error('Lab root #lab not found');
}

mount(LabIsland, { target });

import './install';
import '../styles.css';
import '../lib/frontendLogBridge.install';
import { render } from 'solid-js/web';
import { MockRoot } from './MockRoot';

const target = document.getElementById('app');
if (!target) {
	throw new Error('App root #app not found');
}

export const disposeMockApp = render(() => <MockRoot />, target);

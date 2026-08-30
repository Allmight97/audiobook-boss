import '../styles.css';
import { render } from 'solid-js/web';
import { Lab } from './Lab';

const target = document.getElementById('lab');
if (!target) {
	throw new Error('Lab root #lab not found');
}

export const disposeLab = render(() => <Lab />, target);

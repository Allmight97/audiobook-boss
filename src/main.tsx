import './styles.css';
import './lib/frontendLogBridge.install';
import { render } from '@solidjs/web';
import { ProductionRoot } from './app/runtime/ProductionRoot';

const target = document.getElementById('app');
if (!target) {
	throw new Error('App root #app not found');
}

render(() => <ProductionRoot />, target);

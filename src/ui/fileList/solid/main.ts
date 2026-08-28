import '../../../styles.css';
import { mountFileListIsland } from './mount';

const target = document.getElementById('file-list-solid');
if (!target) {
	throw new Error('File List Solid root #file-list-solid not found');
}

mountFileListIsland(target);

import '../../../styles.css';
import { mountFileListIsland } from './mount';

const target = document.getElementById('file-list-solid2');
if (!target) {
	throw new Error('File List Solid 2 root #file-list-solid2 not found');
}

mountFileListIsland(target);

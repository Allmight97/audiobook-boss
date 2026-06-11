export function coverArtBytesToDataUrl(coverArtBytes: number[]): string {
	const uint8Array = new Uint8Array(coverArtBytes);
	const chunkSize = 0x8000;
	let binary = '';
	for (let offset = 0; offset < uint8Array.length; offset += chunkSize) {
		binary += String.fromCharCode(...uint8Array.subarray(offset, offset + chunkSize));
	}
	const base64String = btoa(binary);

	let mimeType = 'image/jpeg';
	if (coverArtBytes.length >= 12) {
		if (coverArtBytes[0] === 0x89 && coverArtBytes[1] === 0x50) {
			mimeType = 'image/png';
		} else if (
			coverArtBytes[0] === 0x52 &&
			coverArtBytes[1] === 0x49 &&
			coverArtBytes[2] === 0x46 &&
			coverArtBytes[3] === 0x46 &&
			coverArtBytes[8] === 0x57 &&
			coverArtBytes[9] === 0x45 &&
			coverArtBytes[10] === 0x42 &&
			coverArtBytes[11] === 0x50
		) {
			mimeType = 'image/webp';
		}
	} else if (coverArtBytes.length >= 2 && coverArtBytes[0] === 0x89 && coverArtBytes[1] === 0x50) {
		mimeType = 'image/png';
	}

	return `data:${mimeType};base64,${base64String}`;
}

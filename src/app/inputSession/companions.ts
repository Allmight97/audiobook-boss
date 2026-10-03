/** How the companion PDFs of the given titles read in the inspector. */
export type CompanionSummary = {
	readonly text: string;
	readonly title: string;
};

export function companionSummary(
	companions: Readonly<Record<string, readonly string[]>>,
	inputIds: ReadonlyArray<string | undefined>,
): CompanionSummary {
	const names = inputIds.map((inputId) => (inputId ? (companions[inputId] ?? []) : []));
	const pdfCount = names.reduce((count, files) => count + files.length, 0);
	const title = Array.from(new Set(names.flat())).join(', ');
	if (inputIds.length === 0) return { text: '---', title };
	if (pdfCount === 0) return { text: 'None', title };
	if (inputIds.length === 1) {
		return { text: pdfCount === 1 ? title || 'PDF attached' : `${pdfCount} PDFs attached`, title };
	}
	const text = `${pdfCount} ${pdfCount === 1 ? 'PDF' : 'PDFs'} across ${inputIds.length} selected files`;
	return { text, title: text };
}

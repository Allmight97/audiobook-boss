#!/usr/bin/env bun
import { readdir, stat } from 'node:fs/promises';
import path from 'node:path';

const FILE_WARN_THRESHOLD = 50_000;
const SIZE_WARN_THRESHOLD = 10 * 1024 * 1024 * 1024;

type ScanResult = {
	directories: number;
	files: number;
	sizeBytes: number;
};

const repoRoot = path.resolve(import.meta.dir, '..', '..');

function targetRoot(): string {
	const configured = process.env.CARGO_TARGET_DIR;
	if (!configured) {
		return path.join(repoRoot, 'target');
	}

	return path.isAbsolute(configured) ? configured : path.resolve(repoRoot, configured);
}

function formatBytes(bytes: number): string {
	const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
	let value = bytes;
	let unitIndex = 0;
	while (value >= 1024 && unitIndex < units.length - 1) {
		value /= 1024;
		unitIndex += 1;
	}

	return `${value.toFixed(unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

async function scanDirectory(directory: string): Promise<ScanResult> {
	const result: ScanResult = { directories: 0, files: 0, sizeBytes: 0 };
	const entries = await readdir(directory, { withFileTypes: true });

	for (const entry of entries) {
		const entryPath = path.join(directory, entry.name);
		if (entry.isDirectory()) {
			result.directories += 1;
			const child = await scanDirectory(entryPath);
			result.directories += child.directories;
			result.files += child.files;
			result.sizeBytes += child.sizeBytes;
			continue;
		}

		result.files += 1;
		result.sizeBytes += (await stat(entryPath)).size;
	}

	return result;
}

function warningsFor(scan: ScanResult): string[] {
	const warnings: string[] = [];
	if (scan.files > FILE_WARN_THRESHOLD) {
		warnings.push(`file count exceeds ${FILE_WARN_THRESHOLD.toLocaleString()}`);
	}
	if (scan.sizeBytes > SIZE_WARN_THRESHOLD) {
		warnings.push(`apparent size exceeds ${formatBytes(SIZE_WARN_THRESHOLD)}`);
	}
	return warnings;
}

async function main(): Promise<void> {
	const root = targetRoot();
	const depsDir = path.join(root, 'debug', 'deps');

	console.log(`[rust-target] target root: ${root}`);
	console.log(`[rust-target] debug/deps: ${depsDir}`);

	try {
		await stat(depsDir);
	} catch {
		console.log('[rust-target] status: OK (debug/deps does not exist)');
		return;
	}

	const scan = await scanDirectory(depsDir);
	const warnings = warningsFor(scan);

	console.log(`[rust-target] debug/deps files: ${scan.files.toLocaleString()}`);
	console.log(`[rust-target] debug/deps directories: ${scan.directories.toLocaleString()}`);
	console.log(`[rust-target] debug/deps apparent size: ${formatBytes(scan.sizeBytes)}`);
	console.log(
		warnings.length > 0
			? `[rust-target] warning: ${warnings.join('; ')}`
			: '[rust-target] status: OK',
	);
}

await main();

import { mkdir, readFile, writeFile, appendFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

export const PERF_DIR = resolve(__dirname, '..');
export const RESULTS_DIR = resolve(PERF_DIR, 'results');
export const BASELINES_DIR = resolve(PERF_DIR, 'baselines');
export const REPO_ROOT = resolve(PERF_DIR, '..', '..');

export function nowIso() {
	return new Date().toISOString();
}

export function toAbsolutePath(pathLike) {
	if (!pathLike) return pathLike;
	return resolve(REPO_ROOT, pathLike);
}

export async function ensureDir(pathLike) {
	await mkdir(pathLike, { recursive: true });
}

export async function readJson(pathLike, fallback = null) {
	try {
		const raw = await readFile(pathLike, 'utf8');
		return JSON.parse(raw);
	} catch (error) {
		// FALLBACK[FB-017]: trigger=optional perf artifact absent on disk (ENOENT)
		// observe=callers explicitly pass fallback defaults and resulting reports indicate missing baselines/history
		// sunset=2026-06-30 issue=#195
		if (error && error.code === 'ENOENT') {
			return fallback;
		}
		throw error;
	}
}

export async function writeJson(pathLike, value) {
	await ensureDir(dirname(pathLike));
	await writeFile(pathLike, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

export async function appendNdjson(pathLike, value) {
	await ensureDir(dirname(pathLike));
	await appendFile(pathLike, `${JSON.stringify(value)}\n`, 'utf8');
}

export async function readNdjson(pathLike) {
	try {
		const raw = await readFile(pathLike, 'utf8');
		return raw
			.split('\n')
			.map((line) => line.trim())
			.filter(Boolean)
			.map((line) => JSON.parse(line));
	} catch (error) {
		// FALLBACK[FB-017]: trigger=perf history file not created yet (ENOENT)
		// observe=report output indicates empty history and rebuilds from first run
		// sunset=2026-06-30 issue=#195
		if (error && error.code === 'ENOENT') {
			return [];
		}
		throw error;
	}
}

export async function writeText(pathLike, value) {
	await ensureDir(dirname(pathLike));
	await writeFile(pathLike, value, 'utf8');
}

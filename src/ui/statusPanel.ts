/**
 * FALLBACK[FB-018]: trigger=legacy imports still target src/ui/statusPanel.ts path
 * observe=static import graph + test/build references continue to resolve through this shim
 * sunset=framework migration phase 2 completion, no later than 2026-06-30 issue=#195
 *
 * StatusPanel aggregator shim
 * 
 * This file maintains import compatibility by re-exporting the 
 * StatusPanel implementation from the new modular structure.
 */

export * from './statusPanel/index';

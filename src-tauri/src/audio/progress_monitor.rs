//! Progress monitoring and process execution management
//!
//! This module handles all aspects of progress tracking during audio processing,
//! including FFmpeg process monitoring, progress calculation, display formatting,
//! and process lifecycle management.

use super::constants::*;
use super::context::ProcessingContext;
use super::progress::ProgressEmitter;
use crate::errors::{AppError, Result};
use crate::ffmpeg::FFmpegError;
use std::io::{BufRead, BufReader};
use std::process::{Command, Child};

// Progress estimation constants
const MIN_PROGRESS_UPDATES_FOR_ESTIMATION: i32 = 5;
const MIN_PROGRESS_RATIO_FOR_ESTIMATION: f64 = 0.1;

/// Process execution state for tracking progress
pub struct ProcessExecution {
    pub child: Child,
    pub emitter: ProgressEmitter,
    pub last_progress_time: f32,
    pub estimated_total_time: f64,
    pub progress_count: i32,
}

/// Sets up FFmpeg process and initial state
pub fn setup_process_execution(
    mut cmd: Command,
    context: &ProcessingContext,
) -> Result<ProcessExecution> {
    let child = cmd.spawn()
        .map_err(|_| AppError::FFmpeg(FFmpegError::ExecutionFailed("Failed to start FFmpeg".to_string())))?;
    
    let emitter = ProgressEmitter::new(context.window.clone());
    
    Ok(ProcessExecution {
        child,
        emitter,
        last_progress_time: 0.0,
        estimated_total_time: 0.0,
        progress_count: 0,
    })
}

/// Monitors FFmpeg process output and handles progress updates
pub fn monitor_process_with_progress(
    execution: &mut ProcessExecution,
    context: &ProcessingContext,
    total_duration: f64,
) -> Result<()> {
    if let Some(stderr) = execution.child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            check_cancellation_and_kill_context(context, &mut execution.child)?;
            
            let line = line.map_err(|_| AppError::FFmpeg(FFmpegError::ExecutionFailed("Error reading FFmpeg output".to_string())))?;
            
            handle_progress_line(&line, execution, context, total_duration)?;
        }
    }
    Ok(())
}

/// Handles a single line of FFmpeg output for progress and error checking
pub fn handle_progress_line(
    line: &str,
    execution: &mut ProcessExecution,
    _context: &ProcessingContext,
    total_duration: f64,
) -> Result<()> {
    let speed_multiplier = parse_speed_multiplier(line);

    // Parse progress from FFmpeg output and emit events
    if let Some(progress_time) = crate::audio::progress::parse_ffmpeg_progress(line) {
        process_progress_update_context(
            progress_time,
            &mut execution.last_progress_time,
            &mut execution.progress_count,
            &mut execution.estimated_total_time,
            total_duration,
            speed_multiplier,
            &execution.emitter,
        )?;
    }
    
    // Check for errors (but ignore case-insensitive matches in file paths)
    if (line.contains("Error") || line.contains("error")) && 
       !line.contains("Output") && !line.contains("Input") {
        log::error!("FFmpeg error line: {line}");
        if line.contains("No such file") || line.contains("Invalid data") {
            log::error!("FFmpeg critical error: {line}");
            return Err(AppError::FFmpeg(FFmpegError::ExecutionFailed(
                format!("FFmpeg failed to process audio files: {line}")
            )));
        }
    }
    
    Ok(())
}

/// Waits for process completion and checks exit status
pub fn finalize_process_execution(
    mut execution: ProcessExecution,
    context: &ProcessingContext,
) -> Result<()> {
    // Check if process was cancelled before waiting
    if context.is_cancelled() {
        log::info!("Processing cancelled before FFmpeg completion");
        return Err(AppError::InvalidInput("Processing was cancelled by user before FFmpeg completion".to_string()));
    }
    
    // Wait for completion only if not cancelled
    let status = execution.child.wait()
        .map_err(|e| {
            let msg = format!("Failed to wait for FFmpeg process completion: {e}");
            log::error!("{msg}");
            AppError::FFmpeg(FFmpegError::ExecutionFailed(msg))
        })?;
    
    if !status.success() {
        let exit_code = status.code()
            .map(|c| format!(" (exit code: {c})"))
            .unwrap_or_default();
        let msg = format!("FFmpeg process failed during audio conversion{exit_code}");
        log::error!("{msg}");
        // At this point stderr has been consumed during monitoring. We cannot re-read it,
        // but we can hint where to look for the cause via prior logs.
        return Err(AppError::FFmpeg(FFmpegError::ExecutionFailed(msg)));
    }
    
    Ok(())
}

/// Checks for cancellation and kills process if needed (context-based)
pub fn check_cancellation_and_kill_context(
    context: &ProcessingContext,
    child: &mut Child,
) -> Result<()> {
    if context.is_cancelled() {
        log::debug!("Cancellation detected, killing FFmpeg process...");
        let _ = child.kill();
        
        // Wait for process to actually terminate
        for i in 0..PROCESS_TERMINATION_MAX_ATTEMPTS {  // Try for 2 seconds max
            if let Ok(Some(_)) = child.try_wait() {
                log::debug!("FFmpeg process terminated successfully");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(PROCESS_TERMINATION_CHECK_DELAY_MS));
            if i == PROCESS_TERMINATION_MAX_ATTEMPTS - 1 {
                log::warn!("FFmpeg process may not have terminated cleanly");
            }
        }
        // Best-effort reap to avoid zombie processes
        let _ = child.wait();
        return Err(AppError::InvalidInput("Processing was cancelled".to_string()));
    }
    Ok(())
}

/// Processes progress update and emits events (context-based)
pub fn process_progress_update_context(
    progress_time: f32,
    last_progress_time: &mut f32,
    progress_count: &mut i32,
    estimated_total_time: &mut f64,
    total_duration: f64,
    speed_multiplier: Option<f64>,
    emitter: &ProgressEmitter,
) -> Result<()> {
    if progress_time == PROGRESS_COMPLETE {
        handle_progress_completion(emitter);
    } else if progress_time > *last_progress_time {
        *last_progress_time = progress_time;
        *progress_count += 1;
        
        update_time_estimation(estimated_total_time, *progress_count, total_duration, progress_time);
        
        let progress_percentage = calculate_and_display_progress(
            progress_time,
            *estimated_total_time,
            *progress_count,
            speed_multiplier,
        );
        
        let eta_seconds = if let Some(speed) = speed_multiplier {
            let remaining_time = (*estimated_total_time - progress_time as f64) / speed;
            if remaining_time > 0.0 { Some(remaining_time) } else { None }
        } else {
            None
        };
        
        emitter.emit_converting_progress(
            progress_percentage.min(PROGRESS_CONVERTING_MAX as f64) as f32,
            "Converting and merging audio files...",
            None,
            eta_seconds,
        );
    }
    Ok(())
}

/// Handles completion state when progress reaches 100%
pub fn handle_progress_completion(emitter: &ProgressEmitter) {
    eprint!("\rConverting: Done!                                          \n");
    // Transition UI away from converting (79%) into finalization stage
    emitter.emit_finalizing("Finalizing audio conversion...");
}

/// Updates time estimation based on current progress
pub fn update_time_estimation(
    estimated_total_time: &mut f64,
    progress_count: i32,
    total_duration: f64,
    progress_time: f32,
) {
    if *estimated_total_time == 0.0 && progress_count > MIN_PROGRESS_UPDATES_FOR_ESTIMATION {
        *estimated_total_time = total_duration;
    } else if progress_count > MIN_PROGRESS_UPDATES_FOR_ESTIMATION && progress_time > 0.0 {
        let progress_ratio = progress_time as f64 / total_duration;
        if progress_ratio > MIN_PROGRESS_RATIO_FOR_ESTIMATION {
            *estimated_total_time = total_duration;
        }
    }
}

/// Calculates and displays progress information
pub fn calculate_and_display_progress(
    progress_time: f32,
    estimated_total_time: f64,
    progress_count: i32,
    speed_multiplier: Option<f64>,
) -> f64 {
    if estimated_total_time > 0.0 {
        let file_progress = (progress_time as f64 / estimated_total_time).min(1.0);
        let speed_text = speed_multiplier
            .map(|s| format!(" [Speed: {s:.1}x]"))
            .unwrap_or_default();
        let eta_text = if let Some(speed) = speed_multiplier {
            let remaining_time = (estimated_total_time - progress_time as f64) / speed;
            if remaining_time > 0.0 {
                let minutes = (remaining_time / SECONDS_PER_MINUTE) as u32;
                let seconds = (remaining_time % SECONDS_PER_MINUTE) as u32;
                format!(" [ETA: {minutes}m {seconds}s]")
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        
        display_progress_with_duration(file_progress, progress_time, estimated_total_time, &speed_text, &eta_text)
    } else {
        display_analysis_progress(progress_count)
    }
}

/// Displays progress with known duration
pub fn display_progress_with_duration(
    file_progress: f64,
    progress_time: f32,
    estimated_total_time: f64,
    speed_text: &str,
    eta_text: &str,
) -> f64 {
    let percentage = PROGRESS_CONVERTING_START as f64 + (file_progress * PROGRESS_RANGE_MULTIPLIER);
    
    eprint!("\rConverting: {:.1}% ({:.1}s / {:.1}s){}{}", 
        file_progress * 100.0, 
        progress_time, 
        estimated_total_time,
        speed_text,
        eta_text);
    
    percentage
}

/// Displays progress during analysis phase
pub fn display_analysis_progress(progress_count: i32) -> f64 {
    let percentage = PROGRESS_CONVERTING_START as f64 + ((progress_count as f64).min(MAX_INITIAL_PROGRESS_COUNT) * ANALYSIS_PROGRESS_MULTIPLIER);
    eprint!("\rConverting: {percentage:.1}% (analyzing...)");
    percentage
}

/// Parses speed multiplier from FFmpeg output line
pub fn parse_speed_multiplier(line: &str) -> Option<f64> {
    if let Some(speed_start) = line.find("speed=") {
        let speed_part = &line[speed_start + 6..];
        if let Some(speed_end) = speed_part.find('x') {
            let speed_str = &speed_part[..speed_end].trim();
            speed_str.parse::<f64>().ok()
        } else {
            None
        }
    } else {
        None
    }
}

// ADAPTER FUNCTIONS for backward compatibility

/// Processes progress update and emits events (ADAPTER)
/// 
/// ADAPTER FUNCTION: Maintains backward compatibility by converting parameters
/// to use the new ProgressEmitter approach internally.
#[deprecated = "Use process_progress_update_context for new code - this adapter maintains compatibility"]
#[allow(dead_code)]
pub fn process_progress_update(
    progress_time: f32,
    last_progress_time: &mut f32,
    progress_count: &mut i32,
    estimated_total_time: &mut f64,
    total_duration: f64,
    speed_multiplier: Option<f64>,
    window: &tauri::Window,
) -> Result<()> {
    let emitter = ProgressEmitter::new(window.clone());
    process_progress_update_context(
        progress_time,
        last_progress_time,
        progress_count,
        estimated_total_time,
        total_duration,
        speed_multiplier,
        &emitter,
    )
}

/// Checks for cancellation and kills process if needed (ADAPTER)
/// 
/// ADAPTER FUNCTION: Maintains backward compatibility by converting parameters
/// to use the new context-based approach internally.
#[deprecated = "Use check_cancellation_and_kill_context for new code - this adapter maintains compatibility"]
#[allow(dead_code)]
pub fn check_cancellation_and_kill(
    state: &tauri::State<'_, crate::ProcessingState>,
    child: &mut Child,
) -> Result<()> {
    let is_cancelled = state.is_cancelled.lock()
        .map_err(|_| AppError::InvalidInput("Failed to check cancellation state".to_string()))?;
    
    if *is_cancelled {
        log::debug!("Cancellation detected, killing FFmpeg process...");
        let _ = child.kill();
        
        // Wait for process to actually terminate
        for i in 0..PROCESS_TERMINATION_MAX_ATTEMPTS {  // Try for 2 seconds max
            if let Ok(Some(_)) = child.try_wait() {
                log::debug!("FFmpeg process terminated successfully");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(PROCESS_TERMINATION_CHECK_DELAY_MS));
            if i == PROCESS_TERMINATION_MAX_ATTEMPTS - 1 {
                log::warn!("FFmpeg process may not have terminated cleanly");
            }
        }
        // Best-effort reap to avoid zombie processes
        let _ = child.wait();
        return Err(AppError::InvalidInput("Processing was cancelled".to_string()));
    }
    Ok(())
}

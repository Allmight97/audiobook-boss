//! The batch queue: the imported titles and the live preview or export queue.

use std::borrow::Cow;

use abb_engine::audio::AudioFile;
use abb_engine::work_runtime::ChildJobStatus;
use rune::tok::{self, type_};
use rune::{
    Cx, El, FileDialogOptions, GrowthSpec, ScrollSpec, TextStyle, col, el, row, text, text_input,
};

use crate::state::{self, State};

pub fn screen(state: &State, _: &mut Cx<State>) -> El {
    let theme = tok::theme(true);
    let mut shell = col()
        .key("batch-queue")
        .w_full()
        .h_full()
        .padding(16.0)
        .gap(12.0)
        .fill(if state.drag_over {
            theme.card
        } else {
            theme.bg
        })
        .on_drop_files(|state: &mut State, files, _| state.import_paths(files.to_vec()))
        .accepts_files(state::accepts_drop)
        .on_drop_hover(|state: &mut State, over, _| state.drag_over = over);
    shell = shell.child(header(state));
    shell = shell.child(banner(state));
    shell.child(body(state))
}

fn header(state: &State) -> El {
    let theme = tok::theme(true);
    let field = text_input(&state.output_dir, type_::BODY)
        .key("output-dir")
        .w_full()
        .min_h(36.0)
        .padding(8.0)
        .radius(8.0)
        .fill(theme.card)
        .single_line()
        .placeholder("Output folder", theme.text_secondary)
        .on_edit(|state: &mut State| &mut state.output_dir)
        .on_submit(|state: &mut State, _| state.commit_output_directory());
    let folder = if state.engine_directory.is_empty() {
        "Engine folder: none".to_string()
    } else {
        format!("Engine folder: {}", state.engine_directory)
    };
    col()
        .gap(8.0)
        .w_full()
        .child(
            row()
                .w_full()
                .items_center()
                .gap(8.0)
                .child(
                    text("Batch queue")
                        .font(TextStyle::new(20.0, 600.0))
                        .color(theme.text),
                )
                .child(el().grow(1.0))
                .child(button("add-files", "Add file", open_file))
                .child(button("add-folder", "Add folder", open_folder))
                .child(button("preview", "Preview", |state, _| {
                    state.dispatch(abb_engine::session::SessionIntent::Preview);
                }))
                .child(button("start", "Start processing", |state, _| {
                    state.dispatch(abb_engine::session::SessionIntent::Submit);
                }))
                .child(button("cancel", "Cancel", |state, _| {
                    state.cancel_current()
                })),
        )
        .child(field)
        .child(
            text(folder)
                .key("engine-folder")
                .font(type_::LABEL)
                .color(theme.text_secondary),
        )
}

fn banner(state: &State) -> El {
    let theme = tok::theme(true);
    let message = if state.banner.is_empty() {
        "Drop audio files or a folder on the window.".to_string()
    } else {
        state.banner.clone()
    };
    text(message)
        .key("banner")
        .font(type_::BODY)
        .color(theme.text_secondary)
}

fn body(state: &State) -> El {
    let mut titles = title_pane(state);
    let mut queue = queue_pane(state);
    if !state.layout_bug {
        titles = titles.min_h(0.0);
        queue = queue.min_h(0.0);
    }
    let mut line = row()
        .w_full()
        .grow(1.0)
        .gap(12.0)
        .child(titles)
        .child(queue);
    if !state.layout_bug {
        line = line.min_h(0.0);
    }
    line
}

fn title_pane(state: &State) -> El {
    let theme = tok::theme(true);
    let heading = format!("{} titles", state.files.len());
    let mut column = col().grow(1.0).gap(8.0).child(
        row()
            .w_full()
            .items_center()
            .gap(8.0)
            .child(text("Titles").font(type_::TITLE).color(theme.text))
            .child(
                text(heading)
                    .key("title-count")
                    .font(type_::LABEL)
                    .color(theme.text_secondary),
            )
            .child(
                text(state.visible.clone())
                    .key("visible-range")
                    .font(type_::LABEL)
                    .color(theme.text_secondary),
            ),
    );
    if !state.layout_bug {
        column = column.min_h(0.0);
    }
    column.child(title_scroll(state))
}

fn title_scroll(state: &State) -> El {
    let theme = tok::theme(true);
    let mut viewport = col()
        .key("title-scroll")
        .w_full()
        .grow(1.0)
        .fill(theme.card)
        .radius(10.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        });
    if !state.layout_bug {
        viewport = viewport.min_h(0.0);
    }
    if state.files.is_empty() {
        return viewport.padding(12.0).child(
            text("No titles yet.")
                .font(type_::BODY)
                .color(theme.text_secondary),
        );
    }
    let rows = col()
        .key("title-rows")
        .w_full()
        .growth(GrowthSpec::default())
        .virtual_rows(
            state.files.len(),
            44.0,
            |state: &State, index| Cow::Borrowed(state.files[index].input_id.as_str()),
            title_row,
        )
        .virtual_rows_version(state.files_revision())
        .on_visible_rows(|state: &mut State, range| {
            let label = format!(
                "showing {}-{} of {}",
                range.start + 1,
                range.end,
                state.files.len()
            );
            if state.visible == label {
                return false;
            }
            state.visible = label;
            true
        });
    viewport.child(rows)
}

fn title_row(state: &State, index: usize) -> El {
    let theme = tok::theme(true);
    let file = &state.files[index];
    let title_id = file.input_id.clone();
    let selected = state.selected.contains(&title_id);
    row()
        .key(title_id.clone())
        .w_full()
        .h(44.0)
        .padding(8.0)
        .gap(8.0)
        .items_center()
        .fill(if selected {
            theme.control_off
        } else {
            rune::Color::TRANSPARENT
        })
        .on_press(move |state: &mut State| state.select_title(title_id.clone()))
        .child(
            text(state::title_status_label(file.status))
                .font(type_::LABEL)
                .color(status_color(file)),
        )
        .child(
            text(file.label.clone())
                .font(type_::BODY)
                .color(theme.text)
                .grow(1.0)
                .shrink(),
        )
        .child(
            text(clock(file.duration))
                .font(type_::LABEL)
                .color(theme.text_secondary),
        )
}

fn queue_pane(state: &State) -> El {
    let theme = tok::theme(true);
    let operation = state.queue_operation();
    let percent = operation
        .map(|item| item.progress.percentage)
        .unwrap_or(0.0);
    let step = operation
        .map(|item| item.progress.message.clone())
        .filter(|message| !message.is_empty())
        .unwrap_or_else(|| "Idle".into());
    let submission = state
        .output
        .as_ref()
        .and_then(|output| output.submission.as_ref())
        .map(state::submission_line)
        .unwrap_or_else(|| step.clone());
    let mut column = col()
        .w(340.0)
        .gap(8.0)
        .fill(theme.card)
        .radius(10.0)
        .padding(12.0);
    if !state.layout_bug {
        column = column.min_h(0.0);
    }
    column
        .child(text("Queue").font(type_::TITLE).color(theme.text))
        .child(chips(state))
        .child(progress_bar(percent))
        .child(
            text(format!("{percent:.0}%"))
                .key("progress-readout")
                .font(type_::BODY)
                .color(theme.text),
        )
        .child(
            text(submission)
                .key("queue-step")
                .font(type_::LABEL)
                .color(theme.text_secondary),
        )
        .child(queue_scroll(state))
}

fn chips(state: &State) -> El {
    let theme = tok::theme(true);
    let children = state
        .queue_operation()
        .map(|operation| operation.children.as_slice())
        .unwrap_or(&[]);
    let count = |status: ChildJobStatus| {
        children
            .iter()
            .filter(|child| child.status == status)
            .count()
    };
    row()
        .key("queue-chips")
        .gap(6.0)
        .child(chip(
            format!("{} running", count(ChildJobStatus::Running)),
            tok::accent(0, true),
        ))
        .child(chip(
            format!("{} queued", count(ChildJobStatus::Queued)),
            theme.control_off,
        ))
        .child(chip(
            format!(
                "{} complete",
                count(ChildJobStatus::Completed) + count(ChildJobStatus::Skipped)
            ),
            theme.indicator,
        ))
        .child(chip(
            format!("{} failed", count(ChildJobStatus::Failed)),
            theme.control_track,
        ))
}

fn chip(label: String, fill: rune::Color) -> El {
    el().padding(6.0)
        .radius(8.0)
        .fill(fill)
        .child(text(label).font(type_::LABEL).color(tok::theme(true).text))
}

fn progress_bar(percent: f32) -> El {
    let theme = tok::theme(true);
    let width = 300.0 * (percent / 100.0).clamp(0.0, 1.0);
    el().key("progress")
        .h(8.0)
        .w_full()
        .radius(4.0)
        .fill(theme.control_off)
        .child(el().h(8.0).w(width).radius(4.0).fill(tok::accent(0, true)))
}

fn queue_scroll(state: &State) -> El {
    let theme = tok::theme(true);
    let count = state
        .queue_operation()
        .map(|operation| operation.children.len())
        .unwrap_or(0);
    let mut viewport = col()
        .key("queue-scroll")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        });
    if count == 0 {
        return viewport.child(
            text("Nothing queued.")
                .font(type_::BODY)
                .color(theme.text_secondary),
        );
    }
    let rows = col()
        .key("queue-rows")
        .w_full()
        .growth(GrowthSpec::default())
        .virtual_rows(count, 36.0, queue_key, queue_row);
    viewport = viewport.child(rows);
    viewport
}

fn queue_key(state: &State, index: usize) -> Cow<'_, str> {
    state
        .queue_operation()
        .and_then(|operation| operation.children.get(index))
        .map(|child| Cow::Borrowed(child.child_job_id.as_str()))
        .unwrap_or_else(|| Cow::Owned(format!("missing-{index}")))
}

fn queue_row(state: &State, index: usize) -> El {
    let theme = tok::theme(true);
    let Some(child) = state
        .queue_operation()
        .and_then(|operation| operation.children.get(index))
    else {
        return el();
    };
    let child_id = child.child_job_id.clone();
    let cancellable = child.cancellable;
    let mut line = row()
        .key(child_id.clone())
        .w_full()
        .h(36.0)
        .gap(8.0)
        .items_center()
        .child(
            text(state::child_status_label(child.status))
                .font(type_::LABEL)
                .color(theme.text_secondary),
        )
        .child(
            text(child.label.clone())
                .font(type_::BODY)
                .color(theme.text)
                .grow(1.0)
                .shrink(),
        )
        .child(
            text(format!("{:.0}%", child.progress.percentage))
                .font(type_::LABEL)
                .color(theme.text),
        );
    if cancellable {
        line = line.child(
            el().key(format!("cancel-{child_id}"))
                .padding(4.0)
                .radius(6.0)
                .fill(theme.control_off)
                .child(text("Cancel").font(type_::LABEL).color(theme.text))
                .on_press(move |state: &mut State| state.cancel_child(child_id.clone())),
        );
    }
    line
}

fn button(key: &'static str, label: &'static str, on_press: fn(&mut State, &mut rune::Tx)) -> El {
    let theme = tok::theme(true);
    el().key(key)
        .role("button")
        .label(label)
        .focusable(true)
        .padding(8.0)
        .radius(8.0)
        .fill(theme.control_off)
        .on_press_tx(on_press)
        .child(text(label).font(type_::BODY).color(theme.text))
}

fn open_file(_state: &mut State, tx: &mut rune::Tx) {
    let extensions = state::audio_filters();
    let borrowed: Vec<&str> = extensions.iter().map(String::as_str).collect();
    let options = FileDialogOptions {
        title: "Add audio".into(),
        ..FileDialogOptions::default()
    }
    .filter("Audio", &borrowed);
    tx.open_file(options, |state: &mut State, result, _| match result {
        Ok(Some(path)) => state.import_paths(vec![path]),
        Ok(None) => {}
        Err(error) => state.banner = error.to_string(),
    });
}

fn open_folder(_state: &mut State, tx: &mut rune::Tx) {
    let options = FileDialogOptions {
        title: "Add folder".into(),
        ..FileDialogOptions::default()
    };
    tx.open_folder(options, |state: &mut State, result, _| match result {
        Ok(Some(path)) => state.import_paths(vec![path]),
        Ok(None) => {}
        Err(error) => state.banner = error.to_string(),
    });
}

fn status_color(file: &AudioFile) -> rune::Color {
    let theme = tok::theme(true);
    match file.status {
        abb_engine::audio::TitleStatus::Ready => tok::accent(0, true),
        abb_engine::audio::TitleStatus::Failed => theme.text,
        abb_engine::audio::TitleStatus::RemovedFinished => theme.text_secondary,
    }
}

fn clock(seconds: Option<f64>) -> String {
    let Some(seconds) = seconds else {
        return "--:--".into();
    };
    let total = seconds.round().max(0.0) as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

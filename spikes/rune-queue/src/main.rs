//! Throwaway Rune host for ABB's batch queue. It drives `abb-engine` the way
//! the Tauri host does, and it is not a product UI.

mod state;
mod view;

use std::sync::atomic::Ordering;
use std::time::Instant;

use rune::{AppSpec, Cx, El, Tx, Waker};

use state::State;

static STARTED: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

fn boot() -> Instant {
    *STARTED.get_or_init(Instant::now)
}

fn init() -> State {
    let _ = boot();
    State::from_args(rune::extra_args())
}

fn scene_collapsed(state: &mut State) {
    state.layout_bug = true;
}

fn setup(state: &mut State, waker: Waker) {
    state.attach(waker, boot());
}

fn poll(state: &mut State, _seconds: f64, tx: &mut Tx) -> bool {
    state.on_frame(boot(), tx)
}

fn view(state: &State, context: &mut Cx<State>) -> El {
    view::screen(state, context)
}

fn main() {
    let _ = boot();
    rune::run(AppSpec {
        scenes: &[("collapsed", scene_collapsed)],
        setup: Some(setup),
        poll: Some(poll),
        ..AppSpec::new("Batch queue", (1180.0, 740.0), init, view)
    });
    if state::probe_requested() && !state::ENGINE_STARTED.load(Ordering::Relaxed) {
        std::process::exit(1);
    }
}

use crossterm::{
    self,
    event::EventStream,
    event::{Event, KeyCode, KeyEvent, KeyEventKind},
};
use futures::TryStreamExt;

use crate::flame_graph::{FlameGraph, FlameGraphState, MockNode, MockNodeMetadata};

mod app;

mod flame_graph;
mod livloc_app;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let node = MockNode::new(
        100,
        vec![
            (
                MockNodeMetadata::new("foo"),
                MockNode::new(
                    30,
                    vec![
                        (MockNodeMetadata::new("foo_bar"), MockNode::new(20, vec![])),
                        (MockNodeMetadata::new("foo_baz"), MockNode::new(10, vec![])),
                    ],
                ),
            ),
            (
                MockNodeMetadata::new("bar"),
                MockNode::new(
                    70,
                    vec![
                        (MockNodeMetadata::new("bar_qux"), MockNode::new(40, vec![])),
                        (MockNodeMetadata::new("bar_juzz"), MockNode::new(25, vec![])),
                        (MockNodeMetadata::new("bar_joox"), MockNode::new(5, vec![])),
                    ],
                ),
            ),
        ],
    );

    // let mut events = EventStream::new();
    // events.try_next().await?;

    run_async(|mut term| async move {
        let mut state = FlameGraphState::new();

        loop {
            term.draw(|frame| {
                let flame_graph = FlameGraph { root: &node };
                frame.render_stateful_widget(flame_graph, frame.area(), &mut state);
            })?;

            let ev = crossterm::event::read()?;
            #[allow(clippy::single_match)]
            match ev {
                Event::Key(KeyEvent {
                    code,
                    kind: KeyEventKind::Press,
                    ..
                }) => match code {
                    KeyCode::Left => state.select_left(),
                    KeyCode::Right => state.select_right(),
                    KeyCode::Up => state.select_up(),
                    KeyCode::Down => state.select_down(),
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    _ => {}
                },
                _ => {}
            }
        }

        eyre::Ok(())
    })
    .await?;

    Ok(())
}

async fn run_async<F, Fut>(f: F) -> Fut::Output
where
    F: FnOnce(ratatui::DefaultTerminal) -> Fut,
    Fut: Future,
{
    let terminal = ratatui::init();
    let result = f(terminal).await;
    ratatui::restore();
    result
}

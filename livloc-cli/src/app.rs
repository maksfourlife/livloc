use std::io;

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
use futures::{
    StreamExt,
    channel::mpsc::{UnboundedSender, unbounded},
    stream,
};
use ratatui::Frame;

pub trait App {
    fn draw(&mut self, frame: &mut Frame<'_>);
    /// Returns whether should withdraw frame
    fn handle_event(&mut self, ev: Event) -> HandleResult;
    fn register_sender(
        &mut self,
        sender: UnboundedSender<AppEvent>,
    ) -> impl Future<Output = eyre::Result<()>>;
}

pub enum AppEvent {
    Redraw,
}

pub enum HandleResult {
    Skipped(Event),
    Processed { redraw: bool },
}

pub async fn run_async<F, Fut>(f: F) -> Fut::Output
where
    F: FnOnce(ratatui::DefaultTerminal) -> Fut,
    Fut: Future,
{
    let terminal = ratatui::init();
    let result = f(terminal).await;
    ratatui::restore();
    result
}

pub async fn run_app(app: &mut impl App) -> eyre::Result<()> {
    let (sender, receiver) = unbounded();
    app.register_sender(sender).await?;

    let events = EventStream::new();

    enum EventKind {
        Term(io::Result<Event>),
        App(AppEvent),
    }

    let mut stream = stream::select(events.map(EventKind::Term), receiver.map(EventKind::App));

    run_async(|mut term| async move {
        let mut draw = true;

        loop {
            if draw {
                term.draw(|frame| app.draw(frame))?;
            }

            match stream.next().await {
                Some(EventKind::Term(result)) => match app.handle_event(result?) {
                    HandleResult::Skipped(ev) => match ev {
                        Event::Key(KeyEvent {
                            code,
                            kind: KeyEventKind::Press,
                            ..
                        }) => match code {
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            _ => {}
                        },
                        _ => {}
                    },
                    HandleResult::Processed { redraw } => draw = redraw,
                },
                Some(EventKind::App(AppEvent::Redraw)) => {
                    draw = true;
                }
                None => {}
            }
        }
    })
    .await
}

use crossterm::event::{Event, EventStream};
use futures::{TryStreamExt, future};
use ratatui::Frame;

pub trait App {
    type NotifiedFut: Future + Unpin;

    fn draw(&mut self, frame: &mut Frame<'_>);
    /// Returns whether should withdraw frame
    fn handle_event(&mut self, ev: Event) -> bool;
    /// Wait for redraw notification
    fn notified(&self) -> Self::NotifiedFut;
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
    let mut events = EventStream::new();

    run_async(|mut term| async move {
        let mut next_event = events.try_next();
        let mut notified_redraw = app.notified();

        let mut draw = true;

        loop {
            if draw {
                term.draw(|frame| app.draw(frame))?;
            }

            let selected = future::select(next_event, notified_redraw).await;

            match selected {
                future::Either::Left((result, fut)) => {
                    if let Some(ev) = result? {
                        draw = app.handle_event(ev);
                    }
                    next_event = events.try_next();
                    notified_redraw = fut;
                }
                future::Either::Right((_, fut)) => {
                    draw = true;
                    notified_redraw = app.notified();
                    next_event = fut;
                }
            }
        }
    })
    .await
}

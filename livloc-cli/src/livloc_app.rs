use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use bytes::Bytes;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, MouseEvent, MouseEventKind};
use eyre::Context;
use futures::{
    SinkExt,
    channel::mpsc::{Receiver, Sender, UnboundedSender, channel},
};
use livloc_proto::{Tree, codec::Decode};
use ouroboros::self_referencing;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::Style,
    text::{Line, Span},
};
use reqwest::Url;
use tokio::{task::JoinHandle, time::interval};

use crate::{
    app::{App, AppEvent, HandleResult},
    flame_graph::{FlameGraph, FlameGraphState, find_node},
};

pub struct LivlocApp {
    tree: Arc<Mutex<ParsedTree>>,
    last_error: Arc<Mutex<Option<eyre::Report>>>,
    sender_sender: Sender<UnboundedSender<AppEvent>>,
    flame_graph_state: FlameGraphState,
    handle: JoinHandle<eyre::Result<()>>,
}

#[self_referencing]
struct ParsedTree {
    data: Bytes,
    #[covariant]
    #[borrows(data)]
    tree: Tree<'this>,
}

pub struct AppOptions {
    pub tree_url: Url,
    pub update_period: Duration,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            tree_url: "http://127.0.0.1:3000".parse().unwrap(),
            update_period: Duration::from_secs(10),
        }
    }
}

impl LivlocApp {
    pub async fn with(client: reqwest::Client, opts: AppOptions) -> eyre::Result<Self> {
        let tree = fetch_tree(&client, opts.tree_url.clone()).await?;

        let tree = Arc::new(Mutex::new(tree));
        let last_error = Arc::new(Mutex::new(None));

        let (sender, receiver) = channel(1);

        let background = Background {
            client,
            tree: tree.clone(),
            last_error: last_error.clone(),
            sender_receiver: receiver,
            tree_url: opts.tree_url,
            update_period: opts.update_period,
        };
        let handle = tokio::spawn(background.run());

        Ok(Self {
            tree,
            last_error,
            flame_graph_state: FlameGraphState::new(),
            sender_sender: sender,
            handle,
        })
    }
}

struct Background {
    client: reqwest::Client,
    tree: Arc<Mutex<ParsedTree>>,
    last_error: Arc<Mutex<Option<eyre::Report>>>,
    sender_receiver: Receiver<UnboundedSender<AppEvent>>,
    tree_url: Url,
    update_period: Duration,
}

impl Background {
    async fn run(mut self) -> eyre::Result<()> {
        let mut interval = interval(self.update_period);
        let mut senders = vec![];

        loop {
            tokio::select! {
                result = self.sender_receiver.recv() => {
                    senders.push(result?);
                }
                _ = interval.tick() => {
                    let result = fetch_tree(&self.client, self.tree_url.clone()).await;
                    match result {
                        Ok(ok) => {
                            *self.tree.lock().unwrap() = ok;
                            *self.last_error.lock().unwrap() = None;
                        }
                        Err(err) => *self.last_error.lock().unwrap() = Some(err),
                    }
                    for sender in &mut senders {
                        sender.send(AppEvent::Redraw).await?;
                    }
                }
            }
        }
    }
}

async fn fetch_tree(client: &reqwest::Client, tree_url: Url) -> eyre::Result<ParsedTree> {
    let response = client.get(tree_url).send().await?.error_for_status()?;
    let content = response.bytes().await?;
    ParsedTree::try_new(content, |bytes| Tree::decode(&mut &**bytes))
        .context("could not parse tree")
}

impl App for LivlocApp {
    fn draw(&mut self, frame: &mut Frame<'_>) {
        let layout =
            Layout::vertical([Constraint::Length(4), Constraint::Fill(1)]).split(frame.area());

        let tree = self.tree.lock().unwrap();
        let last_error = self.last_error.lock().unwrap();

        let flame_graph = FlameGraph::new(tree.borrow_tree());
        frame.render_stateful_widget(flame_graph, layout[1], &mut self.flame_graph_state);

        let header_layout = Layout::vertical([Constraint::Length(1); 4]).split(layout[0]);

        // render after frame so path is corrected
        if let Some((meta, node)) = find_node(tree.borrow_tree(), self.flame_graph_state.selected())
        {
            let label_style = Style::new().bold();

            let line1 = Line::from_iter([
                Span::styled("Name: ", label_style),
                Span::raw(
                    meta.symbols
                        .first()
                        .and_then(|x| x.name.as_deref())
                        .unwrap_or("<unresolved>"),
                ),
            ]);
            frame.render_widget(line1, header_layout[0]);

            let line2 = Line::from_iter([
                Span::styled("Size: ", label_style),
                Span::raw(format!("{}, ", node.size)),
                Span::styled("Count: ", label_style),
                Span::raw(node.count.to_string()),
            ]);
            frame.render_widget(line2, header_layout[1]);
        }

        if let Some(err) = &*last_error {
            let error_style = Style::new().light_red();
            let label_style = error_style.bold();

            let line = Line::from_iter([
                Span::styled("Last error: ", label_style),
                Span::styled(err.to_string(), error_style),
            ]);
            frame.render_widget(line, header_layout[2]);
        }

        frame.render_widget(
            format!("{:?}", self.flame_graph_state.selected()),
            header_layout[3],
        );
    }

    fn handle_event(&mut self, ev: Event) -> HandleResult {
        match ev {
            Event::Key(KeyEvent {
                code,
                kind: KeyEventKind::Press,
                ..
            }) => match code {
                KeyCode::Left => {
                    self.flame_graph_state.select_left();
                    HandleResult::Processed { redraw: true }
                }
                KeyCode::Right => {
                    self.flame_graph_state.select_right();
                    HandleResult::Processed { redraw: true }
                }
                KeyCode::Up => {
                    self.flame_graph_state.select_up();
                    HandleResult::Processed { redraw: true }
                }
                KeyCode::Down => {
                    self.flame_graph_state.select_down();
                    HandleResult::Processed { redraw: true }
                }
                KeyCode::Enter => {
                    self.flame_graph_state.push_origin();
                    HandleResult::Processed { redraw: true }
                }
                KeyCode::Char(' ') => {
                    self.flame_graph_state.pop_origin();
                    HandleResult::Processed { redraw: true }
                }
                _ => HandleResult::Skipped(ev),
            },
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(_),
                column,
                row,
                ..
            }) => {
                self.flame_graph_state.select_pos((column, row).into());
                HandleResult::Processed { redraw: true }
            }
            _ => HandleResult::Skipped(ev),
        }
    }

    async fn register_sender(&mut self, sender: UnboundedSender<AppEvent>) -> eyre::Result<()> {
        self.sender_sender.send(sender).await?;
        Ok(())
    }
}

use std::{borrow::Cow, cmp};

use livloc_proto::{Frame, Tree};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::StatefulWidget,
};

pub struct FlameGraph<'a, T> {
    root: &'a T,
}

pub trait Node: Sized {
    type Metadata: NodeMetadata;
    fn size(&self) -> usize;
    fn children(&self) -> &[(Self::Metadata, Self)];
}

pub trait NodeMetadata {
    fn name(&self) -> Cow<'_, str>;
}

#[derive(Debug, Default)]
pub struct FlameGraphState {
    selected_path: Vec<usize>,
}

impl<'a, T> FlameGraph<'a, T> {
    pub fn new(root: &'a T) -> Self {
        Self { root }
    }
}

impl FlameGraphState {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn selected(&self) -> &[usize] {
        &self.selected_path
    }

    pub fn select_up(&mut self) {
        self.selected_path.push(0);
    }

    pub fn select_down(&mut self) {
        self.selected_path.pop();
    }

    pub fn select_left(&mut self) {
        if let Some(last) = self.selected_path.last_mut() {
            *last = last.saturating_sub(1);
        }
    }

    pub fn select_right(&mut self) {
        if let Some(last) = self.selected_path.last_mut() {
            *last = last.saturating_add(1);
        }
    }
}

impl<T: Node> StatefulWidget for FlameGraph<'_, T> {
    type State = FlameGraphState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        render_node(self.root, area, buf, &mut state.selected_path);
    }
}

/// # Parameters
/// * `path`: selected path relative to the current node
fn render_node(node: &impl Node, area: Rect, buf: &mut Buffer, path: &mut [usize]) {
    const HOR_PAD: u16 = 0;
    const VER_PAD: u16 = 0;
    const NODE_HEIGHT: u16 = 1;

    const BG_COLORS: [Color; 4] = [Color::Red, Color::Yellow, Color::Magenta, Color::LightRed];
    const FG_COLOR: Color = Color::Black;

    let total_hor_padding = node.children().len() as u16 * HOR_PAD + 1;

    let Some(avail_width) = area.width.checked_sub(total_hor_padding) else {
        // total width is less than total width padding
        return;
    };

    let mut node_x = area.x;
    let node_y = area.height - NODE_HEIGHT;

    // if node has no children but path len is greater then 0 we trim it ???

    for (i, (metadata, child)) in node.children().iter().enumerate() {
        let node_width = (avail_width as usize * child.size() / node.size()) as u16;
        let node_rect = Rect::new(node_x, node_y, node_width, NODE_HEIGHT);

        // correct path index at current level to be in bounds
        if let Some(first) = path.first_mut() {
            *first = cmp::min(*first, node.children().len() - 1);
        }

        let path_contains_curr = matches!(path.first(), Some(first) if *first == i);
        let curr_selected = path_contains_curr && path.len() == 1;

        let bg_color = if curr_selected {
            Color::Blue
        } else {
            BG_COLORS[i % BG_COLORS.len()]
        };

        buf.set_style(node_rect, Style::new().bg(bg_color).fg(FG_COLOR));
        buf.set_stringn(
            node_rect.x,
            node_rect.y,
            metadata.name(),
            node_rect.width as usize,
            Style::new().bg(bg_color).fg(FG_COLOR),
        );

        let child_area = Rect::new(node_rect.x, area.y, node_width, node_y - VER_PAD);

        let child_path = if path_contains_curr {
            &mut path[1..]
        } else {
            &mut []
        };

        render_node(child, child_area, buf, child_path);

        node_x += node_width + HOR_PAD;
    }
}

pub fn find_node<'a, T: Node>(root: &'a T, path: &[usize]) -> Option<&'a (T::Metadata, T)> {
    let (&first, path) = path.split_first()?;
    let mut curr = root.children().get(first)?;

    for &index in path {
        curr = curr.1.children().get(index)?;
    }

    Some(curr)
}

pub struct MockNode {
    pub size: usize,
    pub children: Vec<(MockNodeMetadata, Self)>,
}

pub struct MockNodeMetadata {
    pub name: &'static str,
}

impl MockNode {
    pub fn new(size: usize, children: Vec<(MockNodeMetadata, Self)>) -> Self {
        Self { size, children }
    }
}

impl MockNodeMetadata {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl NodeMetadata for MockNodeMetadata {
    fn name(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.name)
    }
}

impl Node for MockNode {
    type Metadata = MockNodeMetadata;

    fn size(&self) -> usize {
        self.size
    }

    fn children(&self) -> &[(Self::Metadata, Self)] {
        &self.children
    }
}

impl<'a> Node for Tree<'a> {
    type Metadata = Frame<'a>;

    fn size(&self) -> usize {
        self.size
    }

    fn children(&self) -> &[(Self::Metadata, Self)] {
        &self.children
    }
}

impl NodeMetadata for Frame<'_> {
    fn name(&self) -> Cow<'_, str> {
        // TODO: print address
        Cow::Borrowed(
            self.symbols
                .first()
                .and_then(|x| x.name.as_deref())
                .unwrap_or("<unresolved>"),
        )
    }
}

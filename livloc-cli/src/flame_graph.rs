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
    /// points to path segments and marks joints to view tree from
    origins: Vec<usize>,
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

    pub fn pop_origin(&mut self) {
        self.origins.pop();
    }

    pub fn push_origin(&mut self) {
        let lesser_or_empty = match self.origins.last() {
            Some(&last) => last < self.selected_path.len(),
            None => true,
        };
        if lesser_or_empty {
            self.origins.push(self.selected_path.len());
        }
    }
}

impl<T: Node> StatefulWidget for FlameGraph<'_, T> {
    type State = FlameGraphState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        correct_path(self.root, &mut state.selected_path);
        // TODO: correct original
        let (origin, path) = if let Some(origin_index) = state.origins.last() {
            let (origin_path, relative_path) = state.selected_path.split_at(*origin_index);
            let (_, origin) = find_node(self.root, origin_path).unwrap();
            (origin, relative_path)
        } else {
            (self.root, &*state.selected_path)
        };

        render_node(origin, area, buf, path, 0);
    }
}

/// # Parameters
/// * `path`: selected path relative to the current node
fn render_node(node: &impl Node, area: Rect, buf: &mut Buffer, path: &[usize], mut count: usize) {
    const HOR_PAD: u16 = 0;
    const VER_PAD: u16 = 0;
    const NODE_HEIGHT: u16 = 1;

    const BG_COLORS: [Color; 4] = [Color::Red, Color::Yellow, Color::Magenta, Color::LightRed];
    const FG_COLOR: Color = Color::Black;

    if node.size() == 0 {
        return;
    }

    let total_hor_padding = node.children().len() as u16 * HOR_PAD;

    let Some(avail_width) = area.width.checked_sub(total_hor_padding) else {
        // total width is less than total width padding
        return;
    };

    let max_y = area.y + area.height;

    let mut node_x = area.x;

    let Some(node_y) = max_y.checked_sub(NODE_HEIGHT) else {
        return;
    };

    for (i, (metadata, child)) in node.children().iter().enumerate() {
        if child.size() > node.size() {
            // TODO: display error or skip
        }

        // path should skip items which width is 0 (to do that need last direction)

        let node_width =
            (avail_width as u128 * (child.size() as u128) / (node.size() as u128)) as u16;
        let node_rect = Rect::new(node_x, node_y, node_width, NODE_HEIGHT);

        let path_contains_curr = matches!(path.first(), Some(first) if *first == i);
        let curr_selected = path_contains_curr && path.len() == 1;

        let bg_color = if curr_selected {
            Color::Blue
        } else {
            BG_COLORS[count % BG_COLORS.len()]
        };

        buf.set_style(node_rect, Style::new().bg(bg_color).fg(FG_COLOR));
        buf.set_stringn(
            node_rect.x,
            node_rect.y,
            metadata.name(),
            node_rect.width as usize,
            Style::new().bg(bg_color).fg(FG_COLOR),
        );

        count += 1;

        if let Some(child_height) = area.height.checked_sub(NODE_HEIGHT) {
            let child_area = Rect::new(node_rect.x, area.y, node_width, child_height);
            let child_path = if path_contains_curr { &path[1..] } else { &[] };

            render_node(child, child_area, buf, child_path, count);

            // node_x += node_width + HOR_PAD;
            node_x = node_x
                .checked_add(node_width)
                .ok_or_else(|| {
                    format!(
                        "overflow {node_x} + {node_width} ({} / {})",
                        child.size(),
                        node.size()
                    )
                })
                .unwrap();
        }
    }
}

/// Root node has no metadata
pub fn find_node<'a, T: Node>(root: &'a T, path: &[usize]) -> Option<&'a (T::Metadata, T)> {
    let (&first, path) = path.split_first()?;
    let mut curr = root.children().get(first)?;

    for &index in path {
        curr = curr.1.children().get(index)?;
    }

    Some(curr)
}

fn correct_path<T: Node>(root: &T, path: &mut Vec<usize>) {
    let mut curr = root;
    for (i, segment) in path.iter_mut().enumerate() {
        if let Some(max_index) = curr.children().len().checked_sub(1) {
            *segment = cmp::min(*segment, max_index);
            let x = &curr.children()[*segment];
            curr = &x.1;
        } else {
            // children are empty
            path.truncate(i);
            break;
        }
    }
}

#[derive(Debug, Default)]
pub struct MockNode {
    pub size: usize,
    pub children: Vec<(MockNodeMetadata, Self)>,
}

#[derive(Debug, Default)]
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

#[cfg(test)]
mod test_correct_path {
    use crate::flame_graph::{MockNode, correct_path};

    #[test]
    fn test_1_node() {
        let node = MockNode {
            size: 0,
            children: vec![],
        };
        let mut path = vec![0];
        correct_path(&node, &mut path);
        assert_eq!(path, &[]);
    }

    #[test]
    fn test_2_node_extra_segment() {
        let node = MockNode {
            size: 0,
            children: vec![(
                Default::default(),
                MockNode {
                    size: 0,
                    children: vec![],
                },
            )],
        };
        let mut path = vec![0, 1];
        correct_path(&node, &mut path);
        assert_eq!(path, &[0]);
    }

    #[test]
    fn test_2_nodes_index_greater() {
        let node = MockNode {
            size: 0,
            children: vec![(
                Default::default(),
                MockNode {
                    size: 0,
                    children: vec![],
                },
            )],
        };
        let mut path = vec![1];
        correct_path(&node, &mut path);
        assert_eq!(path, &[0]);
    }

    #[test]
    fn test_3_nodes_2nd_index_greater() {
        let node = MockNode {
            size: 0,
            children: vec![(
                Default::default(),
                MockNode {
                    size: 0,
                    children: vec![(
                        Default::default(),
                        MockNode {
                            size: 0,
                            children: vec![],
                        },
                    )],
                },
            )],
        };
        let mut path = vec![0, 1];
        correct_path(&node, &mut path);
        assert_eq!(path, &[0, 0]);
    }
}

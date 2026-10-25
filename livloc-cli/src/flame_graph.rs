use std::{borrow::Cow, cmp};

use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Color, Style},
    widgets::StatefulWidget,
};

// TODO:
// [ ] skip nodes which width is zero when correcting path
// [ ] jump to sibling node when going horizontally
// [x] select node by click

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

#[derive(Debug)]
enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Default)]
pub struct FlameGraphState {
    selected_path: Vec<usize>,
    pending_dirs: Vec<Direction>,
    pending_pos: Option<Position>,
}

struct Paths {
    selected: Vec<usize>,
    origin: Option<Box<Self>>,
}

impl Paths {
    fn selected(&self) -> &[usize] {
        &self.selected
    }

    fn origin(&self) -> &[usize] {
        self.origin.as_ref().map(|x| &*x.selected).unwrap_or(&[])
    }

    fn push(self) -> Self {
        Self {
            selected: vec![],
            origin: Some(Box::new(self)),
        }
    }

    fn pop(self) -> Self {
        if let Some(x) = self.origin { *x } else { self }
    }
}

struct Frame {
    origin_path: Vec<usize>,
    /// path from the origin node to the selected node
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

    pub fn select_pos(&mut self, pos: Position) {
        self.pending_pos = Some(pos);
    }

    pub fn selected(&self) -> &[usize] {
        &self.selected_path
    }

    pub fn select_up(&mut self) {
        self.pending_dirs.push(Direction::Up);
    }

    pub fn select_down(&mut self) {
        self.pending_dirs.push(Direction::Down);
    }

    pub fn select_left(&mut self) {
        self.pending_dirs.push(Direction::Left);
    }

    pub fn select_right(&mut self) {
        self.pending_dirs.push(Direction::Right);
    }

    pub fn pop_origin(&mut self) {
        todo!()
        // self.origins.pop();
    }

    pub fn push_origin(&mut self) {
        todo!()
        // let lesser_or_empty = match self.origins.last() {
        //     Some(&last) => last < self.selected_path.len(),
        //     None => true,
        // };
        // if lesser_or_empty {
        //     self.origins.push(self.selected_path.len());
        // }
    }
}

impl<T: Node> StatefulWidget for FlameGraph<'_, T> {
    type State = FlameGraphState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if let Some(selected_pos) = state.pending_pos.take()
            && let Some(path) = find_path_by_pos(self.root, area, selected_pos)
        {
            state.selected_path = path;
            // TODO: nullify all origins
        }

        // correct_path(self.root, &mut state.selected_path);
        // TODO: correct original
        // let (origin, path) = if let Some(origin_index) = state.origins.last() {
        //     let (origin_path, relative_path) = state.selected_path.split_at(*origin_index);
        //     let (_, origin) = find_node_with_meta(self.root, origin_path).unwrap();
        //     (origin, relative_path)
        // } else {
        //     (self.root, &*state.selected_path)
        // };

        update_path(self.root, &mut state.selected_path, &mut state.pending_dirs);

        render_node(self.root, area, buf, &state.selected_path, 0);
    }
}

const NODE_HEIGHT: u16 = 1;

/// # Parameters
/// * `path`: selected path relative to the current node
fn render_node(node: &impl Node, area: Rect, buf: &mut Buffer, path: &[usize], mut count: usize) {
    const BG_COLORS: [Color; 4] = [Color::Red, Color::Yellow, Color::Magenta, Color::LightRed];
    const FG_COLOR: Color = Color::Black;

    if node.size() == 0 {
        return;
    }

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
            (area.width as u128 * (child.size() as u128) / (node.size() as u128)) as u16;
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

            // TODO: maybe move outside of this if-closure?
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

pub fn find_node<'a, T: Node>(root: &'a T, path: &[usize]) -> Option<&'a T> {
    let mut curr = root;
    for &index in path {
        curr = &curr.children().get(index)?.1;
    }
    Some(curr)
}

/// Root node has no metadata
pub fn find_node_with_meta<'a, T: Node>(
    root: &'a T,
    path: &[usize],
) -> Option<&'a (T::Metadata, T)> {
    let (&first, path) = path.split_first()?;
    let mut curr = root.children().get(first)?;

    for &index in path {
        curr = curr.1.children().get(index)?;
    }

    Some(curr)
}

// fn correct_path<T: Node>(root: &T, path: &mut Vec<usize>) {
//     let mut curr = root;
//     for (i, segment) in path.iter_mut().enumerate() {
//         if let Some(max_index) = curr.children().len().checked_sub(1) {
//             *segment = cmp::min(*segment, max_index);
//             let x = &curr.children()[*segment];
//             curr = &x.1;
//         } else {
//             // children are empty
//             path.truncate(i);
//             break;
//         }
//     }
// }

fn find_path_by_pos<T: Node>(root: &T, area: Rect, pos: Position) -> Option<Vec<usize>> {
    fn inner<T: Node>(
        node: &T,
        area: Rect,
        pos: Position,
        path: &mut Vec<usize>,
        height: usize,
    ) -> bool {
        if node.size() == 0 {
            return false;
        }

        let node_y = area.bottom() - NODE_HEIGHT; // TODO: make one check
        // {
        //     let mut node_rect = area;
        //     node_rect.y = node_y;
        //     node_rect.height = 1;
        //     dbg!(node_rect);
        // }

        if pos.y == node_y && pos.x >= area.x && pos.x < area.right() {
            return true;
        }

        let mut child_x = area.x;

        for (i, (_, child)) in node.children().iter().enumerate() {
            if let Some(child_area_hight) = area.height.checked_sub(1) {
                let child_width =
                    (area.width as u128 * (child.size() as u128) / (node.size() as u128)) as u16;

                let child_area = Rect::new(child_x, area.y, child_width, child_area_hight);

                path.truncate(height);
                path.push(i);

                if inner(child, child_area, pos, path, height + 1) {
                    return true;
                }

                child_x += child_width;
            }
        }

        false
    }

    let mut path = vec![];
    if inner(root, area, pos, &mut path, 0) {
        Some(path)
    } else {
        None
    }
}

fn update_path<T: Node>(root: &T, path: &mut Vec<usize>, dirs: &mut Vec<Direction>) {
    // TODO:
    // [ ] skip nodes which length is zero
    // [ ] jump to sibling

    // we need to search node again each time because we have no means of getting a sibling or parent

    for dir in dirs.drain(..) {
        match dir {
            Direction::Left => {
                if let Some(index) = path.last_mut()
                    && let Some(prev) = index.checked_sub(1)
                {
                    *index = prev;
                }
            }
            Direction::Right => {
                if let Some((index, parent_path)) = path.split_last_mut()
                    && let Some(parent) = find_node(root, parent_path)
                {
                    *index = cmp::min(*index + 1, parent.children().len() - 1);
                }
            }
            Direction::Up => {
                if let Some(curr) = find_node(root, path)
                    && !curr.children().is_empty()
                {
                    path.push(0);
                }
            }
            Direction::Down => {
                path.pop();
            }
        }
    }
}

#[cfg(test)]
mod mock_node {
    use std::borrow::Cow;

    use crate::flame_graph::{Node, NodeMetadata};

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
}

impl<'a> Node for livloc_proto::Tree<'a> {
    type Metadata = livloc_proto::Frame<'a>;

    fn size(&self) -> usize {
        self.size
    }

    fn children(&self) -> &[(Self::Metadata, Self)] {
        &self.children
    }
}

impl NodeMetadata for livloc_proto::Frame<'_> {
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

// #[cfg(test)]
// mod test_correct_path {
//     use crate::flame_graph::{correct_path, mock_node::MockNode};

//     #[test]
//     fn test_1_node() {
//         let node = MockNode {
//             size: 0,
//             children: vec![],
//         };
//         let mut path = vec![0];
//         correct_path(&node, &mut path);
//         assert_eq!(path, &[]);
//     }

//     #[test]
//     fn test_2_node_extra_segment() {
//         let node = MockNode {
//             size: 0,
//             children: vec![(
//                 Default::default(),
//                 MockNode {
//                     size: 0,
//                     children: vec![],
//                 },
//             )],
//         };
//         let mut path = vec![0, 1];
//         correct_path(&node, &mut path);
//         assert_eq!(path, &[0]);
//     }

//     #[test]
//     fn test_2_nodes_index_greater() {
//         let node = MockNode {
//             size: 0,
//             children: vec![(
//                 Default::default(),
//                 MockNode {
//                     size: 0,
//                     children: vec![],
//                 },
//             )],
//         };
//         let mut path = vec![1];
//         correct_path(&node, &mut path);
//         assert_eq!(path, &[0]);
//     }

//     #[test]
//     fn test_3_nodes_2nd_index_greater() {
//         let node = MockNode {
//             size: 0,
//             children: vec![(
//                 Default::default(),
//                 MockNode {
//                     size: 0,
//                     children: vec![(
//                         Default::default(),
//                         MockNode {
//                             size: 0,
//                             children: vec![],
//                         },
//                     )],
//                 },
//             )],
//         };
//         let mut path = vec![0, 1];
//         correct_path(&node, &mut path);
//         assert_eq!(path, &[0, 0]);
//     }
// }

#[cfg(test)]
mod test_find_path_by_pos {
    use ratatui::layout::{Position, Rect};

    use crate::flame_graph::{
        find_path_by_pos,
        mock_node::{MockNode, MockNodeMetadata},
    };

    #[test]
    fn test_root_node() {
        // [  5  ][ 3 ]
        // [    8     ]

        let node = MockNode::new(
            8,
            vec![
                (MockNodeMetadata::new(""), MockNode::new(5, vec![])),
                (MockNodeMetadata::new(""), MockNode::new(3, vec![])),
            ],
        );
        let area = Rect::new(0, 0, 8, 2);
        let pos = Position::new(2, 1);
        let path = find_path_by_pos(&node, area, pos).unwrap();
        assert_eq!(path, &[]);
    }

    #[test]
    fn test_first_node() {
        // [  5  ][ 3 ]
        // [    8     ]

        let node = MockNode::new(
            8,
            vec![
                (MockNodeMetadata::new(""), MockNode::new(5, vec![])),
                (MockNodeMetadata::new(""), MockNode::new(3, vec![])),
            ],
        );
        let area = Rect::new(0, 0, 8, 2);
        let pos = Position::new(2, 0);
        let path = find_path_by_pos(&node, area, pos).unwrap();
        assert_eq!(path, &[0]);
    }

    #[test]
    fn test_in_the_middle() {
        //        [ 3 ] x [ 2]
        // [  5  ][  6   ][ 4  ]
        // [        15         ]

        let node = MockNode::new(
            15,
            vec![
                (MockNodeMetadata::new(""), MockNode::new(5, vec![])),
                (
                    MockNodeMetadata::new(""),
                    MockNode::new(
                        6,
                        vec![(MockNodeMetadata::new(""), MockNode::new(3, vec![]))],
                    ),
                ),
                (
                    MockNodeMetadata::new(""),
                    MockNode::new(
                        4,
                        vec![(MockNodeMetadata::new(""), MockNode::new(2, vec![]))],
                    ),
                ),
            ],
        );
        let area = Rect::new(0, 0, 15, 3);
        let pos = Position::new(9, 0);
        let result = find_path_by_pos(&node, area, pos);
        assert_eq!(result, None);
    }

    #[test]
    fn test_middle_node() {
        //        [ 3 ]   [ 2]
        // [  5  ][  6  x][ 4  ]
        // [        15         ]

        let node = MockNode::new(
            15,
            vec![
                (MockNodeMetadata::new(""), MockNode::new(5, vec![])),
                (
                    MockNodeMetadata::new(""),
                    MockNode::new(
                        6,
                        vec![(MockNodeMetadata::new(""), MockNode::new(3, vec![]))],
                    ),
                ),
                (
                    MockNodeMetadata::new(""),
                    MockNode::new(
                        4,
                        vec![(MockNodeMetadata::new(""), MockNode::new(2, vec![]))],
                    ),
                ),
            ],
        );
        let area = Rect::new(0, 0, 15, 3);
        let pos = Position::new(8, 1);
        let path = find_path_by_pos(&node, area, pos).unwrap();
        assert_eq!(path, &[1]);
    }

    #[test]
    fn test_middle_2nd_leaf() {
        //        [ 3][3x][ 2]
        // [  5  ][  6   ][ 4  ]
        // [        15         ]

        let node = MockNode::new(
            15,
            vec![
                (MockNodeMetadata::new(""), MockNode::new(5, vec![])),
                (
                    MockNodeMetadata::new(""),
                    MockNode::new(
                        6,
                        vec![
                            (MockNodeMetadata::new(""), MockNode::new(3, vec![])),
                            (MockNodeMetadata::new(""), MockNode::new(3, vec![])),
                        ],
                    ),
                ),
                (
                    MockNodeMetadata::new(""),
                    MockNode::new(
                        4,
                        vec![(MockNodeMetadata::new(""), MockNode::new(2, vec![]))],
                    ),
                ),
            ],
        );
        let area = Rect::new(0, 0, 15, 3);
        let pos = Position::new(8, 0);
        let path = find_path_by_pos(&node, area, pos).unwrap();
        assert_eq!(path, &[1, 1]);
    }

    #[test]
    fn test_middle_node_bigger_frame() {
        // ..................................
        // ..................................
        // ..       [ 3 ]   [ 2]  ...........
        // ..[  5  ][  6  x][ 4  ]...........
        // ..[        15         ]...........

        let node = MockNode::new(
            15,
            vec![
                (MockNodeMetadata::new(""), MockNode::new(5, vec![])),
                (
                    MockNodeMetadata::new(""),
                    MockNode::new(
                        6,
                        vec![(MockNodeMetadata::new(""), MockNode::new(3, vec![]))],
                    ),
                ),
                (
                    MockNodeMetadata::new(""),
                    MockNode::new(
                        4,
                        vec![(MockNodeMetadata::new(""), MockNode::new(2, vec![]))],
                    ),
                ),
            ],
        );
        let area = Rect::new(2, 2, 15, 3);
        let pos = Position::new(10, 3);
        let path = find_path_by_pos(&node, area, pos).unwrap();
        assert_eq!(path, &[1]);
    }
}

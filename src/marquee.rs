// SPDX-License-Identifier: GPL-3.0-only

//! The panel title strip: text that scrolls sideways by whole and fractional
//! pixels, rather than jumping a character at a time.
//!
//! Every ordinary container clamps a child to the width available, so a long
//! title can't be laid out at its natural width and then slid along. This
//! widget lays its pieces out unbounded — the title, the gap, and the title
//! again — and draws them shifted left by the scroll offset inside a clipping
//! layer the size of the strip. Because it measures the real text, it also
//! knows exactly whether the title fits: a title that fits is drawn once, still,
//! whatever offset it's given.

use cosmic::iced::advanced::layout::{self, Layout};
use cosmic::iced::advanced::renderer;
use cosmic::iced::advanced::widget::{Tree, Widget};
use cosmic::iced::advanced::Renderer as _;
use cosmic::iced::{mouse, Length, Rectangle, Size, Vector};
use cosmic::Element;

/// A clipped, horizontally scrolling title. See the module docs.
pub struct Marquee<'a, Message> {
    /// Title, gap, title.
    children: Vec<Element<'a, Message>>,
    /// Pixels scrolled so far. Wrapped by the title-plus-gap width when drawn,
    /// so it can grow without limit.
    offset: f32,
}

/// A marquee showing `text`, followed by `gap` before it repeats, scrolled
/// `offset` pixels to the left.
pub fn marquee<'a, Message: 'a>(text: &str, gap: &str, offset: f32) -> Marquee<'a, Message> {
    let piece = |s: &str| -> Element<'a, Message> {
        cosmic::widget::text::body(s.to_string())
            .wrapping(cosmic::iced::widget::text::Wrapping::None)
            .into()
    };
    Marquee { children: vec![piece(text), piece(gap), piece(text)], offset }
}

/// Whether text `text_width` pixels wide needs to scroll in a strip
/// `available` pixels wide. Half a pixel of slack absorbs rounding.
pub fn overflows(text_width: f32, available: f32) -> bool {
    text_width > available + 0.5
}

/// Where the strip starts drawing for a given offset: the offset wrapped into
/// one title-plus-gap cycle, so the second copy of the title slides exactly
/// into the first one's place.
pub fn wrapped_offset(offset: f32, cycle: f32) -> f32 {
    if cycle > 0.0 { offset.rem_euclid(cycle) } else { 0.0 }
}

impl<Message> Widget<Message, cosmic::Theme, cosmic::Renderer> for Marquee<'_, Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut self.children);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &cosmic::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        // Unbounded width, so each piece takes its natural width.
        let piece_limits =
            layout::Limits::new(Size::ZERO, Size::new(f32::INFINITY, limits.max().height));
        let mut x = 0.0;
        let mut height: f32 = 0.0;
        let nodes: Vec<layout::Node> = self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .map(|(child, state)| {
                let node = child.as_widget_mut().layout(state, renderer, &piece_limits).move_to((x, 0.0));
                x += node.size().width;
                height = height.max(node.size().height);
                node
            })
            .collect();
        let size = limits.resolve(Length::Fill, Length::Shrink, Size::new(x, height));
        layout::Node::with_children(size, nodes)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut cosmic::Renderer,
        theme: &cosmic::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let Some(visible) = bounds.intersection(viewport) else {
            return;
        };
        let pieces: Vec<Layout<'_>> = layout.children().collect();
        let [title, gap, _] = pieces.as_slice() else {
            return;
        };
        let title_width = title.bounds().width;
        let cycle = title_width + gap.bounds().width;

        renderer.with_layer(visible, |renderer| {
            if !overflows(title_width, bounds.width) {
                // Fits: the title once, still.
                self.children[0].as_widget().draw(
                    &tree.children[0], renderer, theme, style, *title, cursor, &visible,
                );
                return;
            }
            let shift = wrapped_offset(self.offset, cycle);
            // In the translated frame the strip sits `shift` further right.
            let shifted_view = Rectangle { x: visible.x + shift, ..visible };
            renderer.with_translation(Vector::new(-shift, 0.0), |renderer| {
                for ((child, state), piece) in self.children.iter().zip(&tree.children).zip(&pieces) {
                    child.as_widget().draw(state, renderer, theme, style, *piece, cursor, &shifted_view);
                }
            });
        });
    }
}

impl<'a, Message: 'a> From<Marquee<'a, Message>> for Element<'a, Message> {
    fn from(marquee: Marquee<'a, Message>) -> Self {
        Element::new(marquee)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_wraps_by_one_cycle() {
        assert_eq!(wrapped_offset(0.0, 100.0), 0.0);
        assert_eq!(wrapped_offset(150.0, 100.0), 50.0);
        assert_eq!(wrapped_offset(1_000_000.25, 100.0), 0.25);
        // A degenerate cycle never scrolls.
        assert_eq!(wrapped_offset(42.0, 0.0), 0.0);
    }

    #[test]
    fn only_text_wider_than_the_strip_overflows() {
        assert!(!overflows(120.0, 150.0));
        assert!(!overflows(150.3, 150.0));
        assert!(overflows(151.0, 150.0));
    }
}

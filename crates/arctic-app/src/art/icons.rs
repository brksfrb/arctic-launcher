//! Crisp vector icons drawn with strokes, so they match at every DPI and
//! don't depend on which glyphs the emoji font happens to have.

use std::f32::consts::TAU;

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2, vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Play,
    User,
    /// Two people (play together).
    Friends,
    Layers,
    Gear,
    Info,
    Folder,
    Document,
    Plus,
    Trash,
    Check,
    ChevronDown,
    Close,
    Copy,
    External,
    Stop,
}

/// Draw `icon` centered in `rect` (square-ish, ~16-24 px works best).
pub fn draw(painter: &Painter, icon: Icon, rect: Rect, color: Color32) {
    let size = rect.width().min(rect.height());
    let c = rect.center();
    let s = size / 2.0;
    let stroke = Stroke::new((size * 0.09).max(1.4), color);
    let line = |a: Vec2, b: Vec2| painter.line_segment([c + a * s, c + b * s], stroke);
    let poly = |pts: &[Vec2], closed: bool| {
        let points: Vec<Pos2> = pts.iter().map(|p| c + *p * s).collect();
        if closed {
            painter.add(Shape::closed_line(points, stroke));
        } else {
            painter.add(Shape::line(points, stroke));
        }
    };
    match icon {
        Icon::Play => {
            let pts = [vec2(-0.45, -0.7), vec2(0.7, 0.0), vec2(-0.45, 0.7)];
            painter.add(Shape::convex_polygon(
                pts.iter().map(|p| c + *p * s).collect(),
                color,
                Stroke::NONE,
            ));
        }
        Icon::User => {
            painter.circle_stroke(c + vec2(0.0, -0.38) * s, 0.3 * s, stroke);
            // Shoulders: upper half of an ellipse, closed along the bottom.
            let shoulders: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let a = std::f32::consts::PI * (1.0 + i as f32 / 16.0);
                    c + vec2(a.cos() * 0.7, 0.8 + a.sin() * 0.62) * s
                })
                .collect();
            painter.add(Shape::closed_line(shoulders, stroke));
        }
        Icon::Friends => {
            let person = |dx: f32, scale: f32| {
                let head = c + vec2(dx, -0.3 * scale) * s;
                painter.circle_stroke(head, 0.24 * scale * s, stroke);
                let body: Vec<Pos2> = (0..=12)
                    .map(|i| {
                        let a = std::f32::consts::PI * (1.0 + i as f32 / 12.0);
                        c + vec2(dx + a.cos() * 0.5 * scale, 0.75 + a.sin() * 0.5 * scale) * s
                    })
                    .collect();
                painter.add(Shape::line(body, stroke));
            };
            person(-0.36, 0.9);
            person(0.4, 0.8);
        }
        Icon::Layers => {
            for dy in [-0.45, 0.0, 0.45] {
                let d = vec2(0.0, dy);
                poly(
                    &[
                        d + vec2(-0.75, 0.0),
                        d + vec2(0.0, -0.32),
                        d + vec2(0.75, 0.0),
                        d + vec2(0.0, 0.32),
                    ],
                    true,
                );
            }
        }
        Icon::Gear => {
            painter.circle_stroke(c, 0.42 * s, stroke);
            painter.circle_stroke(c, 0.16 * s, stroke);
            for k in 0..8 {
                let d = Vec2::angled(k as f32 * TAU / 8.0);
                line(d * 0.5, d * 0.78);
            }
        }
        Icon::Info => {
            painter.circle_stroke(c, 0.78 * s, stroke);
            line(vec2(0.0, -0.1), vec2(0.0, 0.42));
            painter.circle_filled(c + vec2(0.0, -0.38) * s, stroke.width * 0.8, color);
        }
        Icon::Folder => poly(
            &[
                vec2(-0.8, -0.55),
                vec2(-0.25, -0.55),
                vec2(-0.1, -0.35),
                vec2(0.8, -0.35),
                vec2(0.8, 0.6),
                vec2(-0.8, 0.6),
            ],
            true,
        ),
        Icon::Document => {
            poly(
                &[
                    vec2(-0.55, -0.8),
                    vec2(0.25, -0.8),
                    vec2(0.6, -0.45),
                    vec2(0.6, 0.8),
                    vec2(-0.55, 0.8),
                ],
                true,
            );
            for y in [-0.15, 0.15, 0.45] {
                line(vec2(-0.3, y), vec2(0.35, y));
            }
        }
        Icon::Plus => {
            line(vec2(-0.7, 0.0), vec2(0.7, 0.0));
            line(vec2(0.0, -0.7), vec2(0.0, 0.7));
        }
        Icon::Trash => {
            line(vec2(-0.75, -0.55), vec2(0.75, -0.55));
            poly(
                &[
                    vec2(-0.25, -0.55),
                    vec2(-0.2, -0.78),
                    vec2(0.2, -0.78),
                    vec2(0.25, -0.55),
                ],
                false,
            );
            poly(
                &[
                    vec2(-0.55, -0.55),
                    vec2(-0.45, 0.8),
                    vec2(0.45, 0.8),
                    vec2(0.55, -0.55),
                ],
                false,
            );
        }
        Icon::Check => poly(
            &[vec2(-0.65, 0.0), vec2(-0.2, 0.48), vec2(0.7, -0.5)],
            false,
        ),
        Icon::ChevronDown => poly(
            &[vec2(-0.55, -0.2), vec2(0.0, 0.32), vec2(0.55, -0.2)],
            false,
        ),
        Icon::Close => {
            line(vec2(-0.55, -0.55), vec2(0.55, 0.55));
            line(vec2(-0.55, 0.55), vec2(0.55, -0.55));
        }
        Icon::Copy => {
            poly(
                &[
                    vec2(-0.3, -0.3),
                    vec2(0.75, -0.3),
                    vec2(0.75, 0.75),
                    vec2(-0.3, 0.75),
                ],
                true,
            );
            poly(
                &[vec2(-0.75, 0.3), vec2(-0.75, -0.75), vec2(0.3, -0.75)],
                false,
            );
        }
        Icon::External => {
            poly(&[vec2(0.1, -0.7), vec2(0.7, -0.7), vec2(0.7, -0.1)], false);
            line(vec2(0.7, -0.7), vec2(-0.05, 0.05));
            poly(
                &[
                    vec2(0.35, 0.2),
                    vec2(0.35, 0.7),
                    vec2(-0.7, 0.7),
                    vec2(-0.7, -0.35),
                    vec2(-0.2, -0.35),
                ],
                false,
            );
        }
        Icon::Stop => {
            let r = Rect::from_center_size(c, vec2(1.1, 1.1) * s);
            painter.rect_filled(r, 2.0, color);
        }
    }
}

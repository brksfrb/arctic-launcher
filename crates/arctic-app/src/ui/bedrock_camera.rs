//! Local camera-path editing views. These never send camera or replay commands.
use arctic_core::bedrock::offline::{CameraCurve, CameraPath, CameraPose};
use eframe::egui::{self, Sense, Stroke, vec2};

use crate::t;
use crate::theme::Palette;
/// Same ordered steps as Java ReplayViewer, available before connecting and
/// while playing. Non-preset speeds step to the next preset in that direction.
pub(super) fn speed(ui: &mut egui::Ui, value: &mut f64) -> bool {
    use arctic_core::bedrock::offline::{MAX_REPLAY_SPEED, MIN_REPLAY_SPEED, REPLAY_SPEEDS};
    ui.label(t!("bedrock.speed"));
    let mut changed = ui
        .add(
            egui::DragValue::new(value)
                .range(MIN_REPLAY_SPEED..=MAX_REPLAY_SPEED)
                .speed(0.1)
                .suffix("×"),
        )
        .changed();
    let slower = REPLAY_SPEEDS
        .iter()
        .rev()
        .copied()
        .find(|candidate| *candidate < *value);
    let faster = REPLAY_SPEEDS
        .iter()
        .copied()
        .find(|candidate| *candidate > *value);
    if ui
        .add_enabled(slower.is_some(), egui::Button::new(t!("bedrock.slower")))
        .clicked()
        && let Some(next) = slower
    {
        *value = next;
        changed = true;
    }
    if ui
        .add_enabled(faster.is_some(), egui::Button::new(t!("bedrock.faster")))
        .clicked()
        && let Some(next) = faster
    {
        *value = next;
        changed = true;
    }
    changed
}

pub(super) struct Pick {
    pub time: u32,
    pub pose: CameraPose,
}

pub(super) fn show(
    ui: &mut egui::Ui,
    p: &Palette,
    path: &CameraPath,
    playhead: u32,
    duration: u32,
    edit_time: u32,
) -> Option<Pick> {
    if path.keyframes.is_empty() {
        return None;
    }
    let curve = match path.curve() {
        Ok(curve) => curve,
        Err(error) => {
            ui.colored_label(p.error, error.to_string());
            return None;
        }
    };
    ui.add_space(6.0);
    ui.label(t!("bedrock.camera_timeline"));
    let end = duration.max(path.keyframes.last().unwrap().time_ms).max(1);
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width().max(40.0), 40.0),
        Sense::click_and_drag(),
    );
    let rect = rect.shrink2(vec2(10.0, 7.0));
    let painter = ui.painter_at(rect);
    let y = rect.center().y;
    let x_at =
        |time: u32| rect.left() + rect.width() * (time as f64 / end as f64).clamp(0.0, 1.0) as f32;
    painter.line_segment(
        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
        Stroke::new(3.0, p.card_stroke),
    );
    let mut pick = None;
    if (response.clicked() || response.dragged())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let nearest = path.keyframes.iter().min_by(|a, b| {
            (x_at(a.time_ms) - pointer.x)
                .abs()
                .total_cmp(&(x_at(b.time_ms) - pointer.x).abs())
        });
        if let Some(key) = nearest.filter(|key| (x_at(key.time_ms) - pointer.x).abs() <= 6.0) {
            // Selecting a key edits its original values; sampling its curve
            // must not silently round the saved FOV or replace its yaw turn.
            pick = Some(Pick {
                time: key.time_ms,
                pose: key.pose,
            });
        } else {
            let time = (((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64
                * end as f64)
                .round() as u32;
            match curve.sample(time as f64) {
                Ok(pose) => pick = Some(Pick { time, pose }),
                Err(error) => {
                    ui.colored_label(p.error, error.to_string());
                }
            }
        }
    }
    for key in &path.keyframes {
        let x = x_at(key.time_ms);
        let selected = key.time_ms == edit_time;
        let color = if selected { p.text } else { p.accent };
        painter.line_segment(
            [egui::pos2(x, y - 6.0), egui::pos2(x, y + 6.0)],
            Stroke::new(if selected { 3.0 } else { 2.0 }, color),
        );
    }
    for (time, color) in [(playhead, p.warn), (edit_time, p.accent)] {
        let x = x_at(time);
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            Stroke::new(1.0, color),
        );
    }
    response.on_hover_text(t!("bedrock.click_or_drag_to_sample_the_path_blue_marks"));
    ui.horizontal_wrapped(|ui| {
        if ui.button(t!("bedrock.sample_path_at_edit_time")).clicked() {
            match curve.sample(edit_time as f64) {
                Ok(pose) => {
                    pick = Some(Pick {
                        time: edit_time,
                        pose,
                    })
                }
                Err(error) => {
                    ui.colored_label(p.error, error.to_string());
                }
            }
        }
        if ui
            .add_enabled(
                path.keyframes.iter().any(|key| key.time_ms < edit_time),
                egui::Button::new(t!("bedrock.previous_keyframe")),
            )
            .clicked()
            && let Some(key) = path
                .keyframes
                .iter()
                .rev()
                .find(|key| key.time_ms < edit_time)
        {
            pick = Some(Pick {
                time: key.time_ms,
                pose: key.pose,
            });
        }
        if ui
            .add_enabled(
                path.keyframes.iter().any(|key| key.time_ms > edit_time),
                egui::Button::new(t!("bedrock.next_keyframe")),
            )
            .clicked()
            && let Some(key) = path.keyframes.iter().find(|key| key.time_ms > edit_time)
        {
            pick = Some(Pick {
                time: key.time_ms,
                pose: key.pose,
            });
        }
    });
    egui::CollapsingHeader::new(t!("bedrock.camera_path_overview_x_z"))
        .default_open(true)
        .show(ui, |ui| {
            if let Some(selected) = overview(ui, p, path, &curve, edit_time) {
                pick = Some(selected);
            }
            ui.label(
                egui::RichText::new(t!("bedrock.click_a_point_to_select_its_keyframe_preview"))
                    .small()
                    .color(p.muted),
            );
        });
    pick
}

fn overview(
    ui: &mut egui::Ui,
    p: &Palette,
    path: &CameraPath,
    curve: &CameraCurve,
    edit_time: u32,
) -> Option<Pick> {
    // Include each key even when timestamps are uneven. Bound the additional
    // curve samples; hold jumps and rejected overshoots never become fake lines.
    let subdivisions = (1024 / path.keyframes.len().saturating_sub(1).max(1)).clamp(1, 24);
    let mut points: Vec<CameraPose> = path.keyframes.iter().map(|key| key.pose).collect();
    let mut lines = Vec::new();
    let mut rejected = false;
    for pair in path.keyframes.windows(2) {
        if pair[0].ease == "hold" {
            continue;
        }
        let mut previous = None;
        for index in 0..=subdivisions {
            let time = pair[0].time_ms as f64
                + (pair[1].time_ms - pair[0].time_ms) as f64 * index as f64 / subdivisions as f64;
            match curve.sample(time) {
                Ok(pose) => {
                    if let Some(before) = previous {
                        lines.push((before, pose));
                    }
                    points.push(pose);
                    previous = Some(pose);
                }
                Err(_) => {
                    rejected = true;
                    previous = None;
                }
            }
        }
    }
    let selected = curve.sample(edit_time as f64).ok();
    if let Some(pose) = selected {
        points.push(pose);
    }
    let min_x = points
        .iter()
        .map(|pose| pose.x)
        .fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|pose| pose.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_z = points
        .iter()
        .map(|pose| pose.z)
        .fold(f64::INFINITY, f64::min);
    let max_z = points
        .iter()
        .map(|pose| pose.z)
        .fold(f64::NEG_INFINITY, f64::max);
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width().max(40.0), 160.0), Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, egui::CornerRadius::same(5), p.surface);
    let scale = ((rect.width() - 24.0) as f64 / (max_x - min_x).max(1.0))
        .min((rect.height() - 24.0) as f64 / (max_z - min_z).max(1.0));
    let project = |pose: CameraPose| {
        egui::pos2(
            rect.center().x + ((pose.x - (min_x + max_x) * 0.5) * scale) as f32,
            rect.center().y - ((pose.z - (min_z + max_z) * 0.5) * scale) as f32,
        )
    };
    for (a, b) in lines {
        painter.line_segment([project(a), project(b)], Stroke::new(1.5, p.accent));
    }
    for key in &path.keyframes {
        painter.circle_filled(
            project(key.pose),
            if key.time_ms == edit_time { 4.0 } else { 2.5 },
            if key.time_ms == edit_time {
                p.text
            } else {
                p.accent
            },
        );
    }
    if let Some(pose) = selected {
        let center = project(pose);
        painter.circle_filled(center, 4.0, p.warn);
        let angle = pose.yaw.to_radians();
        let direction = vec2(-angle.sin() as f32, -angle.cos() as f32) * 12.0;
        painter.line_segment([center, center + direction], Stroke::new(2.0, p.warn));
    }
    let pick = if response.clicked() {
        response.interact_pointer_pos().and_then(|pointer| {
            path.keyframes
                .iter()
                .min_by(|a, b| {
                    project(a.pose)
                        .distance_sq(pointer)
                        .total_cmp(&project(b.pose).distance_sq(pointer))
                })
                .filter(|key| project(key.pose).distance_sq(pointer) <= 100.0)
                .map(|key| Pick {
                    time: key.time_ms,
                    pose: key.pose,
                })
        })
    } else {
        None
    };
    response.on_hover_text(t!("bedrock.top_view_x_runs_right_and_z_runs_up_amber"));
    if rejected {
        ui.colored_label(
            p.error,
            t!("bedrock.some_curve_samples_leave_the_supported"),
        );
    }
    pick
}

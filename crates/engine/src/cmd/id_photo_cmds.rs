//! ID Photo 3×4: one command that removes the background of the active pixel layer (Remove
//! Background), frames the subject head and shoulders at 3 × 4 cm and resamples to 300 dpi.
//! It is Remove Background followed by Crop, each a step the user could also take by hand.

use photocraft_geom::Rect;
use serde_json::{Value, json};

use crate::commands::CommandSpec;
use crate::{EngineError, Result, Session};

const CMD: &str = "image.idPhoto";

/// 3 × 4 cm at 300 dpi: 3 / 2.54 × 300 and 4 / 2.54 × 300, rounded.
const DPI: f32 = 300.0;
const TARGET_W: u32 = 354;
const TARGET_H: u32 = 472;

/// The subject fills this much of the frame's height (head and shoulders)...
const SUBJECT_HEIGHT: f64 = 0.7;
/// ...or this much of its width, when the subject is wider than that.
const SUBJECT_WIDTH: f64 = 0.9;
/// Share of the spare height left above the subject; the rest goes below the shoulders.
const HEAD_ROOM: f64 = 0.3;

/// The 3:4 frame around `subject`, inside `canvas`. The subject is centred horizontally and
/// kept whole; the frame shrinks to fit the canvas and keeps its shape. `None` for an empty
/// subject or canvas.
pub(crate) fn frame(subject: Rect, canvas: Rect) -> Option<Rect> {
    if subject.is_empty() || canvas.is_empty() {
        return None;
    }
    let (sw, sh) = (f64::from(subject.width()), f64::from(subject.height()));
    let mut ch = (sh / SUBJECT_HEIGHT).max(sw / SUBJECT_WIDTH * 4.0 / 3.0);
    let mut cw = ch * 3.0 / 4.0;
    let fit = (f64::from(canvas.width()) / cw).min(f64::from(canvas.height()) / ch).min(1.0);
    ch *= fit;
    cw *= fit;
    let cx = f64::from(subject.x0) + sw / 2.0;
    let top = f64::from(subject.y0) - (ch - sh) * HEAD_ROOM;
    let x = (cx - cw / 2.0).clamp(f64::from(canvas.x0), f64::from(canvas.x1) - cw);
    let y = top.clamp(f64::from(canvas.y0), f64::from(canvas.y1) - ch);
    let (x0, y0) = (x.round() as i32, y.round() as i32);
    let w = (cw.round() as i32).max(1);
    let h = (ch.round() as i32).max(1);
    Some(Rect::new(x0, y0, x0 + w, y0 + h))
}

fn enabled(s: &Session) -> std::result::Result<(), String> {
    s.active().map(|_| ()).ok_or_else(|| "no document open".into())
}

fn run(s: &mut Session, _p: &Value) -> Result<Value> {
    let canvas = s.active().ok_or(EngineError::NoDocument)?.doc.bounds();
    let cut = s.execute("layer.removeBackground", json!({}))?;
    let num = |i: usize| cut["bounds"].get(i).and_then(Value::as_i64).and_then(|v| i32::try_from(v).ok());
    let subject = match (num(0), num(1), num(2), num(3)) {
        (Some(x), Some(y), Some(w), Some(h)) if w > 0 && h > 0 => Rect::new(x, y, x + w, y + h),
        _ => return Err(EngineError::Other("no subject found".into())),
    };
    let r = frame(subject, canvas).ok_or_else(|| EngineError::Other("nothing to frame".into()))?;
    s.execute(
        "image.crop",
        json!({
            "x": r.x0,
            "y": r.y0,
            "width": r.width(),
            "height": r.height(),
            "targetWidth": TARGET_W,
            "targetHeight": TARGET_H,
            "resolution": DPI,
        }),
    )?;
    Ok(json!({ "width": TARGET_W, "height": TARGET_H, "resolution": DPI }))
}

/// ID Photo 3×4 command spec. Its menu item lives in `crates/ui-egui/src/menu_catalog.rs`.
pub fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec {
        id: CMD,
        label: "ID Photo 3×4",
        menu: &[],
        shortcut: None,
        params: r##"{} → {width,height,resolution} (Remove Background, then crops to 3 × 4 cm at 300 dpi, 354 × 472 px)"##,
        enabled,
        run,
        journal: true,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas() -> Rect {
        Rect::new(0, 0, 400, 300)
    }

    /// The frame keeps the subject whole, stays on the canvas and has a 3:4 shape.
    #[test]
    fn frame_keeps_subject_inside_a_three_by_four_frame() {
        let subject = Rect::new(120, 60, 260, 240);
        let r = frame(subject, canvas()).unwrap();
        assert!(r.contains_rect(&subject));
        assert!(canvas().contains_rect(&r));
        let (w, h) = (i64::from(r.width()), i64::from(r.height()));
        assert!((w * 4 - h * 3).abs() <= 4, "frame {w}x{h} is not 3:4");
    }

    /// A subject larger than the canvas still gets a 3:4 frame that fits inside the canvas.
    #[test]
    fn frame_shrinks_to_the_canvas_for_a_large_subject() {
        let subject = Rect::new(0, 0, 400, 300);
        let r = frame(subject, canvas()).unwrap();
        assert!(canvas().contains_rect(&r));
        assert!(r.height() <= 300 && r.width() <= 400);
    }

    #[test]
    fn frame_is_none_for_empty_input() {
        assert!(frame(Rect::EMPTY, canvas()).is_none());
        assert!(frame(Rect::new(0, 0, 10, 10), Rect::EMPTY).is_none());
    }
}

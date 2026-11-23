//! Photo sizes: one command per size that removes the background of the active pixel layer (Remove
//! Background), frames the subject at that size's aspect ratio and resamples to 300 dpi.
//! Each command is Remove Background followed by Crop, the two steps the user could take by hand.

use photocraft_geom::Rect;
use serde_json::{Value, json};

use crate::commands::CommandSpec;
use crate::{EngineError, Result, Session};

/// Pixels per inch for every size.
const DPI: f64 = 300.0;

/// The subject fills this much of the frame's height (head and shoulders)...
const SUBJECT_HEIGHT: f64 = 0.7;
/// ...or this much of its width, when the subject is wider than that.
const SUBJECT_WIDTH: f64 = 0.9;
/// Share of the spare height left above the subject; the rest goes below the shoulders.
const HEAD_ROOM: f64 = 0.3;

/// A photo size in centimetres, portrait: `width_cm` × `height_cm`.
#[derive(Clone, Copy)]
struct Size {
    width_cm: f64,
    height_cm: f64,
}

impl Size {
    /// Pixels along one side at 300 dpi.
    fn px(cm: f64) -> u32 {
        (cm / 2.54 * DPI).round() as u32
    }

    fn target_w(self) -> u32 {
        Self::px(self.width_cm)
    }

    fn target_h(self) -> u32 {
        Self::px(self.height_cm)
    }
}

const ID_3X4: Size = Size { width_cm: 3.0, height_cm: 4.0 };
const ID_3_5X4_5: Size = Size { width_cm: 3.5, height_cm: 4.5 };
const PHOTO_4X6: Size = Size { width_cm: 4.0, height_cm: 6.0 };
const PHOTO_6X9: Size = Size { width_cm: 6.0, height_cm: 9.0 };
const PHOTO_9X12: Size = Size { width_cm: 9.0, height_cm: 12.0 };
const PHOTO_10X15: Size = Size { width_cm: 10.0, height_cm: 15.0 };

/// The frame of `size` around `subject`, inside `canvas`. The subject is centred horizontally and
/// kept whole; the frame shrinks to fit the canvas and keeps its shape. `None` for an empty
/// subject or canvas.
fn frame(subject: Rect, canvas: Rect, size: Size) -> Option<Rect> {
    if subject.is_empty() || canvas.is_empty() {
        return None;
    }
    let aspect = size.width_cm / size.height_cm;
    let (sw, sh) = (f64::from(subject.width()), f64::from(subject.height()));
    let mut ch = (sh / SUBJECT_HEIGHT).max(sw / SUBJECT_WIDTH / aspect);
    let mut cw = ch * aspect;
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

/// Remove Background, then crop the subject's frame to `size` at 300 dpi.
fn run_size(s: &mut Session, size: Size) -> Result<Value> {
    let canvas = s.active().ok_or(EngineError::NoDocument)?.doc.bounds();
    let cut = s.execute("layer.removeBackground", json!({}))?;
    let num = |i: usize| cut["bounds"].get(i).and_then(Value::as_i64).and_then(|v| i32::try_from(v).ok());
    let subject = match (num(0), num(1), num(2), num(3)) {
        (Some(x), Some(y), Some(w), Some(h)) if w > 0 && h > 0 => Rect::new(x, y, x + w, y + h),
        _ => return Err(EngineError::Other("no subject found".into())),
    };
    let r = frame(subject, canvas, size).ok_or_else(|| EngineError::Other("nothing to frame".into()))?;
    s.execute(
        "image.crop",
        json!({
            "x": r.x0,
            "y": r.y0,
            "width": r.width(),
            "height": r.height(),
            "targetWidth": size.target_w(),
            "targetHeight": size.target_h(),
            "resolution": DPI,
        }),
    )?;
    Ok(json!({ "width": size.target_w(), "height": size.target_h(), "resolution": DPI }))
}

fn run_id_3x4(s: &mut Session, _p: &Value) -> Result<Value> {
    run_size(s, ID_3X4)
}

fn run_id_3_5x4_5(s: &mut Session, _p: &Value) -> Result<Value> {
    run_size(s, ID_3_5X4_5)
}

fn run_photo_4x6(s: &mut Session, _p: &Value) -> Result<Value> {
    run_size(s, PHOTO_4X6)
}

fn run_photo_6x9(s: &mut Session, _p: &Value) -> Result<Value> {
    run_size(s, PHOTO_6X9)
}

fn run_photo_9x12(s: &mut Session, _p: &Value) -> Result<Value> {
    run_size(s, PHOTO_9X12)
}

fn run_photo_10x15(s: &mut Session, _p: &Value) -> Result<Value> {
    run_size(s, PHOTO_10X15)
}

const PARAMS: &str = r##"{} → {width,height,resolution} (Remove Background, then crops to the size at 300 dpi)"##;

/// Photo size command specs. Their menu items live in `crates/ui-egui/src/menu_catalog.rs`.
pub fn specs() -> Vec<CommandSpec> {
    let spec = |id, label, run| CommandSpec { id, label, menu: &[], shortcut: None, params: PARAMS, enabled, run, journal: true };
    vec![
        spec("image.idPhoto.3x4", "ID Photo 3×4", run_id_3x4),
        spec("image.idPhoto.3_5x4_5", "ID Photo 3.5×4.5", run_id_3_5x4_5),
        spec("image.photoSize.4x6", "Photo 4×6", run_photo_4x6),
        spec("image.photoSize.6x9", "Photo 6×9", run_photo_6x9),
        spec("image.photoSize.9x12", "Photo 9×12", run_photo_9x12),
        spec("image.photoSize.10x15", "Photo 10×15", run_photo_10x15),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas() -> Rect {
        Rect::new(0, 0, 400, 300)
    }

    const ALL: [Size; 6] = [ID_3X4, ID_3_5X4_5, PHOTO_4X6, PHOTO_6X9, PHOTO_9X12, PHOTO_10X15];

    /// Every frame keeps the subject whole, stays on the canvas and matches its size's shape.
    #[test]
    fn frame_keeps_subject_inside_its_aspect_ratio() {
        let subject = Rect::new(120, 60, 260, 240);
        for size in ALL {
            let r = frame(subject, canvas(), size).unwrap();
            assert!(r.contains_rect(&subject));
            assert!(canvas().contains_rect(&r));
            let (w, h) = (f64::from(r.width()), f64::from(r.height()));
            let want = size.width_cm / size.height_cm;
            assert!((w / h - want).abs() < 0.02, "frame {w}x{h} is not {want}");
        }
    }

    /// A subject larger than the canvas still gets a frame that fits inside the canvas.
    #[test]
    fn frame_shrinks_to_the_canvas_for_a_large_subject() {
        let subject = Rect::new(0, 0, 400, 300);
        for size in ALL {
            let r = frame(subject, canvas(), size).unwrap();
            assert!(canvas().contains_rect(&r));
        }
    }

    #[test]
    fn frame_is_none_for_empty_input() {
        assert!(frame(Rect::EMPTY, canvas(), ID_3X4).is_none());
        assert!(frame(Rect::new(0, 0, 10, 10), Rect::EMPTY, ID_3X4).is_none());
    }

    /// Target pixels at 300 dpi match the sizes in centimetres.
    #[test]
    fn target_pixels_are_300_dpi() {
        assert_eq!((ID_3X4.target_w(), ID_3X4.target_h()), (354, 472));
        assert_eq!((ID_3_5X4_5.target_w(), ID_3_5X4_5.target_h()), (413, 531));
        assert_eq!((PHOTO_4X6.target_w(), PHOTO_4X6.target_h()), (472, 709));
        assert_eq!((PHOTO_6X9.target_w(), PHOTO_6X9.target_h()), (709, 1063));
        assert_eq!((PHOTO_9X12.target_w(), PHOTO_9X12.target_h()), (1063, 1417));
        assert_eq!((PHOTO_10X15.target_w(), PHOTO_10X15.target_h()), (1181, 1772));
    }
}

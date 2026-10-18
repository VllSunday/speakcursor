//! Windows virtual-desktop cursor tracking and full-monitor screenshots.
//! Nothing is persisted: frames and trace exist only for the active request.

use std::{
    ffi::c_void,
    io::Cursor,
    time::{Duration, Instant},
};

use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use slint::{Rgba8Pixel, SharedPixelBuffer};

type Handle = *mut c_void;
const MONITOR_DEFAULTTONEAREST: u32 = 2;
const SRCCOPY: u32 = 0x00cc0020;
const CAPTUREBLT: u32 = 0x40000000;
const BI_RGB: u32 = 0;
const DIB_RGB_COLORS: u32 = 0;
const TRAIL_LIFETIME: Duration = Duration::from_millis(500);
const TRAIL_SCALE: i32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, Eq, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
impl Rect {
    pub fn width(self) -> u32 {
        (self.right - self.left).max(0) as u32
    }
    pub fn height(self) -> u32 {
        (self.bottom - self.top).max(0) as u32
    }
}

#[repr(C)]
struct MonitorInfo {
    size: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
}

#[repr(C)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_pels_per_meter: i32,
    y_pels_per_meter: i32,
    clr_used: u32,
    clr_important: u32,
}
#[repr(C)]
struct BitmapInfo {
    header: BitmapInfoHeader,
    colors: [u32; 1],
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetCursorPos(point: *mut Point) -> i32;
    fn MonitorFromPoint(point: Point, flags: u32) -> Handle;
    fn GetMonitorInfoW(monitor: Handle, info: *mut MonitorInfo) -> i32;
    fn GetDC(hwnd: Handle) -> Handle;
    fn ReleaseDC(hwnd: Handle, dc: Handle) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateCompatibleDC(dc: Handle) -> Handle;
    fn DeleteDC(dc: Handle) -> i32;
    fn CreateCompatibleBitmap(dc: Handle, width: i32, height: i32) -> Handle;
    fn SelectObject(dc: Handle, object: Handle) -> Handle;
    fn DeleteObject(object: Handle) -> i32;
    fn BitBlt(
        dest: Handle,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        src: Handle,
        src_x: i32,
        src_y: i32,
        operation: u32,
    ) -> i32;
    fn GetDIBits(
        dc: Handle,
        bitmap: Handle,
        start: u32,
        lines: u32,
        bits: *mut c_void,
        info: *mut BitmapInfo,
        usage: u32,
    ) -> i32;
}

#[derive(Clone)]
pub struct MonitorPath {
    pub rect: Rect,
    pub points: Vec<Point>,
}

pub struct Session {
    pub monitors: Vec<MonitorPath>,
    last: Point,
    visual_points: Vec<TimedPoint>,
}

struct TimedPoint {
    point: Point,
    recorded_at: Instant,
}

pub struct Frame {
    pub label: String,
    pub original_png: Vec<u8>,
    pub marked_png: Option<Vec<u8>>,
}

pub fn cursor() -> Result<Point, String> {
    let mut point = Point::default();
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return Err("Не удалось получить позицию мыши".into());
    }
    Ok(point)
}

fn monitor_at(point: Point) -> Result<Rect, String> {
    let monitor = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_null() {
        return Err("Не удалось определить монитор".into());
    }
    let mut info = MonitorInfo {
        size: std::mem::size_of::<MonitorInfo>() as u32,
        monitor: Rect::default(),
        work: Rect::default(),
        flags: 0,
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return Err("Не удалось прочитать размеры монитора".into());
    }
    Ok(info.monitor)
}

impl Session {
    pub fn start() -> Result<Self, String> {
        let last = cursor()?;
        Ok(Self {
            monitors: vec![MonitorPath {
                rect: monitor_at(last)?,
                points: Vec::new(),
            }],
            last,
            visual_points: Vec::new(),
        })
    }

    pub fn sample(&mut self) -> Result<bool, String> {
        let current = cursor()?;
        let rect = monitor_at(current)?;
        self.visual_points
            .retain(|point| point.recorded_at.elapsed() < TRAIL_LIFETIME);
        Ok(self.record_position(current, rect))
    }

    fn record_position(&mut self, current: Point, rect: Rect) -> bool {
        if current == self.last {
            return false;
        }
        let recorded_at = Instant::now();
        if self.visual_points.is_empty() {
            self.visual_points.push(TimedPoint {
                point: self.last,
                recorded_at,
            });
        }
        self.visual_points.push(TimedPoint {
            point: current,
            recorded_at,
        });
        if let Some(path) = self
            .monitors
            .iter_mut()
            .find(|path| path.rect.left == rect.left && path.rect.top == rect.top)
        {
            if path.points.is_empty()
                && self.last.x >= rect.left
                && self.last.x < rect.right
                && self.last.y >= rect.top
                && self.last.y < rect.bottom
            {
                path.points.push(self.last);
            }
            path.points.push(current);
        } else {
            self.monitors.push(MonitorPath {
                rect,
                points: vec![current],
            });
        }
        self.last = current;
        true
    }

    pub fn capture(&self) -> Result<Vec<Frame>, String> {
        self.monitors
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let original = screenshot(path.rect)?;
                let original_png = encode_png(&original)?;
                let marked_png = if path.points.is_empty() {
                    None
                } else {
                    let mut marked = original.clone();
                    draw_path(marked.as_mut(), path.rect, &path.points, 4);
                    Some(encode_png(&marked)?)
                };
                Ok(Frame {
                    label: format!("Монитор {}", index + 1),
                    original_png,
                    marked_png,
                })
            })
            .collect()
    }

    pub fn overlay(&self) -> Option<(Rect, SharedPixelBuffer<Rgba8Pixel>)> {
        let now = Instant::now();
        let active = self
            .visual_points
            .iter()
            .filter(|point| now.duration_since(point.recorded_at) < TRAIL_LIFETIME)
            .collect::<Vec<_>>();
        if active.is_empty() {
            return None;
        }
        // Render at half resolution with a feathered edge. The full cursor
        // path above remains available for the screenshot sent to the model.
        let rect = virtual_desktop()?;
        let width = (rect.width() + TRAIL_SCALE as u32 - 1) / TRAIL_SCALE as u32;
        let height = (rect.height() + TRAIL_SCALE as u32 - 1) / TRAIL_SCALE as u32;
        let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
        draw_fading_trail(
            buffer.make_mut_bytes(),
            width as usize,
            height as usize,
            rect,
            &active,
            now,
        );
        Some((rect, buffer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_only_visited_monitors_and_leaves_still_cursor_unmarked() {
        let left = Rect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        };
        let right = Rect {
            left: 100,
            top: 0,
            right: 200,
            bottom: 100,
        };
        let mut session = Session {
            monitors: vec![MonitorPath {
                rect: left,
                points: vec![],
            }],
            last: Point { x: 30, y: 30 },
            visual_points: Vec::new(),
        };
        assert!(!session.record_position(Point { x: 30, y: 30 }, left));
        assert_eq!(session.monitors.len(), 1);
        assert!(session.monitors[0].points.is_empty());
        assert!(session.record_position(Point { x: 40, y: 40 }, left));
        assert_eq!(session.monitors[0].points.len(), 2);
        assert!(session.record_position(Point { x: 150, y: 40 }, right));
        assert_eq!(session.monitors.len(), 2);
        assert_eq!(session.monitors[1].points, vec![Point { x: 150, y: 40 }]);
        assert!(!session.visual_points.is_empty());
    }

    #[test]
    fn visible_trace_fades_without_discarding_request_path() {
        let now = Instant::now();
        let points = [
            TimedPoint {
                point: Point { x: 4, y: 4 },
                recorded_at: now,
            },
            TimedPoint {
                point: Point { x: 12, y: 4 },
                recorded_at: now,
            },
        ];
        let rect = Rect {
            left: 0,
            top: 0,
            right: 24,
            bottom: 24,
        };
        let mut fresh = vec![0u8; 12 * 12 * 4];
        let mut faded = fresh.clone();
        let references = points.iter().collect::<Vec<_>>();
        draw_fading_trail(&mut fresh, 12, 12, rect, &references, now);
        draw_fading_trail(
            &mut faded,
            12,
            12,
            rect,
            &references,
            now + Duration::from_millis(250),
        );
        let center = (2 * 12 + 4) * 4 + 3;
        assert!(fresh[center] > faded[center]);
        assert!(faded[center] > 0);
    }
}

fn draw_fading_trail(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    rect: Rect,
    points: &[&TimedPoint],
    now: Instant,
) {
    for pair in points.windows(2) {
        let start = pair[0].point;
        let end = pair[1].point;
        let dx = (end.x - start.x) as f32 / TRAIL_SCALE as f32;
        let dy = (end.y - start.y) as f32 / TRAIL_SCALE as f32;
        let steps = dx.abs().max(dy.abs()).ceil().max(1.0) as usize;
        let age = now.duration_since(pair[1].recorded_at).as_secs_f32();
        let opacity = (1.0 - age / TRAIL_LIFETIME.as_secs_f32())
            .clamp(0.0, 1.0)
            .powi(2);
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let x = (start.x - rect.left) as f32 / TRAIL_SCALE as f32 + dx * t;
            let y = (start.y - rect.top) as f32 / TRAIL_SCALE as f32 + dy * t;
            glow_dot(pixels, width, height, x, y, opacity);
        }
    }
    if let Some(last) = points.last() {
        let age = now.duration_since(last.recorded_at).as_secs_f32();
        let opacity = (1.0 - age / TRAIL_LIFETIME.as_secs_f32())
            .clamp(0.0, 1.0)
            .powi(2);
        let x = (last.point.x - rect.left) as f32 / TRAIL_SCALE as f32;
        let y = (last.point.y - rect.top) as f32 / TRAIL_SCALE as f32;
        glow_dot(pixels, width, height, x, y, opacity);
    }
}

fn glow_dot(pixels: &mut [u8], width: usize, height: usize, x: f32, y: f32, opacity: f32) {
    if opacity <= 0.0 {
        return;
    }
    // A pale narrow core and broad blue halo; fractional coordinates keep
    // diagonal movement smooth even when the overlay is scaled to the desktop.
    const RADIUS: i32 = 8;
    for py in (y.floor() as i32 - RADIUS)..=(y.ceil() as i32 + RADIUS) {
        if py < 0 || py as usize >= height {
            continue;
        }
        for px in (x.floor() as i32 - RADIUS)..=(x.ceil() as i32 + RADIUS) {
            if px < 0 || px as usize >= width {
                continue;
            }
            let distance2 = (px as f32 - x).powi(2) + (py as f32 - y).powi(2);
            let halo = (-distance2 / 14.0).exp() * 70.0;
            let core = (-distance2 / 1.2).exp() * 170.0;
            let alpha = ((halo + core) * opacity).min(240.0) as u8;
            let offset = (py as usize * width + px as usize) * 4;
            if alpha > pixels[offset + 3] {
                let center = (-distance2 / 1.2).exp();
                pixels[offset] = (74.0 + 98.0 * center) as u8;
                pixels[offset + 1] = (166.0 + 75.0 * center) as u8;
                pixels[offset + 2] = 255;
                pixels[offset + 3] = alpha;
            }
        }
    }
}

fn virtual_desktop() -> Option<Rect> {
    let left = unsafe { GetSystemMetrics(76) };
    let top = unsafe { GetSystemMetrics(77) };
    let width = unsafe { GetSystemMetrics(78) };
    let height = unsafe { GetSystemMetrics(79) };
    if width <= 0 || height <= 0 {
        return None;
    }
    Some(Rect {
        left,
        top,
        right: left + width,
        bottom: top + height,
    })
}

fn screenshot(rect: Rect) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, String> {
    let (w, h) = (rect.width() as i32, rect.height() as i32);
    if w <= 0 || h <= 0 || w > 16_384 || h > 16_384 {
        return Err("Некорректный размер монитора".into());
    }
    let source = unsafe { GetDC(std::ptr::null_mut()) };
    if source.is_null() {
        return Err("Не удалось открыть экран для снимка".into());
    }
    let memory = unsafe { CreateCompatibleDC(source) };
    let bitmap = unsafe { CreateCompatibleBitmap(source, w, h) };
    let mut result = Err("Не удалось получить снимок экрана".into());
    if !memory.is_null() && !bitmap.is_null() {
        let old = unsafe { SelectObject(memory, bitmap) };
        let copied = unsafe {
            BitBlt(
                memory,
                0,
                0,
                w,
                h,
                source,
                rect.left,
                rect.top,
                SRCCOPY | CAPTUREBLT,
            )
        };
        unsafe { SelectObject(memory, old) };
        if copied != 0 {
            let mut bgra = vec![0u8; w as usize * h as usize * 4];
            let mut info = BitmapInfo {
                header: BitmapInfoHeader {
                    size: 40,
                    width: w,
                    height: -h,
                    planes: 1,
                    bit_count: 32,
                    compression: BI_RGB,
                    size_image: 0,
                    x_pels_per_meter: 0,
                    y_pels_per_meter: 0,
                    clr_used: 0,
                    clr_important: 0,
                },
                colors: [0],
            };
            if unsafe {
                GetDIBits(
                    memory,
                    bitmap,
                    0,
                    h as u32,
                    bgra.as_mut_ptr().cast(),
                    &mut info,
                    DIB_RGB_COLORS,
                )
            } == h
            {
                for pixel in bgra.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                    pixel[3] = 255;
                }
                result = ImageBuffer::from_raw(w as u32, h as u32, bgra)
                    .ok_or_else(|| "Не удалось собрать изображение".into());
            }
        }
    }
    if !bitmap.is_null() {
        unsafe { DeleteObject(bitmap) };
    }
    if !memory.is_null() {
        unsafe { DeleteDC(memory) };
    }
    unsafe { ReleaseDC(std::ptr::null_mut(), source) };
    result
}

fn encode_png(image: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Result<Vec<u8>, String> {
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image.clone())
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|error| format!("Не удалось кодировать снимок: {error}"))?;
    Ok(output.into_inner())
}

fn draw_path(pixels: &mut [u8], rect: Rect, points: &[Point], radius: i32) {
    if points.is_empty() {
        return;
    }
    for pair in points.windows(2) {
        // Interpolate enough points that quick mouse movement remains a line.
        let dx = pair[1].x - pair[0].x;
        let dy = pair[1].y - pair[0].y;
        let steps = dx.abs().max(dy.abs()).max(1);
        for step in 0..=steps {
            let x = pair[0].x + dx * step / steps - rect.left;
            let y = pair[0].y + dy * step / steps - rect.top;
            dot(
                pixels,
                rect.width() as usize,
                rect.height() as usize,
                x,
                y,
                radius,
            );
        }
    }
    let point = points[points.len() - 1];
    dot(
        pixels,
        rect.width() as usize,
        rect.height() as usize,
        point.x - rect.left,
        point.y - rect.top,
        radius,
    );
}

fn dot(pixels: &mut [u8], width: usize, height: usize, x: i32, y: i32, radius: i32) {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let (px, py) = (x + dx, y + dy);
            if px < 0
                || py < 0
                || px as usize >= width
                || py as usize >= height
                || dx * dx + dy * dy > radius * radius
            {
                continue;
            }
            let offset = (py as usize * width + px as usize) * 4;
            let color = &mut pixels[offset..offset + 4];
            let alpha = if dx * dx + dy * dy <= (radius / 2).max(0).pow(2) {
                225u16
            } else {
                80u16
            };
            for (channel, value) in [(0, 45u16), (1, 160), (2, 255)] {
                color[channel] =
                    ((color[channel] as u16 * (255 - alpha) + value * alpha) / 255) as u8;
            }
            color[3] = color[3].max(alpha as u8);
        }
    }
}

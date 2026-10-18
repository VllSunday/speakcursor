//! Windows virtual-desktop cursor tracking and full-monitor screenshots.
//! Nothing is persisted: frames and trace exist only for the active request.

use std::{ffi::c_void, io::Cursor};

use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use slint::{Rgba8Pixel, SharedPixelBuffer};

type Handle = *mut c_void;
const MONITOR_DEFAULTTONEAREST: u32 = 2;
const SRCCOPY: u32 = 0x00cc0020;
const CAPTUREBLT: u32 = 0x40000000;
const BI_RGB: u32 = 0;
const DIB_RGB_COLORS: u32 = 0;

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
}

#[derive(Clone)]
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
        })
    }

    pub fn sample(&mut self) -> Result<bool, String> {
        let current = cursor()?;
        if current == self.last {
            return Ok(false);
        }
        let rect = monitor_at(current)?;
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
        Ok(true)
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
        // A quarter-resolution transparent virtual desktop keeps old traces
        // visible when the cursor crosses monitors without 4K redraw cost.
        let rect = virtual_desktop()?;
        let width = (rect.width() + 3) / 4;
        let height = (rect.height() + 3) / 4;
        let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
        let drawing_rect = Rect {
            left: 0,
            top: 0,
            right: width as i32,
            bottom: height as i32,
        };
        for path in &self.monitors {
            let points = path
                .points
                .iter()
                .map(|point| Point {
                    x: (point.x - rect.left) / 4,
                    y: (point.y - rect.top) / 4,
                })
                .collect::<Vec<_>>();
            draw_path(buffer.make_mut_bytes(), drawing_rect, &points, 1);
        }
        Some((rect, buffer))
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
            } != 0
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

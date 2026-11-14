//! Graphics: a premultiplied-alpha swap chain shown through DirectComposition,
//! drawn with Direct2D by replaying the core's draw list.

use std::collections::HashMap;

use snow_core::{Prim, Rgba};
use windows::core::{Interface, Result};
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use windows::Win32::Graphics::Direct3D11::{D3D11CreateDevice, ID3D11Device, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION};
use windows::Win32::Graphics::DirectComposition::{DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::*;
use windows_numerics::Vector2;

pub struct Gfx {
    swap: IDXGISwapChain1,
    factory: ID2D1Factory1,
    dc: ID2D1DeviceContext,
    brush: ID2D1SolidColorBrush,
    round: ID2D1StrokeStyle1,
    glows: HashMap<[u8; 3], ID2D1RadialGradientBrush>,
    // Kept alive for as long as the overlay shows this swap chain.
    _dcomp: IDCompositionDevice,
    _target: IDCompositionTarget,
    _visual: IDCompositionVisual,
}

fn color(c: Rgba) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r: c.r, g: c.g, b: c.b, a: c.a }
}

fn pt(p: (f32, f32)) -> Vector2 {
    Vector2 { X: p.0, Y: p.1 }
}

fn create_d3d(driver: D3D_DRIVER_TYPE) -> Result<ID3D11Device> {
    let mut device = None;
    unsafe {
        D3D11CreateDevice(None, driver, HMODULE::default(), D3D11_CREATE_DEVICE_BGRA_SUPPORT, None, D3D11_SDK_VERSION, Some(&mut device), None, None)?;
    }
    device.ok_or_else(windows::core::Error::empty)
}

impl Gfx {
    pub fn new(hwnd: HWND, w: u32, h: u32) -> Result<Self> {
        unsafe {
            // Hardware first; the software rasteriser keeps it working on
            // machines and remote sessions without a usable GPU.
            let d3d = create_d3d(D3D_DRIVER_TYPE_HARDWARE).or_else(|_| create_d3d(D3D_DRIVER_TYPE_WARP))?;
            let dxgi_device: IDXGIDevice = d3d.cast()?;
            let dxgi_factory: IDXGIFactory2 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;

            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: w.max(1),
                Height: h.max(1),
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                ..Default::default()
            };
            let swap = dxgi_factory.CreateSwapChainForComposition(&dxgi_device, &desc, None)?;

            let factory: ID2D1Factory1 = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let d2d_device = factory.CreateDevice(&dxgi_device)?;
            let dc = d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;

            // With a flip-model swap chain in D3D11, buffer 0 is always the
            // current back buffer, so one target bitmap serves every frame.
            let surface: IDXGISurface = swap.GetBuffer(0)?;
            let props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                ..Default::default()
            };
            let bitmap = dc.CreateBitmapFromDxgiSurface(&surface, Some(&props))?;
            dc.SetTarget(&bitmap);
            dc.SetDpi(96.0, 96.0);

            let brush = dc.CreateSolidColorBrush(&color(Rgba::WHITE), None)?;
            let round = factory.CreateStrokeStyle(
                &D2D1_STROKE_STYLE_PROPERTIES1 {
                    startCap: D2D1_CAP_STYLE_ROUND,
                    endCap: D2D1_CAP_STYLE_ROUND,
                    dashCap: D2D1_CAP_STYLE_ROUND,
                    lineJoin: D2D1_LINE_JOIN_ROUND,
                    miterLimit: 10.0,
                    dashStyle: D2D1_DASH_STYLE_SOLID,
                    dashOffset: 0.0,
                    transformType: D2D1_STROKE_TRANSFORM_TYPE_NORMAL,
                },
                None,
            )?;

            let dcomp: IDCompositionDevice = DCompositionCreateDevice(&dxgi_device)?;
            let target = dcomp.CreateTargetForHwnd(hwnd, true)?;
            let visual = dcomp.CreateVisual()?;
            visual.SetContent(&swap)?;
            target.SetRoot(&visual)?;
            dcomp.Commit()?;

            Ok(Self { swap, factory, dc, brush, round, glows: HashMap::new(), _dcomp: dcomp, _target: target, _visual: visual })
        }
    }

    /// Draw one frame and present it. `sync_interval` 1 = every refresh, 2 = every other.
    /// An error means the device was lost; the caller rebuilds `Gfx`.
    pub fn render(&mut self, prims: &[Prim], sync_interval: u32) -> Result<()> {
        unsafe {
            self.dc.BeginDraw();
            self.dc.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
            for prim in prims {
                // A primitive that fails to build is skipped, not fatal.
                let _ = self.draw(prim);
            }
            self.dc.EndDraw(None, None)?;
            self.swap.Present(sync_interval, DXGI_PRESENT(0)).ok()
        }
    }

    unsafe fn draw(&mut self, prim: &Prim) -> Result<()> {
        match prim {
            Prim::Circle { x, y, r, color: c } => {
                self.brush.SetColor(&color(*c));
                self.dc.FillEllipse(&D2D1_ELLIPSE { point: pt((*x, *y)), radiusX: *r, radiusY: *r }, &self.brush);
            }
            Prim::Glow { x, y, r, color: c } => {
                if *r <= 0.0 {
                    return Ok(());
                }
                let brush = self.glow_brush(*c)?;
                brush.SetCenter(pt((*x, *y)));
                brush.SetRadiusX(*r);
                brush.SetRadiusY(*r);
                brush.SetOpacity(c.a.clamp(0.0, 1.0));
                self.dc.FillEllipse(&D2D1_ELLIPSE { point: pt((*x, *y)), radiusX: *r, radiusY: *r }, &brush);
            }
            Prim::Line { x0, y0, x1, y1, width, color: c } => {
                self.brush.SetColor(&color(*c));
                self.dc.DrawLine(pt((*x0, *y0)), pt((*x1, *y1)), &self.brush, *width, &self.round);
            }
            Prim::Polyline { pts, width, color: c } => {
                if pts.len() >= 2 {
                    let geo = self.path(pts, false)?;
                    self.brush.SetColor(&color(*c));
                    self.dc.DrawGeometry(&geo, &self.brush, *width, &self.round);
                }
            }
            Prim::Polygon { pts, color: c } => {
                if pts.len() >= 3 {
                    let geo = self.path(pts, true)?;
                    self.brush.SetColor(&color(*c));
                    self.dc.FillGeometry(&geo, &self.brush, None);
                }
            }
            Prim::RoundRect { x, y, w, h, radius, color: c } => {
                if *w > 0.0 && *h > 0.0 {
                    let r = radius.max(0.0).min(w / 2.0).min(h / 2.0);
                    self.brush.SetColor(&color(*c));
                    let rect = D2D_RECT_F { left: *x, top: *y, right: x + w, bottom: y + h };
                    self.dc.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect, radiusX: r, radiusY: r }, &self.brush);
                }
            }
        }
        Ok(())
    }

    /// One radial brush per RGB colour, fading to transparent at the edge.
    /// Per-draw alpha is applied through the brush opacity.
    unsafe fn glow_brush(&mut self, c: Rgba) -> Result<ID2D1RadialGradientBrush> {
        let key = [(c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8];
        if let Some(b) = self.glows.get(&key) {
            return Ok(b.clone());
        }
        let stops = [
            D2D1_GRADIENT_STOP { position: 0.0, color: D2D1_COLOR_F { r: c.r, g: c.g, b: c.b, a: 1.0 } },
            D2D1_GRADIENT_STOP { position: 1.0, color: D2D1_COLOR_F { r: c.r, g: c.g, b: c.b, a: 0.0 } },
        ];
        let collection = self.dc.CreateGradientStopCollection(
            &stops,
            D2D1_COLOR_SPACE_SRGB,
            D2D1_COLOR_SPACE_SRGB,
            D2D1_BUFFER_PRECISION_8BPC_UNORM,
            D2D1_EXTEND_MODE_CLAMP,
            D2D1_COLOR_INTERPOLATION_MODE_STRAIGHT,
        )?;
        let props = D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES { center: pt((0.0, 0.0)), gradientOriginOffset: pt((0.0, 0.0)), radiusX: 1.0, radiusY: 1.0 };
        let brush = self.dc.CreateRadialGradientBrush(&props, None, &collection)?;
        self.glows.insert(key, brush.clone());
        Ok(brush)
    }

    unsafe fn path(&self, pts: &[(f32, f32)], filled: bool) -> Result<ID2D1PathGeometry1> {
        let geo = self.factory.CreatePathGeometry()?;
        let sink = geo.Open()?;
        sink.BeginFigure(pt(pts[0]), if filled { D2D1_FIGURE_BEGIN_FILLED } else { D2D1_FIGURE_BEGIN_HOLLOW });
        let rest: Vec<Vector2> = pts[1..].iter().map(|p| pt(*p)).collect();
        sink.AddLines(&rest);
        sink.EndFigure(if filled { D2D1_FIGURE_END_CLOSED } else { D2D1_FIGURE_END_OPEN });
        sink.Close()?;
        Ok(geo)
    }
}

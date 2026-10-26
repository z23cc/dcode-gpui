//! Phase 0 throwaway, not product code: a zoomable tile canvas and a WebView
//! tile, to measure GPUI before the real canvas is built. The pass criteria and
//! the results live in the GPUI rewrite analysis artifact.
//!
//!   cargo run --release -p canvas-lab [--features fast] -- [flags]
//!     --tiles N     tiles on the canvas (default 64)
//!     --bench       animate pan 5 s, then zoom 5 s, print frame stats, quit
//!     --no-webview  leave the WebView tile out
//!     --overlay     open a deferred overlay and a gpui-kit Popover over the page
//!     --clip        pan so the WebView tile crosses the canvas edge into the sidebar
//!     --zoom Z      start at zoom Z
//!     --quantize N  snap the zoom to 2^(k/N), so text sizes repeat (glyph and layout caches hit)
//!     --secs S      length of each benchmark phase (default 5)
//!     --pan-y Y     start with the canvas panned to Y (default 20)

use gpui_kit::component::{ActiveTheme as _, ThemeColor, button::Button, popover::Popover};
use gpui_kit::*;
use gpui_webview::WebView;
use std::time::{Duration, Instant};

const SIDEBAR_W: f32 = 200.;
const TOOLBAR_H: f32 = 44.;
const BASE_REM: f32 = 16.;
/// Tile size in rems (so it scales with the zoom) and grid pitch in canvas pixels.
const TILE: (f32, f32) = (20., 14.);
const PITCH: (f32, f32) = (340., 250.);
const COLUMNS: usize = 8;
/// The WebView tile: canvas position in pixels, size in rems. Tiles start below it.
const WEB_AT: (f32, f32) = (20., 20.);
const WEB_SIZE: (f32, f32) = (30., 20.);
const GRID_TOP: f32 = 380.;

const LINES: [&str; 6] = [
    "Read crates/dcode-client/src/lib.rs (261 lines)",
    "Edited src/main.rs: +42 −7",
    "Ran cargo clippy --workspace -- -D warnings",
    "All 4 tests passed in 0.01 s",
    "Thinking: the handshake needs the floor manifest…",
    "Waiting for approval to run `git push`",
];

const PAGE: &str = r#"<html><body style="margin:0;height:100vh;display:flex;flex-direction:column;
align-items:center;justify-content:center;font:16px -apple-system;color:white;
background:linear-gradient(135deg,#2563eb,#db2777)"><h1 style="margin:0">WebView page</h1>
<p id="c">clicks on the page: 0</p><script>let n=0;document.body.onclick=()=>{
document.getElementById('c').textContent='clicks on the page: '+(++n)}</script></body></html>"#;

struct Args {
    tiles: usize,
    bench: bool,
    webview: bool,
    overlay: bool,
    clip: bool,
    zoom: f32,
    quantize: Option<f32>,
    phase: Duration,
    pan_y: f32,
}

impl Args {
    fn parse() -> Self {
        let mut args = Self {
            tiles: 64,
            bench: false,
            webview: true,
            overlay: false,
            clip: false,
            zoom: 1.,
            quantize: None,
            phase: Duration::from_secs(5),
            pan_y: 20.,
        };
        let mut it = std::env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--tiles" => {
                    args.tiles = it.next().and_then(|n| n.parse().ok()).expect("--tiles N")
                }
                "--zoom" => args.zoom = it.next().and_then(|z| z.parse().ok()).expect("--zoom Z"),
                "--quantize" => {
                    args.quantize = Some(
                        it.next()
                            .and_then(|n| n.parse().ok())
                            .expect("--quantize N"),
                    )
                }
                "--secs" => {
                    args.phase = Duration::from_secs(
                        it.next().and_then(|s| s.parse().ok()).expect("--secs S"),
                    )
                }
                "--pan-y" => {
                    args.pan_y = it.next().and_then(|y| y.parse().ok()).expect("--pan-y Y")
                }
                "--bench" => args.bench = true,
                "--no-webview" => args.webview = false,
                "--overlay" => args.overlay = true,
                "--clip" => args.clip = true,
                // `open --args` and cargo may pass others; ignore them.
                _ => {}
            }
        }
        args
    }
}

/// Lays out, prepaints and paints its child with `rem` overridden, so every
/// rem-based length inside (text, padding, radii, tile size) follows the zoom.
/// All three phases need it: paint resolves borders and masks from rem too.
struct RemScaled {
    rem: Pixels,
    child: AnyElement,
}

impl IntoElement for RemScaled {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for RemScaled {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let layout = window.with_rem_size(Some(self.rem), |window| {
            self.child.request_layout(window, cx)
        });
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_rem_size(Some(self.rem), |window| {
            self.child.prepaint(window, cx);
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_rem_size(Some(self.rem), |window| self.child.paint(window, cx));
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Pan,
    Zoom,
}

struct Bench {
    phase: Phase,
    started: Instant,
    last: Instant,
    intervals: Vec<Duration>,
}

struct Lab {
    tiles: usize,
    zoom: f32,
    quantize: Option<f32>,
    phase: Duration,
    pan: (f32, f32),
    webview: Option<Entity<WebView>>,
    page_zoom: f32,
    overlay: bool,
    clicked: SharedString,
    bench: Option<Bench>,
    last_bench: SharedString,
}

impl Lab {
    fn new(args: Args, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let webview = args.webview.then(|| {
            cx.new(|cx| {
                WebView::build(wry::WebViewBuilder::new().with_html(PAGE), window, cx)
                    .expect("failed to create the WebView")
            })
        });
        if args.bench {
            // One second to settle, then measure.
            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                this.update(cx, |lab, cx| {
                    let now = Instant::now();
                    println!(
                        "{} · {} tiles · webview {}",
                        backend(),
                        lab.tiles,
                        lab.webview.is_some()
                    );
                    println!("rss before: {}", rss());
                    lab.bench = Some(Bench {
                        phase: Phase::Pan,
                        started: now,
                        last: now,
                        intervals: Vec::new(),
                    });
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        Self {
            tiles: args.tiles,
            zoom: args.zoom,
            quantize: args.quantize,
            phase: args.phase,
            pan: (if args.clip { -250. } else { 20. }, args.pan_y),
            webview,
            page_zoom: 1.,
            overlay: args.overlay,
            clicked: "nothing clicked".into(),
            bench: None,
            last_bench: "".into(),
        }
    }

    /// One frame of the benchmark: record the interval since the last frame,
    /// move the canvas, and ask for the next frame.
    fn step_bench(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = &mut self.bench else {
            return;
        };
        let now = Instant::now();
        let interval = now - bench.last;
        if interval > Duration::from_secs(1) {
            println!(
                "stall: {} ms before frame {} (window active: {})",
                interval.as_millis(),
                bench.intervals.len(),
                window.is_window_active()
            );
        }
        bench.intervals.push(interval);
        bench.last = now;
        let elapsed = now - bench.started;
        let wave = (elapsed.as_secs_f32() * std::f32::consts::TAU / 2.5).sin();
        let phase = bench.phase;
        if elapsed >= self.phase {
            // The first interval spans the warm-up; leave it out.
            let summary = report(phase, &bench.intervals[1..]);
            println!("{summary}");
            self.last_bench = summary.into();
            match phase {
                Phase::Pan => {
                    bench.phase = Phase::Zoom;
                    bench.started = now;
                    bench.intervals.clear();
                }
                Phase::Zoom => {
                    println!("rss after: {}", rss());
                    self.bench = None;
                    cx.quit();
                    return;
                }
            }
        }
        match phase {
            Phase::Pan => self.pan = (-300. + 320. * wave, 20.),
            Phase::Zoom => {
                let zoom = 0.9 + 0.6 * wave;
                self.zoom = match self.quantize {
                    Some(n) => ((zoom.log2() * n).round() / n).exp2(),
                    None => zoom,
                };
            }
        }
        window.request_animation_frame();
    }

    fn on_scroll(&mut self, event: &ScrollEvent, window: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(window.line_height());
        let (dx, dy) = (f32::from(delta.x), f32::from(delta.y));
        if event.modifiers.platform || event.modifiers.control {
            let at = (
                f32::from(event.position.x) - SIDEBAR_W,
                f32::from(event.position.y) - TOOLBAR_H,
            );
            self.zoom_at(at, (1. + dy * 0.005).clamp(0.8, 1.25));
        } else {
            self.pan = (self.pan.0 + dx, self.pan.1 + dy);
        }
        cx.notify();
    }

    /// Zoom by `factor`, keeping the canvas point under `at` where it is.
    fn zoom_at(&mut self, at: (f32, f32), factor: f32) {
        let zoom = (self.zoom * factor).clamp(0.2, 3.);
        let k = zoom / self.zoom;
        self.pan = (
            at.0 - (at.0 - self.pan.0) * k,
            at.1 - (at.1 - self.pan.1) * k,
        );
        self.zoom = zoom;
    }

    fn place(&self, x: f32, y: f32) -> (Pixels, Pixels) {
        (
            px(x * self.zoom + self.pan.0),
            px(y * self.zoom + self.pan.1),
        )
    }
}

impl Render for Lab {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.step_bench(window, cx);
        if let Some(webview) = &self.webview
            && self.page_zoom != self.zoom
        {
            let _ = webview.read(cx).set_zoom(self.zoom as f64);
            self.page_zoom = self.zoom;
        }
        let colors = cx.theme().colors;

        let mut layer = div().absolute().size_full();
        if let Some(webview) = &self.webview {
            let (left, top) = self.place(WEB_AT.0, WEB_AT.1);
            layer = layer.child(
                div()
                    .absolute()
                    .left(left)
                    .top(top)
                    .w(rems(WEB_SIZE.0))
                    .h(rems(WEB_SIZE.1))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded_lg()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.background)
                    .child(tile_title("WebView tile", colors))
                    .child(div().flex_1().child(webview.clone())),
            );
        }
        layer = layer.children((0..self.tiles).map(|ix| {
            let (x, y) = (
                (ix % COLUMNS) as f32 * PITCH.0,
                GRID_TOP + (ix / COLUMNS) as f32 * PITCH.1,
            );
            let (left, top) = self.place(x, y);
            tile(ix, left, top, colors, cx)
        }));

        let canvas = div()
            .id("canvas")
            .relative()
            .flex_1()
            .overflow_hidden()
            .bg(colors.secondary)
            .on_scroll(cx.listener(Self::on_scroll))
            .child(RemScaled {
                rem: px(BASE_REM * self.zoom),
                child: layer.into_any_element(),
            });

        let toolbar = div()
            .h(px(TOOLBAR_H))
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .border_b_1()
            .border_color(colors.border)
            .child(
                Popover::new("popover")
                    .default_open(self.overlay)
                    .trigger(Button::new("popover-trigger").label("Popover"))
                    .content(|_, _, _| {
                        div()
                            .w(px(260.))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child("gpui-kit Popover over the page")
                            .child(Button::new("popover-button").label("Popover button"))
                    }),
            )
            .child(
                Button::new("overlay")
                    .label("Overlay")
                    .on_click(cx.listener(|lab, _, _, cx| {
                        lab.overlay = !lab.overlay;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("zoom-out")
                    .label("−")
                    .on_click(cx.listener(|lab, _, _, cx| {
                        lab.zoom_at((400., 300.), 0.8);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("zoom-in")
                    .label("+")
                    .on_click(cx.listener(|lab, _, _, cx| {
                        lab.zoom_at((400., 300.), 1.25);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child(format!(
                        "{} · zoom {:.2} · {}",
                        backend(),
                        self.zoom,
                        self.clicked
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(self.last_bench.clone()),
            );

        let sidebar = div()
            .w(px(SIDEBAR_W))
            .h_full()
            .flex_none()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .bg(colors.sidebar)
            .border_r_1()
            .border_color(colors.sidebar_border)
            .child(
                div()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child("GPUI SIDEBAR"),
            )
            .children(LINES.iter().map(|line| div().text_sm().child(*line)));

        div()
            .size_full()
            .flex()
            .bg(colors.background)
            .text_color(colors.foreground)
            .child(sidebar)
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(toolbar)
                    .child(canvas),
            )
            .children(self.overlay.then(|| {
                // Over the WebView tile's body, in window coordinates.
                let (left, top) = self.place(WEB_AT.0 + 120., WEB_AT.1 + 90.);
                deferred(
                    anchored()
                        .position(point(left + px(SIDEBAR_W), top + px(TOOLBAR_H)))
                        .child(
                            div()
                                .w(px(280.))
                                .p_3()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .rounded_lg()
                                .border_2()
                                .border_color(colors.danger)
                                .bg(colors.background)
                                .shadow_lg()
                                .child("GPUI deferred overlay")
                                .child(
                                    Button::new("overlay-button")
                                        .label("Overlay button")
                                        .on_click(cx.listener(|lab, _, _, cx| {
                                            lab.clicked = "overlay button".into();
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
            }))
    }
}

fn tile_title(title: impl Into<SharedString>, colors: ThemeColor) -> impl IntoElement {
    div()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(colors.border)
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .child(title.into())
}

fn tile(ix: usize, left: Pixels, top: Pixels, colors: ThemeColor, cx: &Context<Lab>) -> Div {
    div()
        .absolute()
        .left(left)
        .top(top)
        .w(rems(TILE.0))
        .h(rems(TILE.1))
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded_lg()
        .border_1()
        .border_color(colors.border)
        .bg(colors.background)
        .child(tile_title(format!("Agent {ix} · Claude Code"), colors))
        .child(
            div()
                .flex_1()
                .px_3()
                .py_2()
                .flex()
                .flex_col()
                .gap_1()
                .text_xs()
                .text_color(colors.muted_foreground)
                .children(LINES.iter().map(|line| div().child(*line))),
        )
        .child(
            div()
                .px_3()
                .pb_3()
                .child(
                    Button::new(("approve", ix))
                        .label("Approve")
                        .on_click(cx.listener(move |lab, _, _, cx| {
                            lab.clicked = format!("tile {ix}").into();
                            cx.notify();
                        })),
                ),
        )
}

fn backend() -> &'static str {
    if GPUI_FAST { "gpui-fast" } else { "gpui-pre" }
}

fn report(phase: Phase, intervals: &[Duration]) -> String {
    let name = if phase == Phase::Pan { "pan" } else { "zoom" };
    let mut ms: Vec<f64> = intervals.iter().map(|d| d.as_secs_f64() * 1e3).collect();
    if ms.is_empty() {
        return format!("{name}: no frames");
    }
    ms.sort_by(f64::total_cmp);
    let at = |q: f64| ms[((ms.len() - 1) as f64 * q).round() as usize];
    let over = |limit: f64| ms.iter().filter(|&&m| m > limit).count();
    format!(
        "{name}: {} frames, {:.1} fps, p50 {:.2} ms, p95 {:.2} ms, max {:.2} ms, >16.7 ms {}, >50 ms {}",
        ms.len(),
        ms.len() as f64 * 1e3 / ms.iter().sum::<f64>(),
        at(0.5),
        at(0.95),
        ms[ms.len() - 1],
        over(16.7),
        over(50.),
    )
}

fn rss() -> String {
    std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|kb| kb.trim().parse::<u64>().ok())
        .map_or("?".into(), |kb| format!("{} MB", kb / 1024))
}

fn main() {
    let args = Args::parse();
    application().with_assets(assets::Assets).run(move |cx| {
        init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some(format!("canvas-lab · {}", backend()).into()),
                ..Default::default()
            }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1200.), px(800.)),
                cx,
            ))),
            // macOS stops the display link of a covered window, which reads as a
            // multi-second stall; a benchmark window stays on top.
            kind: if args.bench {
                WindowKind::PopUp
            } else {
                WindowKind::Normal
            },
            ..Default::default()
        };
        open_window(options, cx, |window, cx| {
            cx.new(|cx| Lab::new(args, window, cx))
        })
        .expect("failed to open the window");
        cx.activate(true);
    });
}

use dioxus::prelude::*;

/// Color of inline charts. Resolves to a Tailwind `text-*` class so the
/// SVG/div elements use `currentColor` / `bg-current` for both stroke
/// and fill — keeps the components agnostic of theme details.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum ChartColor {
    #[default]
    Brand,
    Ok,
    Warn,
    Bad,
    Info,
    /// Faint dim for inactive trends.
    Muted,
}

impl ChartColor {
    pub fn text_class(self) -> &'static str {
        match self {
            Self::Brand => "text-brand",
            Self::Ok    => "text-success",
            Self::Warn  => "text-warn",
            Self::Bad   => "text-danger",
            Self::Info  => "text-info",
            Self::Muted => "text-fg-faint",
        }
    }
}

/// Inline SVG sparkline. Uses `currentColor` for stroke + the same
/// color at 14% opacity for the underglow fill (matches the design
/// language reference).
///
/// Width fills the parent (`100%`), height is fixed and configurable.
/// `vector-effect: non-scaling-stroke` keeps the line crisp regardless
/// of how the SVG scales horizontally — important when these sit
/// inside flex/grid layouts that change width.
#[component]
pub fn Sparkline(
    data: Vec<f64>,
    #[props(default)] color: ChartColor,
    #[props(default = 36)] height: u32,
    #[props(default = true)] fill: bool,
    #[props(default, into)] class: String,
) -> Element {
    let n = data.len();
    if n < 2 {
        return rsx! { div { class: "h-{height}" } };
    }
    let w: f64 = 200.0;
    let h: f64 = height as f64;
    let min = data.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let span = max - min;

    let mut path = String::new();
    for (i, v) in data.iter().enumerate() {
        let x = (i as f64) / ((n - 1) as f64) * w;
        let y = if span == 0.0 {
            h / 2.0
        } else {
            h - ((v - min) / span) * (h - 4.0) - 2.0
        };
        if i == 0 {
            path.push_str(&format!("M {x:.2} {y:.2}"));
        } else {
            path.push_str(&format!(" L {x:.2} {y:.2}"));
        }
    }
    let area = format!("{path} L {w} {h} L 0 {h} Z");
    let view_box = format!("0 0 {w} {h}");
    let color_cls = color.text_class();
    let cls = format!("block w-full {color_cls} {class}");

    rsx! {
        svg {
            class: "{cls}",
            width: "100%",
            height: "{height}",
            "viewBox": "{view_box}",
            "preserveAspectRatio": "none",
            if fill {
                path {
                    d: "{area}",
                    fill: "currentColor",
                    opacity: "0.14",
                }
            }
            path {
                d: "{path}",
                stroke: "currentColor",
                "stroke-width": "1.4",
                fill: "none",
                "stroke-linecap": "round",
                "stroke-linejoin": "round",
                "vector-effect": "non-scaling-stroke",
            }
        }
    }
}

/// Vertical bar histogram — used for activity-per-hour displays. Bars
/// fill from the right with a subtle opacity ramp so the most recent
/// data point reads strongest, matching the design's hourly histograms.
#[component]
pub fn Bars(
    data: Vec<f64>,
    #[props(default)] color: ChartColor,
    #[props(default = 56)] height: u32,
    #[props(default, into)] class: String,
) -> Element {
    let max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max).max(1.0);
    let n = data.len();
    let color_cls = color.text_class();
    let cls = format!("flex items-end gap-[3px] {color_cls} {class}");

    rsx! {
        div { class: "{cls}", style: "height: {height}px;",
            for (i, v) in data.iter().copied().enumerate() {
                {
                    let pct = (v / max * 100.0).clamp(0.0, 100.0);
                    let opacity = 1.0 - ((n - 1 - i) as f64) * 0.01;
                    let style = format!("height: {pct:.1}%; opacity: {opacity:.2};");
                    rsx! {
                        div {
                            key: "{i}",
                            class: "flex-1 rounded-sm bg-current",
                            style: "{style}",
                        }
                    }
                }
            }
        }
    }
}

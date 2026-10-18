//! YAML projection and the canonical self-contained SVG figure stream.
use crate::report::Report;
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::fmt::Write;

pub fn yaml(report: &Report) -> Result<String> {
    let mut output = String::from("---\n");
    yaml_mapping(&mut output, &crate::metrics::yaml_summary(report), 0)?;
    Ok(output)
}

fn yaml_mapping(output: &mut String, value: &Value, indent: usize) -> Result<()> {
    if let Value::Object(fields) = value {
        for (key, value) in fields {
            write!(
                output,
                "{}{}:",
                " ".repeat(indent),
                serde_json::to_string(key)?
            )
            .unwrap();
            if value.as_object().is_some_and(|v| !v.is_empty()) {
                output.push('\n');
                yaml_mapping(output, value, indent + 2)?;
            } else {
                writeln!(output, " {}", serde_json::to_string(value)?).unwrap();
            }
        }
    }
    Ok(())
}

pub struct Figure {
    pub name: String,
    pub svg: String,
}

/// Extract only our renderer's complete SVG figures, never arbitrary input HTML.
/// Shared classes/CSS remain embedded, and caption IDs become internal titles.
pub fn figures(html: &str, small: bool, mut emit: impl FnMut(Figure) -> Result<()>) -> Result<()> {
    let css = figure_style();
    let mut remaining = html;
    while let Some(start) = remaining.find("<svg ") {
        remaining = &remaining[start..];
        let end = remaining
            .find("</svg>")
            .context("incomplete generated SVG")?
            + 6;
        let inline = &remaining[..end];
        remaining = &remaining[end..];
        let opening_end = inline.find('>').context("incomplete SVG opening tag")?;
        let opening = &inline[..opening_end];
        let Some(label) = attribute(opening, "aria-labelledby") else {
            continue;
        }; // brand icon
        let id = label
            .strip_suffix("-caption")
            .context("unexpected generated figure caption ID")?;
        ensure!(
            !id.is_empty()
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "unsafe generated figure ID"
        );
        let dimensions: Vec<f64> = attribute(opening, "viewBox")
            .context("SVG has no viewBox")?
            .split_whitespace()
            .map(str::parse)
            .collect::<std::result::Result<_, _>>()
            .context("invalid generated SVG viewBox")?;
        ensure!(
            dimensions.len() == 4 && dimensions[2] > 0.0 && dimensions[3] > 0.0,
            "invalid generated SVG dimensions"
        );
        let width = if dimensions[2] > 1000.0 {
            320.0
        } else if small {
            120.0
        } else {
            180.0
        };
        let caption_start = html
            .find(&format!("id=\"{label}\">"))
            .context("missing generated SVG caption")?
            + label.len()
            + 6;
        let caption_end = html[caption_start..]
            .find("</figcaption>")
            .context("missing generated figure caption end")?
            + caption_start;
        let caption = &html[caption_start..caption_end];
        let extra_style = if small && dimensions[2] <= 1000.0 {
            ".tick{font-size:16px}.axis-label{font-size:18px}"
        } else {
            ""
        };
        // HTML legends sit outside SVG for responsive reading. Preserve them
        // inside independent figures so a publication never loses populations.
        let after = &remaining[..remaining.find("</figure>").unwrap_or(0)];
        let mut legends = Vec::new();
        let mut tail = after;
        while let Some(start) = tail.find("<span class=\"legend ") {
            tail = &tail[start + 20..];
            let split = tail.find("\">").context("incomplete generated legend")?;
            let class = &tail[..split];
            tail = &tail[split + 2..];
            let end = tail
                .find("</span>")
                .context("incomplete generated legend label")?;
            let label = &tail[..end];
            let color = match class {
                "all" => "primary",
                "pass" => "passed",
                "qselected-key" => "qselected",
                value if value.starts_with("run") => value,
                _ => "primary",
            };
            legends.push((color, label));
            tail = &tail[end + 7..];
        }
        let legend_height = legends.len().div_ceil(3) * 24;
        let canvas_height = dimensions[3] + 40.0 + legend_height as f64;
        let height = width * canvas_height / dimensions[2];
        let original_viewbox = attribute(opening, "viewBox").unwrap();
        let opening = opening.replace(
            &format!("viewBox=\"{original_viewbox}\""),
            &format!("viewBox=\"0 0 {} {canvas_height}\"", dimensions[2]),
        );
        let mut legend_svg = String::new();
        for (i, (color, label)) in legends.iter().enumerate() {
            let x = 80 + i % 3 * 220;
            let y = dimensions[3] + 45.0 + (i / 3 * 24) as f64;
            write!(legend_svg, "<circle class=\"{color}\" cx=\"{x}\" cy=\"{}\" r=\"4\"/><text class=\"tick\" x=\"{}\" y=\"{y}\">{label}</text>", y - 4.0, x + 12).unwrap();
        }
        let svg = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{opening} xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}mm\" height=\"{height:.3}mm\" style=\"width:{width}mm;height:{height:.3}mm;min-width:0;max-width:none;font-family:Arial,sans-serif;background:white\"><title id=\"{label}\">{caption}</title><style><![CDATA[{}\n{extra_style}]]> </style><text x=\"20\" y=\"24\" style=\"font-size:16px;fill:#253047\">{caption}</text><g transform=\"translate(0 32)\">{}</g>{legend_svg}</svg>",
            css,
            &inline[opening_end + 1..inline.len() - 6]
        );
        emit(Figure {
            name: format!("{id}.svg"),
            svg,
        })?;
    }
    Ok(())
}

// SVG 1.1 renderers do not implement CSS custom properties. Resolve our fixed
// report palette to the same literals browsers compute, for every export format.
fn figure_style() -> String {
    let source = include_str!("report.css");
    let root = source.split_once('{').unwrap().1.split('}').next().unwrap();
    let mut css = source.to_owned();
    for line in root.lines() {
        if let Some((name, value)) = line.trim().split_once(':') {
            if name.starts_with("--") {
                css = css.replace(&format!("var({name})"), value.trim_end_matches(';'));
            }
        }
    }
    css
}

fn attribute<'a>(opening: &'a str, key: &str) -> Option<&'a str> {
    let start = opening.find(&format!("{key}=\""))? + key.len() + 2;
    let end = opening[start..].find('"')? + start;
    Some(&opening[start..end])
}

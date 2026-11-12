//! The input/output balance bar drawn under the map (the numbers come from `fsim-calc`).

use fsim_calc::Flow;

use crate::text::{self, Weight};

/// Unsigned rate per second: `2.5/s`, `12.3/s`, `130/s` (fewer decimal places for
/// larger numbers).
pub(crate) fn format_rate(rate: f64) -> String {
    let magnitude = rate.abs();
    let digits = if magnitude >= 100.0 {
        0
    } else if magnitude >= 10.0 {
        1
    } else {
        2
    };
    let mut number = format!("{magnitude:.digits$}");
    if number.contains('.') {
        number = number
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned();
    }
    format!("{number}/s")
}

/// `iron-plate` -> `Iron plate` (the dump has no localisation, so we prettify the id).
pub(crate) fn display_name(id: &str) -> String {
    let mut name = id.replace('-', " ");
    if let Some(first) = name.get(..1) {
        name = first.to_uppercase() + &name[1..];
    }
    name
}

/// What goes where on the bar (in bar pixels).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Item {
    /// Section header (`Input:` / `Output:`).
    Header { text: &'static str, x: f32, y: f32 },
    /// Row: icon, name and rate per second.
    Row {
        flow: usize,
        icon_x: f32,
        y: f32,
        name: String,
        name_x: f32,
        amount: String,
        amount_x: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Bar {
    pub width: u32,
    pub height: u32,
    pub font_px: f32,
    pub icon: f32,
    pub items: Vec<Item>,
}

/// Lays out the list: `Input:` followed by one row per input, then `Output:` and the output
/// rows. Columns (icon, name, rate) are shared by both sections. The bar is at least
/// as wide as `min_width`, and grows when the longest row is wider.
pub(crate) fn layout(flows: &[Flow], min_width: u32, scale: u32) -> Option<Bar> {
    if flows.is_empty() {
        return None;
    }
    let font_px = (scale as f32 * 0.5).max(9.0);
    let icon = scale.max(10) as f32;
    let margin = (scale as f32 / 4.0).max(3.0);
    let row_height = icon + margin / 2.0;
    let header_height = font_px * 1.5 + margin / 2.0;

    let names: Vec<String> = flows.iter().map(|f| display_name(&f.name)).collect();
    let amounts: Vec<String> = flows.iter().map(|f| format_rate(f.per_second)).collect();
    let widest = |texts: &[String]| {
        texts
            .iter()
            .map(|t| text::width(t, font_px, Weight::Regular))
            .fold(0.0, f32::max)
    };
    let icon_x = margin;
    let name_x = icon_x + icon + margin;
    let amount_x = name_x + widest(&names) + margin * 2.0;
    let content = amount_x + widest(&amounts) + margin;
    let width = (min_width as f32).max(content).ceil() as u32;

    let mut items = Vec::new();
    let mut y = margin;
    let inputs = flows.iter().take_while(|f| f.is_input()).count();
    for (header, range) in [("Input:", 0..inputs), ("Output:", inputs..flows.len())] {
        if range.is_empty() {
            continue;
        }
        items.push(Item::Header {
            text: header,
            x: margin,
            y,
        });
        y += header_height;
        for flow in range {
            items.push(Item::Row {
                flow,
                icon_x,
                y,
                name: names[flow].clone(),
                name_x,
                amount: amounts[flow].clone(),
                amount_x,
            });
            y += row_height;
        }
        y += margin / 2.0;
    }
    Some(Bar {
        width,
        height: (y + margin / 2.0).ceil() as u32,
        font_px,
        icon,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_are_formatted_compactly_per_second() {
        assert_eq!(format_rate(-2.0), "2/s");
        assert_eq!(format_rate(0.25), "0.25/s");
        assert_eq!(format_rate(-12.34), "12.3/s");
        assert_eq!(format_rate(130.4), "130/s");
        assert_eq!(format_rate(1.0 / 3.0), "0.33/s");
    }

    #[test]
    fn ids_become_readable_names() {
        assert_eq!(display_name("iron-gear-wheel"), "Iron gear wheel");
        assert_eq!(display_name("coal"), "Coal");
        assert_eq!(display_name(""), "");
    }

    fn flow(name: &str, rate: f64) -> Flow {
        Flow {
            name: name.into(),
            per_second: rate,
        }
    }

    fn headers(bar: &Bar) -> Vec<&'static str> {
        bar.items
            .iter()
            .filter_map(|i| match i {
                Item::Header { text, .. } => Some(*text),
                Item::Row { .. } => None,
            })
            .collect()
    }

    #[test]
    fn layout_lists_inputs_then_outputs_one_per_line() {
        let flows = [flow("a", -1.0), flow("b", -2.0), flow("c", 3.0)];
        assert!(layout(&[], 500, 32).is_none());
        let bar = layout(&flows, 100, 32).unwrap();
        assert_eq!(headers(&bar), ["Input:", "Output:"]);
        let order: Vec<_> = bar
            .items
            .iter()
            .map(|i| match i {
                Item::Header { text, .. } => (*text).to_owned(),
                Item::Row { name, .. } => name.clone(),
            })
            .collect();
        assert_eq!(order, ["Input:", "A", "B", "Output:", "C"]);
        let ys: Vec<f32> = bar
            .items
            .iter()
            .map(|i| match i {
                Item::Header { y, .. } | Item::Row { y, .. } => *y,
            })
            .collect();
        assert!(
            ys.windows(2).all(|w| w[0] < w[1]),
            "every line is below the previous one"
        );
        assert!(bar.height as f32 > *ys.last().unwrap() + bar.icon);
    }

    #[test]
    fn columns_are_shared_and_the_bar_widens_for_long_rows() {
        let flows = [flow("short", -1.0), flow("a-much-longer-item-name", 1.0)];
        let bar = layout(&flows, 50, 32).unwrap();
        let columns: Vec<(f32, f32)> = bar
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Row {
                    name_x, amount_x, ..
                } => Some((*name_x, *amount_x)),
                Item::Header { .. } => None,
            })
            .collect();
        assert_eq!(columns[0], columns[1]);
        assert!(
            bar.width as f32 > columns[0].1,
            "amounts fit inside the bar"
        );
        assert!(layout(&flows, 5000, 32).unwrap().width >= 5000);
    }

    #[test]
    fn missing_sections_have_no_header() {
        let only_outputs = layout(&[flow("a", 1.0)], 500, 32).unwrap();
        assert_eq!(headers(&only_outputs), ["Output:"]);
        let only_inputs = layout(&[flow("a", -1.0)], 500, 32).unwrap();
        assert_eq!(headers(&only_inputs), ["Input:"]);
    }
}

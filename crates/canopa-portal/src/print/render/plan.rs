use super::{Mode, Request};
use crate::print::{Orientation, Pages};

pub const MAX_PAGES: usize = 10_000;
const MAX_SHEETS: usize = 100_000;
const MM: f64 = 72.0 / 25.4;

#[derive(Clone, Copy, Debug)]
pub(super) struct Page {
    pub bounds: [f64; 4],
    pub rotate: i32,
    pub unit: f64,
}
impl Page {
    pub fn normalized(self) -> anyhow::Result<([f64; 2], Matrix)> {
        let [x0, y0, x1, y1] = self.bounds;
        anyhow::ensure!(
            self.bounds.iter().all(|v| v.is_finite())
                && x1 > x0
                && y1 > y0
                && self.unit.is_finite()
                && self.unit > 0.0
                && self.rotate % 90 == 0,
            "Invalid PDF page geometry"
        );
        let w = (x1 - x0) * self.unit;
        let h = (y1 - y0) * self.unit;
        anyhow::ensure!(
            w.is_finite() && h.is_finite() && w <= 100_000_000.0 && h <= 100_000_000.0,
            "PDF page exceeds limits"
        );
        let translated = Matrix([
            self.unit,
            0.,
            0.,
            self.unit,
            -x0 * self.unit,
            -y0 * self.unit,
        ]);
        anyhow::ensure!(
            translated.0.iter().all(|v| v.is_finite()),
            "PDF page transform exceeds limits"
        );
        let rotation = self.rotate.rem_euclid(360) / 90;
        let (size, matrix) = rotate([w, h], rotation);
        Ok((size, matrix.then(translated)))
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Matrix(pub [f64; 6]);
impl Matrix {
    /// self * right: apply right first, then self (PDF column-vector convention).
    pub fn then(self, right: Self) -> Self {
        let [a, b, c, d, e, f] = self.0;
        let [g, h, i, j, k, l] = right.0;
        Self([
            a * g + c * h,
            b * g + d * h,
            a * i + c * j,
            b * i + d * j,
            a * k + c * l + e,
            b * k + d * l + f,
        ])
    }
}
fn rotate([w, h]: [f64; 2], quarter: i32) -> ([f64; 2], Matrix) {
    match quarter.rem_euclid(4) {
        0 => ([w, h], Matrix([1., 0., 0., 1., 0., 0.])),
        1 => ([h, w], Matrix([0., -1., 1., 0., 0., w])),
        2 => ([w, h], Matrix([-1., 0., 0., -1., w, h])),
        _ => ([h, w], Matrix([0., 1., -1., 0., h, 0.])),
    }
}

pub(super) struct Sheet {
    pub size: [f64; 2],
    pub pages: Vec<usize>,
}
pub(super) struct Plan {
    pub sheets: Vec<Sheet>,
    pub order: Vec<usize>,
    columns: usize,
    rows: usize,
    area: [f64; 4],
}
impl Plan {
    pub fn new(request: &Request, pages: &[Page]) -> anyhow::Result<Self> {
        request.validate()?;
        anyhow::ensure!(
            !pages.is_empty() && pages.len() <= MAX_PAGES,
            "Print page count exceeds limits"
        );
        for page in pages {
            page.normalized()?;
        }
        if request.mode == Mode::Copies {
            let mut sheets = pages
                .iter()
                .enumerate()
                .map(|(index, page)| {
                    Ok(Sheet {
                        size: page.normalized()?.0,
                        pages: vec![index],
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            let selected = (0..sheets.len()).collect();
            let order = copy_order(request, &mut sheets, selected)?;
            return Ok(Self {
                sheets,
                order,
                columns: 1,
                rows: 1,
                area: [0.; 4],
            });
        }
        let source = pages[0].normalized()?.0;
        let mut size = [
            request.page_setup.width.map_or(source[0], |v| v * MM),
            request.page_setup.height.map_or(source[1], |v| v * MM),
        ];
        if matches!(
            request.layout.orientation,
            Orientation::Landscape | Orientation::ReverseLandscape
        ) {
            size.swap(0, 1);
        }
        let left = request.page_setup.left.unwrap_or(0.) * MM;
        let right = request.page_setup.right.unwrap_or(0.) * MM;
        let top = request.page_setup.top.unwrap_or(0.) * MM;
        let bottom = request.page_setup.bottom.unwrap_or(0.) * MM;
        let area = [left, bottom, size[0] - left - right, size[1] - top - bottom];
        anyhow::ensure!(
            area[2] > 0. && area[3] > 0.,
            "Print margins leave no page area"
        );
        let count = request.layout.number_up as usize;
        let mut best = (1, count, 0.0_f64);
        for columns in 1..=count {
            let rows = count.div_ceil(columns);
            let w = area[2] / columns as f64;
            let h = area[3] / rows as f64;
            let score = (w / source[0])
                .min(h / source[1])
                .max((w / source[1]).min(h / source[0]));
            if score > best.2 {
                best = (columns, rows, score);
            }
        }
        // Ranges describe a sequence, including repeated pages, rather than a
        // set. Preserve that sequence before imposition (as prepared clients do).
        let selected = if request.layout.pages == Pages::Ranges {
            request
                .layout
                .ranges
                .iter()
                .flat_map(|range| range.first as usize..=(range.last as usize).min(pages.len() - 1))
                .collect::<Vec<_>>()
        } else {
            (0..pages.len()).collect()
        };
        anyhow::ensure!(
            !selected.is_empty(),
            "No document pages match the approved ranges"
        );
        let mut sheets = selected
            .chunks(count)
            .map(|pages| Sheet {
                size,
                pages: pages.to_vec(),
            })
            .collect::<Vec<_>>();
        // Page-set and reverse apply to physical sheets after imposition, then
        // copies repeat sheets, not individual logical cells (GTK probe evidence).
        let mut selected = (0..sheets.len())
            .filter(|index| match request.layout.page_set.as_str() {
                "odd" => index % 2 == 0,
                "even" => index % 2 == 1,
                _ => true,
            })
            .collect::<Vec<_>>();
        if request.layout.reverse {
            selected.reverse();
        }
        let order = copy_order(request, &mut sheets, selected)?;
        Ok(Self {
            sheets,
            order,
            columns: best.0,
            rows: best.1,
            area,
        })
    }

    pub fn placement(
        &self,
        request: &Request,
        page: Page,
        slot: usize,
    ) -> anyhow::Result<([f64; 4], Matrix)> {
        if request.mode == Mode::Copies {
            anyhow::ensure!(slot == 0, "A prepared page cannot be imposed twice");
            let (size, matrix) = page.normalized()?;
            return Ok(([0., 0., size[0], size[1]], matrix));
        }
        let name = request.layout.number_up_layout.as_bytes();
        let horizontal_first = matches!(&name[..2], b"lr" | b"rl");
        let (x, y) = if horizontal_first {
            (slot % self.columns, slot / self.columns)
        } else {
            (slot / self.rows, slot % self.rows)
        };
        let horizontal = if horizontal_first {
            &name[..2]
        } else {
            &name[2..]
        };
        let vertical = if horizontal_first {
            &name[2..]
        } else {
            &name[..2]
        };
        let x = if horizontal == b"rl" {
            self.columns - 1 - x
        } else {
            x
        };
        let y = if vertical == b"tb" {
            self.rows - 1 - y
        } else {
            y
        };
        let [left, bottom, w, h] = self.area;
        let cell = [
            left + x as f64 * w / self.columns as f64,
            bottom + y as f64 * h / self.rows as f64,
            w / self.columns as f64,
            h / self.rows as f64,
        ];
        let (source, normalized) = page.normalized()?;
        let normal = (cell[2] / source[0]).min(cell[3] / source[1]);
        let rotated = (cell[2] / source[1]).min(cell[3] / source[0]);
        let mut quarter = i32::from(rotated > normal + 0.000000001);
        if matches!(
            request.layout.orientation,
            Orientation::ReversePortrait | Orientation::ReverseLandscape
        ) {
            quarter += 2;
        }
        let (display, rotation) = rotate(source, quarter);
        let fit = if request.layout.number_up == 1 {
            1.0
        } else {
            (cell[2] / display[0]).min(cell[3] / display[1])
        };
        let scale = fit * request.layout.scale / 100.;
        let matrix = Matrix([
            scale,
            0.,
            0.,
            scale,
            cell[0] + (cell[2] - display[0] * scale) / 2.,
            cell[1] + (cell[3] - display[1] * scale) / 2.,
        ])
        .then(rotation)
        .then(normalized);
        Ok((cell, matrix))
    }
}

fn copy_order(
    request: &Request,
    sheets: &mut Vec<Sheet>,
    mut selected: Vec<usize>,
) -> anyhow::Result<Vec<usize>> {
    // A copy starts on a fresh physical sheet. Group both sides before
    // uncollated repetition; pad an odd final front before repeating copies.
    if request.duplex && request.layout.copies > 1 && selected.len() % 2 == 1 {
        let last = *selected.last().expect("odd selection is nonempty");
        selected.push(sheets.len());
        sheets.push(Sheet {
            size: sheets[last].size,
            pages: Vec::new(),
        });
    }
    let output = selected
        .len()
        .checked_mul(request.layout.copies as usize)
        .ok_or_else(|| anyhow::anyhow!("Print sheet count overflow"))?;
    anyhow::ensure!(
        output > 0 && output <= MAX_SHEETS,
        "Print sheet count exceeds limits"
    );
    anyhow::ensure!(
        request.format != crate::print::Format::Svg || output == 1,
        "SVG output requires a single sheet"
    );
    let mut order = Vec::with_capacity(output);
    if request.layout.collate {
        for _ in 0..request.layout.copies {
            order.extend_from_slice(&selected);
        }
    } else {
        for group in selected.chunks(if request.duplex { 2 } else { 1 }) {
            for _ in 0..request.layout.copies {
                order.extend_from_slice(group);
            }
        }
    }
    Ok(order)
}

#[cfg(test)]
mod tests;

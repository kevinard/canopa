use super::*;
use crate::print::{Format, Layout, PageRange, PageSetup};
fn request() -> Request {
    Request {
        layout: Layout::default(),
        page_setup: PageSetup::default(),
        format: Format::Pdf,
        mode: Mode::Layout,
        duplex: false,
    }
}
fn pages() -> Vec<Page> {
    vec![
        Page {
            bounds: [0., 0., 595., 842.],
            rotate: 0,
            unit: 1.
        };
        4
    ]
}
fn sequence(plan: &Plan) -> Vec<usize> {
    plan.order
        .iter()
        .flat_map(|index| plan.sheets[*index].pages.iter().copied())
        .collect()
}

#[test]
fn physical_sheet_order_matches_measured_prepared_client_semantics() {
    let mut r = request();
    r.layout.number_up = 2;
    r.layout.page_set = "odd".into();
    assert_eq!(sequence(&Plan::new(&r, &pages()).unwrap()), [0, 1]);
    r.layout.reverse = true;
    assert_eq!(sequence(&Plan::new(&r, &pages()).unwrap()), [0, 1]);
    r.layout.page_set = "all".into();
    assert_eq!(sequence(&Plan::new(&r, &pages()).unwrap()), [2, 3, 0, 1]);
    r.layout.reverse = false;
    r.layout.copies = 2;
    r.layout.collate = false;
    assert_eq!(
        sequence(&Plan::new(&r, &pages()).unwrap()),
        [0, 1, 0, 1, 2, 3, 2, 3]
    );
    r.layout.collate = true;
    assert_eq!(
        sequence(&Plan::new(&r, &pages()).unwrap()),
        [0, 1, 2, 3, 0, 1, 2, 3]
    );
}
#[test]
fn ranges_are_zero_based_and_preserve_repeated_pages() {
    let mut r = request();
    r.layout.pages = Pages::Ranges;
    r.layout.ranges = vec![
        PageRange { first: 1, last: 2 },
        PageRange { first: 2, last: 3 },
    ];
    assert_eq!(sequence(&Plan::new(&r, &pages()).unwrap()), [1, 2, 2, 3]);
    r.layout.ranges = vec![PageRange {
        first: 99,
        last: 99,
    }];
    assert!(Plan::new(&r, &pages()).is_err());
}
#[test]
fn range_order_survives_imposition_page_set_reverse_and_duplex_copies() {
    let source = vec![pages()[0]; 7];
    let mut r = request();
    r.layout.pages = Pages::Ranges;
    r.layout.ranges = vec![
        PageRange { first: 4, last: 6 },
        PageRange { first: 1, last: 4 },
    ];
    assert_eq!(
        sequence(&Plan::new(&r, &source).unwrap()),
        [4, 5, 6, 1, 2, 3, 4]
    );
    r.layout.number_up = 2;
    r.layout.page_set = "even".into();
    r.layout.reverse = true;
    r.layout.copies = 2;
    r.duplex = true;
    let plan = Plan::new(&r, &source).unwrap();
    let sides: Vec<_> = plan
        .order
        .iter()
        .map(|index| plan.sheets[*index].pages.as_slice())
        .collect();
    assert_eq!(sides, [&[4][..], &[6, 1], &[4], &[6, 1]]);
}
#[test]
fn ranges_clip_to_the_document_without_reordering_valid_ranges() {
    let mut r = request();
    r.layout.pages = Pages::Ranges;
    r.layout.ranges = vec![
        PageRange {
            first: 99,
            last: 99,
        },
        PageRange {
            first: 2,
            last: 100,
        },
        PageRange { first: 0, last: 0 },
    ];
    assert_eq!(sequence(&Plan::new(&r, &pages()).unwrap()), [2, 3, 0]);
    r.layout.ranges = vec![PageRange {
        first: u32::MAX - 1,
        last: u32::MAX - 1,
    }];
    assert!(Plan::new(&r, &pages()).is_err());
    r.layout.pages = Pages::All;
    assert_eq!(sequence(&Plan::new(&r, &pages()).unwrap()), [0, 1, 2, 3]);
}
#[test]
fn repeated_ranges_still_obey_the_output_sheet_limit() {
    let mut r = request();
    r.layout.pages = Pages::Ranges;
    r.layout.ranges = vec![PageRange { first: 0, last: 3 }; 128];
    r.layout.copies = 196;
    assert!(Plan::new(&r, &pages()).is_err());
    r.layout.copies = 195;
    assert_eq!(Plan::new(&r, &pages()).unwrap().order.len(), 99840);
}
#[test]
fn every_supported_number_up_and_cell_order_stays_inside_the_printable_area() {
    for count in 1..=16 {
        for layout in [
            "lrtb", "lrbt", "rltb", "rlbt", "tblr", "tbrl", "btlr", "btrl",
        ] {
            let mut r = request();
            r.layout.number_up = count;
            r.layout.number_up_layout = layout.into();
            r.page_setup = PageSetup {
                width: Some(210.),
                height: Some(297.),
                left: Some(10.),
                right: Some(12.),
                top: Some(14.),
                bottom: Some(16.),
                ..Default::default()
            };
            let plan = Plan::new(&r, &pages()).unwrap();
            let mut cells = Vec::new();
            for slot in 0..count as usize {
                let (cell, matrix) = plan.placement(&r, pages()[0], slot).unwrap();
                assert!(cell[0] >= 10. * MM - 1e-8 && cell[1] >= 16. * MM - 1e-8);
                assert!(
                    cell[0] + cell[2] <= 198. * MM + 1e-8 && cell[1] + cell[3] <= 283. * MM + 1e-8
                );
                assert!(matrix.0.iter().all(|value| value.is_finite()));
                assert!(!cells.contains(&cell));
                cells.push(cell);
            }
        }
    }
}
#[test]
fn crop_origin_rotation_units_and_scale_are_composed_once() {
    let mut r = request();
    r.layout.scale = 75.;
    let page = Page {
        bounds: [10., 20., 210., 120.],
        rotate: 90,
        unit: 2.,
    };
    let (size, matrix) = page.normalized().unwrap();
    assert_eq!(size, [200., 400.]);
    fn point(m: Matrix, x: f64, y: f64) -> [f64; 2] {
        [
            m.0[0] * x + m.0[2] * y + m.0[4],
            m.0[1] * x + m.0[3] * y + m.0[5],
        ]
    }
    assert_eq!(point(matrix, 10., 20.), [0., 400.]);
    assert_eq!(point(matrix, 210., 120.), [200., 0.]);
    let plan = Plan::new(&r, &[page]).unwrap();
    let (_, m) = plan.placement(&r, page, 0).unwrap();
    assert!((m.0[2].abs() - 1.5).abs() < 1e-8);
}
#[test]
fn bounded_sheet_counts_and_single_sheet_svg_fail_before_output() {
    let mut r = request();
    r.format = Format::Svg;
    assert!(Plan::new(&r, &pages()).is_err());
    r.layout.number_up = 4;
    assert!(Plan::new(&r, &pages()).is_ok());
    r.format = Format::Pdf;
    r.layout.number_up = 1;
    r.layout.copies = 9999;
    assert_eq!(Plan::new(&r, &pages()).unwrap().order.len(), 39996);
    assert!(Plan::new(&r, &vec![pages()[0]; MAX_PAGES + 1]).is_err());
}

fn output_sides(plan: &Plan) -> Vec<Option<usize>> {
    plan.order
        .iter()
        .map(|index| plan.sheets[*index].pages.first().copied())
        .collect()
}

#[test]
fn duplex_copies_repeat_whole_sheets_and_pad_odd_copy_boundaries() {
    let source = &pages()[..3];
    for mode in [Mode::Layout, Mode::Copies] {
        let mut r = Request::copies(2, true, true, Format::Pdf);
        r.mode = mode;
        assert_eq!(
            output_sides(&Plan::new(&r, source).unwrap()),
            [
                Some(0),
                Some(1),
                Some(2),
                None,
                Some(0),
                Some(1),
                Some(2),
                None
            ]
        );
        r.layout.collate = false;
        assert_eq!(
            output_sides(&Plan::new(&r, source).unwrap()),
            [
                Some(0),
                Some(1),
                Some(0),
                Some(1),
                Some(2),
                None,
                Some(2),
                None
            ]
        );
        r.duplex = false;
        assert_eq!(
            output_sides(&Plan::new(&r, source).unwrap()),
            [Some(0), Some(0), Some(1), Some(1), Some(2), Some(2)]
        );
        r.duplex = true;
        r.layout.copies = 1;
        assert_eq!(
            output_sides(&Plan::new(&r, source).unwrap()),
            [Some(0), Some(1), Some(2)]
        );
    }
}

#[test]
fn duplex_copy_grouping_happens_after_number_up() {
    let mut r = request();
    r.layout.copies = 2;
    r.layout.number_up = 2;
    r.duplex = true;
    let plan = Plan::new(&r, &pages()).unwrap();
    assert_eq!(sequence(&plan), [0, 1, 2, 3, 0, 1, 2, 3]);
    r.layout.number_up = 4;
    let plan = Plan::new(&r, &pages()).unwrap();
    assert_eq!(plan.order.len(), 4);
    assert!(plan.sheets[plan.order[1]].pages.is_empty());
    assert!(plan.sheets[plan.order[3]].pages.is_empty());
}

#[test]
fn copy_only_preserves_each_page_geometry_and_refuses_a_second_layout() {
    let mut r = Request::copies(2, true, true, Format::Pdf);
    let source = [
        Page {
            bounds: [0., 0., 100., 200.],
            rotate: 0,
            unit: 1.,
        },
        Page {
            bounds: [0., 0., 300., 100.],
            rotate: 0,
            unit: 1.,
        },
        Page {
            bounds: [10., 20., 210., 120.],
            rotate: 90,
            unit: 2.,
        },
    ];
    let plan = Plan::new(&r, &source).unwrap();
    let sizes: Vec<_> = plan.order.iter().map(|i| plan.sheets[*i].size).collect();
    assert_eq!(
        sizes,
        [
            [100., 200.],
            [300., 100.],
            [200., 400.],
            [200., 400.],
            [100., 200.],
            [300., 100.],
            [200., 400.],
            [200., 400.]
        ]
    );
    let (clip, matrix) = plan.placement(&r, source[2], 0).unwrap();
    assert_eq!(clip, [0., 0., 200., 400.]);
    assert_eq!(matrix.0, [0., -2., 2., 0., -40., 420.]);
    r.layout.scale = 75.;
    assert!(Plan::new(&r, &source).is_err());
    r.layout.scale = 100.;
    r.page_setup.width = Some(210.);
    r.page_setup.height = Some(297.);
    assert!(Plan::new(&r, &source).is_err());
}

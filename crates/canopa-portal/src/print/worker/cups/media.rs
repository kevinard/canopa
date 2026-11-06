//! Exact concrete geometry, queried in the isolated worker and rechecked before
//! Create-Job. A range marker or a nearest-sheet result never grants a sheet.
use super::*;

const EXACT: u32 = 4;
const DUPLEX: u32 = 2;

pub(super) fn exact(
    selected: &Selected<'_>,
    size: CustomSize,
    sides: Option<Sides>,
) -> anyhow::Result<Media> {
    size.validate()?;
    let session = selected.session;
    let name = CString::new(size.name())?;
    let mut raw = RawMedia::default();
    let flags = EXACT
        | if matches!(sides, Some(Sides::LongEdge | Sides::ShortEdge)) {
            DUPLEX
        } else {
            0
        };
    // SAFETY: retained session/destination/info, bounded integer dimensions and
    // initialized public cups_size_t output. No call transfers ownership.
    let (supported, found) = unsafe {
        let http = session.http.as_ptr();
        let dest = selected.raw.as_ptr();
        let info = selected.info.expect("capabilities").as_ptr();
        let supported = (session.api.supported)(http, dest, info, c"media".as_ptr(), name.as_ptr());
        // Lazy database construction reuses CUPS's borrowed custom PWG record.
        // Finish it before resolving our name; a range-only destination can
        // legitimately have no enumerated sheets, so the count grants nothing.
        (session.api.media_count)(http, dest, info, flags);
        // The numeric lookup can normalize near-standard sizes even with EXACT.
        // The explicit name preserves the requested hundredths of a millimetre.
        (
            supported,
            (session.api.media_by_name)(http, dest, info, name.as_ptr(), flags, &mut raw),
        )
    };
    anyhow::ensure!(
        supported != 0
            && found != 0
            && raw.width == size.width as c_int
            && raw.height == size.height as c_int,
        "Printer does not support this exact custom paper size"
    );
    let media = Media {
        id: size.name(),
        width_mm: f64::from(raw.width) / 100.,
        height_mm: f64::from(raw.height) / 100.,
        top_mm: f64::from(raw.top) / 100.,
        bottom_mm: f64::from(raw.bottom) / 100.,
        left_mm: f64::from(raw.left) / 100.,
        right_mm: f64::from(raw.right) / 100.,
    };
    media.page_setup().validate()?;
    Ok(media)
}

pub(super) fn ppd_custom_name(name: &str) -> bool {
    name.get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("Custom."))
}

pub(super) fn preference(
    selected: &Selected<'_>,
    name: &str,
    sides: Option<Sides>,
) -> anyhow::Result<Media> {
    let ppd = ppd_custom_name(name);
    let name = CString::new(name)?;
    let mut raw = RawMedia::default();
    if ppd {
        // cupsGetDestMediaByName accepts PWG/legacy names, not the saved
        // Custom.WxHunit PPD spelling. Resolve that spelling with the public
        // parser, then independently authorize its concrete size below.
        // SAFETY: the string/library live through the call. Copy the two
        // dimensions before any later CUPS call can reuse its borrowed storage.
        let parsed = unsafe { (selected.session.api.media_for_ppd)(name.as_ptr()).as_ref() }
            .ok_or_else(|| anyhow::anyhow!("Saved custom paper is unavailable"))?;
        let (width, height) = (parsed.width, parsed.height);
        anyhow::ensure!(width > 0 && height > 0, "Saved custom paper is unavailable");
        return exact(
            selected,
            CustomSize {
                width: width as u32,
                height: height as u32,
            },
            sides,
        );
    }
    // SAFETY: live objects, retained NUL-terminated preference and initialized
    // public output. The subsequent exact lookup checks dimensions and margins.
    let found = unsafe {
        (selected.session.api.media_by_name)(
            selected.session.http.as_ptr(),
            selected.raw.as_ptr(),
            selected.info.expect("capabilities").as_ptr(),
            name.as_ptr(),
            EXACT,
            &mut raw,
        )
    };
    anyhow::ensure!(
        found != 0 && raw.width > 0 && raw.height > 0,
        "Saved custom paper is unavailable"
    );
    exact(
        selected,
        CustomSize {
            width: raw.width as u32,
            height: raw.height as u32,
        },
        sides,
    )
}

pub(in crate::print::worker) fn custom_media(
    destination: Destination,
    size: CustomSize,
    sides: Option<Sides>,
    cancel: &Cancellation,
) -> anyhow::Result<Media> {
    size.validate()?;
    cancel.check()?;
    let session = Session::open()?;
    let selected = session.destination(&destination, true)?;
    let media = exact(&selected, size, sides)?;
    cancel.check()?;
    Ok(media)
}

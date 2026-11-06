//! One validation path for the Iced draft and fresh pre-Create authorization.
use super::*;

pub(super) fn selected(
    selected: &Selected<'_>,
    options: &JobOptions,
) -> anyhow::Result<Vec<(CString, CString)>> {
    let mut values = options.values()?;
    let session = selected.session;
    let http = session.http.as_ptr();
    let dest = selected.raw.as_ptr();
    let info = selected.info.expect("capabilities").as_ptr();
    if let Some(approved) = &options.custom_media {
        let current = media::exact(selected, CustomSize::from_media(approved)?, options.sides)?;
        anyhow::ensure!(
            current == *approved,
            "Printer paper dimensions or margins changed since approval"
        );
    }
    for (name, value) in &values {
        // SAFETY: live CUPS objects and retained bounded option strings.
        anyhow::ensure!(
            unsafe { (session.api.supported)(http, dest, info, name.as_ptr(), value.as_ptr()) }
                != 0,
            "Printer no longer supports an approved option"
        );
    }
    let mut raw = option_array(&values);
    // SAFETY: public API, retained input array, no proposed extra option and no
    // output arrays requested. Never ask CUPS to resolve a conflict for us.
    let conflicts = unsafe {
        (session.api.conflicts)(
            http,
            dest,
            info,
            raw.len() as c_int,
            raw.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    anyhow::ensure!(
        conflicts == 0,
        "These printer settings cannot be used together"
    );
    if let Some(driver) = ppd::Driver::fetch(selected)? {
        values.extend(driver.validate(selected, options, &values)?);
    }
    Ok(values)
}

pub(in crate::print::worker) fn check_printer(
    destination: Destination,
    mut options: JobOptions,
    custom_size: Option<CustomSize>,
    cancel: &Cancellation,
) -> anyhow::Result<JobOptions> {
    cancel.check()?;
    options.values()?;
    let session = Session::open()?;
    let printer = session.destination(&destination, true)?;
    if let Some(size) = custom_size {
        anyhow::ensure!(
            options.media.is_none() && options.custom_media.is_none(),
            "Conflicting custom paper query"
        );
        options.custom_media = Some(media::exact(&printer, size, options.sides)?);
    }
    selected(&printer, &options)?;
    cancel.check()?;
    Ok(options)
}

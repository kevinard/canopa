//! A copied saved-container recipe and its limited original source claims.
use super::*;
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags, open, openat};
use sha2::{Digest, Sha256};
use std::os::fd::AsRawFd;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(in crate::operations) enum Phase {
    Ready,
    RootClaimed,
    FileClaimed,
}

pub(in crate::operations) struct Transition {
    before: Migration,
    after: Migration,
}
impl Transition {
    pub(in crate::operations) fn before(&self) -> &Migration {
        &self.before
    }
    pub(in crate::operations) fn after(&self) -> &Migration {
        &self.after
    }
    fn save(state: &mut Store, migration: &mut Migration, phase: Phase) -> io::Result<()> {
        let mut after = migration.clone();
        after.cleanup = Some(phase);
        state.saved_migration_phase(&Self {
            before: migration.clone(),
            after: after.clone(),
        })?;
        *migration = after;
        Ok(())
    }
}
fn recipe(m: &Migration) -> io::Result<[u8; 32]> {
    let bytes = serde_json::to_vec(&(
        m.id,
        m.ordinal,
        &m.source,
        &m.source_version,
        &m.source_fingerprint,
        &m.stage,
        m.copy_identity,
        &m.fingerprint,
        &m.context,
        &m.relocation,
    ))
    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut hash = Sha256::new();
    hash.update(b"canopa-files-saved-copy-v1\0");
    hash.update(bytes);
    Ok(hash.finalize().into())
}
fn movement(m: &Migration) -> io::Result<&Movement> {
    let Change::Rename {
        move_facts: None,
        original,
        current,
        identity,
        retained: None,
        reviewed: None,
        saved: Some(saved),
    } = &m.change
    else {
        return Err(attention("The saved copy needs its exact namespace step."));
    };
    if !m.forward
        || *identity != m.source.identity
        || *original != m.source.directory
        || current.parent() != m.stage.directory.parent()
        || m.source.identity.device == m.stage.identity.device
        || m.stage.sibling_payload
        || m.source.sibling_payload
        || m.stage.payload_identity != Some(m.copy_identity)
        || m.stage.replacement.is_some()
        || m.stage.entry_move.is_some()
        || m.stage.source_claim.is_some()
        || m.stage.directory_claim.is_some()
        || m.relocation.is_some()
    {
        return Err(attention(
            "The saved copy recipe has incompatible placement or roles.",
        ));
    }
    Ok(saved)
}
pub(in crate::operations) fn validate(m: &Migration) -> io::Result<()> {
    validate_change(&m.change)?;
    validate_stage(Some(&m.source), &m.source.directory)?;
    validate_stage(Some(&m.stage), m.destination())?;
    let saved = movement(m)?;
    let scope = saved
        .proof
        .as_ref()
        .ok_or_else(|| attention("The saved copy has no original source proof."))?;
    let binding = saved
        .copied
        .as_ref()
        .ok_or_else(|| attention("The saved copy has no original task recipe."))?;
    let target = m
        .stage
        .ownership
        .as_ref()
        .ok_or_else(|| attention("The saved copy has no original created target proof."))?;
    if m.cleanup.is_none()
        || (!m.catalogued && m.cleanup != Some(Phase::Ready))
        || scope.version.is_none()
        || saved.ownership(true).as_ref() != m.source.ownership.as_deref()
        || target
            .payload_origins()
            .is_none_or(|(parent, _)| parent != scope.destination_parent)
        || m.source.parent_identity != Some(saved.parent_identity)
        || saved
            .destination_parent
            .as_deref()
            .is_none_or(|p| Some(p.identity) != m.stage.parent_identity)
        || binding.recipe != recipe(m)?
        || binding.bytes != m.source_fingerprint.copied_bytes()
    {
        return Err(attention(
            "The saved copy differs from its original immutable recipe.",
        ));
    }
    Ok(())
}
pub(super) fn bind_prepared(
    m: &mut Migration,
    source: &fingerprint::source::Opened,
    copy: &staging::Prepared,
) -> io::Result<()> {
    let scope = movement(m)?
        .proof
        .as_ref()
        .ok_or_else(|| attention("The selected saved container has no original proof."))?;
    scope.directory.require(source.parent())?;
    scope.previous.require(source.entry())?;
    if !m.source_version.matches_opened(source.entry(), false)? {
        return Err(attention(
            "The selected previous version changed before binding.",
        ));
    }
    copy.require_current()?;
    let target = m
        .stage
        .ownership
        .as_ref()
        .ok_or_else(|| attention("The created saved copy has no original proof."))?;
    target.require_parent(copy.parent())?;
    target.require_directory(copy.directory())?;
    target.require_payload(copy.entry())?;
    scope.destination_parent.require(copy.parent())?;
    if copy.fingerprint(&Cancellation::default())? != m.fingerprint {
        return Err(attention("The created saved copy changed before binding."));
    }
    let binding = CopyBinding {
        recipe: recipe(m)?,
        bytes: m.source_fingerprint.copied_bytes(),
    };
    let Change::Rename {
        saved: Some(saved), ..
    } = &mut m.change
    else {
        unreachable!()
    };
    saved.copied = Some(binding);
    validate(m)
}
pub(in crate::operations) fn virtual_forward(m: &Migration) -> Pending {
    Pending::Forward {
        id: m.id,
        ordinal: m.ordinal,
        change: m.change.clone(),
        stage: Some(m.stage.clone()),
    }
}
pub(in crate::operations) fn claim_path(m: &Migration) -> PathBuf {
    m.source.directory.parent().unwrap().join(format!(
        ".canopa-files-source-claim-{}-{}-{}",
        m.id, m.ordinal, m.source.identity.inode
    ))
}
fn named(parent: &fs::File, name: impl rustix::path::Arg) -> io::Result<fs::File> {
    Ok(fs::File::from(openat(
        parent,
        name,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?))
}
fn optional(parent: &fs::File, name: impl rustix::path::Arg) -> io::Result<Option<fs::File>> {
    match named(parent, name) {
        Ok(file) => Ok(Some(file)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
fn parent(path: &Path, original: &original::Original) -> io::Result<fs::File> {
    let file = fs::File::from(open(
        path,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    original.require(&file)?;
    Ok(file)
}
fn sync(dir: &fs::File) -> io::Result<()> {
    fs::File::from(openat(
        dir,
        c".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?)
    .sync_all()
}
fn only(dir: &fs::File, allowed: &[&str]) -> io::Result<()> {
    if !dir.metadata()?.is_dir() {
        return Err(attention(
            "The original saved container is no longer a folder.",
        ));
    }
    for item in fs::read_dir(format!("/proc/self/fd/{}", dir.as_raw_fd()))? {
        let item = item?;
        if !allowed.iter().any(|name| item.file_name() == *name) {
            return Err(attention(
                "The saved claim contains additional work; it is preserved.",
            ));
        }
    }
    Ok(())
}
fn rename(
    a: &fs::File,
    name: impl rustix::path::Arg,
    b: &fs::File,
    target: impl rustix::path::Arg,
) -> io::Result<()> {
    rustix::fs::renameat_with(a, name, b, target, RenameFlags::NOREPLACE)?;
    sync(a)?;
    sync(b)
}
fn root(
    m: &Migration,
    parent: &fs::File,
    name: &std::ffi::OsStr,
    target: bool,
) -> io::Result<Option<fs::File>> {
    let Some(file) = optional(parent, name)? else {
        return Ok(None);
    };
    let ownership = if target {
        m.stage.ownership.as_ref()
    } else {
        m.source.ownership.as_ref()
    }
    .ok_or_else(|| attention("The saved root has no original proof."))?;
    ownership.require_directory(&file)?;
    let identity = if target {
        m.stage.identity
    } else {
        m.source.identity
    };
    if !file.metadata()?.is_dir() || Identity::from_metadata(&file.metadata()?) != identity {
        return Err(attention("The original saved folder changed."));
    }
    Ok(Some(file))
}
fn source_file(
    m: &Migration,
    dir: &fs::File,
    name: &std::ffi::CStr,
    moved: bool,
) -> io::Result<fs::File> {
    let file = named(dir, name)?;
    let scope = movement(m)?.proof.as_ref().unwrap();
    scope.previous.require(&file)?;
    if !m.source_version.matches_opened(&file, moved)?
        || Fingerprint::read_opened(&file, &Cancellation::default())? != m.source_fingerprint
    {
        return Err(attention(
            "The original saved version changed; both copies are preserved.",
        ));
    }
    Ok(file)
}
fn copied_file(m: &Migration, dir: &fs::File, name: &std::ffi::CStr) -> io::Result<fs::File> {
    let file = named(dir, name)?;
    m.stage.ownership.as_ref().unwrap().require_payload(&file)?;
    if Identity::from_metadata(&file.metadata()?) != m.copy_identity
        || Fingerprint::read_opened(&file, &Cancellation::default())? != m.fingerprint
    {
        return Err(attention(
            "The created saved version changed; source data is preserved.",
        ));
    }
    Ok(file)
}
fn current_parents(m: &Migration) -> io::Result<(fs::File, fs::File)> {
    let scope = movement(m)?.proof.as_ref().unwrap();
    Ok((
        parent(m.source.directory.parent().unwrap(), &scope.source_parent)?,
        parent(m.destination().parent().unwrap(), &scope.destination_parent)?,
    ))
}
fn finish_empty(
    state: &mut Store,
    m: &Migration,
    source_parent: &fs::File,
    claimed: &fs::File,
) -> io::Result<()> {
    only(claimed, &[])?;
    let scope = movement(m)?.proof.as_ref().unwrap();
    parent(m.source.directory.parent().unwrap(), &scope.source_parent)?;
    let claim = claim_path(m);
    let name = claim.file_name().unwrap();
    let current = root(m, source_parent, name, false)?
        .ok_or_else(|| attention("The claimed saved root disappeared before cleanup."))?;
    only(&current, &[])?;
    rustix::fs::unlinkat(source_parent, name, AtFlags::REMOVEDIR)?;
    sync(source_parent)?;
    at("root-removed", m);
    state.clear_intent()
}

#[cfg(test)]
fn at(point: &str, m: &Migration) {
    transfer::saved_cross_boundary::migration(point, m);
}
#[cfg(not(test))]
fn at(_point: &str, _m: &Migration) {}

pub(super) fn recover(state: &mut Store, original: &Migration) -> io::Result<()> {
    validate(original)?;
    state.require_saved_migration(original)?;
    let mut m = original.clone();
    at("prepared", &m);
    let scope = movement(&m)?.proof.as_ref().unwrap();
    let source_parent = parent(m.source.directory.parent().unwrap(), &scope.source_parent)?;
    if m.catalogued && m.cleanup != Some(Phase::Ready) {
        let claim = claim_path(&m);
        let Some(claimed) = root(&m, &source_parent, claim.file_name().unwrap(), false)? else {
            // A verified vacated claim never follows the old source name or
            // reopens a completed target whose namespace may have changed.
            return state.clear_intent();
        };
        if m.cleanup == Some(Phase::FileClaimed) && optional(&claimed, c"folder")?.is_none() {
            return finish_empty(state, &m, &source_parent, &claimed);
        }
    }
    let target_parent = parent(m.destination().parent().unwrap(), &scope.destination_parent)?;
    if !m.catalogued {
        let source = root(
            &m,
            &source_parent,
            m.source.directory.file_name().unwrap(),
            false,
        )?
        .ok_or_else(|| attention("The original saved source is unavailable."))?;
        only(&source, &["previous"])?;
        source_file(&m, &source, c"previous", false)?;
        let published = root(
            &m,
            &target_parent,
            m.destination().file_name().unwrap(),
            true,
        )?;
        let target = if let Some(target) = published {
            target
        } else {
            let work = root(
                &m,
                &target_parent,
                m.stage.directory.file_name().unwrap(),
                true,
            )?
            .ok_or_else(|| attention("The original created saved folder is unavailable."))?;
            if optional(&work, c"previous")?.is_none() {
                only(&work, &["folder"])?;
                copied_file(&m, &work, c"folder")?;
                rename(&work, c"folder", &work, c"previous")?;
            }
            at("normalized", &m);
            only(&work, &["previous"])?;
            copied_file(&m, &work, c"previous")?;
            current_parents(&m)?;
            rename(
                &target_parent,
                m.stage.directory.file_name().unwrap(),
                &target_parent,
                m.destination().file_name().unwrap(),
            )?;
            root(
                &m,
                &target_parent,
                m.destination().file_name().unwrap(),
                true,
            )?
            .ok_or_else(|| attention("The published saved root disappeared."))?
        };
        only(&target, &["previous"])?;
        let previous = copied_file(&m, &target, c"previous")?;
        source_file(&m, &source, c"previous", false)?;
        current_parents(&m)?;
        let version = fingerprint::Version::from_metadata(&previous.metadata()?);
        at("installed", &m);
        state.publish_saved_stage(&m, &version)?;
        m.catalogued = true;
        at("catalogued", &m);
    }
    state.require_saved_migration(&m)?;
    let target = root(
        &m,
        &target_parent,
        m.destination().file_name().unwrap(),
        true,
    )?
    .ok_or_else(|| attention("The original published saved root is unavailable."))?;
    copied_file(&m, &target, c"previous")?;
    let claim = claim_path(&m);
    let name = claim.file_name().unwrap().to_owned();
    if m.cleanup == Some(Phase::Ready) {
        let claimed = if let Some(claimed) = root(&m, &source_parent, &name, false)? {
            claimed
        } else {
            let source = root(
                &m,
                &source_parent,
                m.source.directory.file_name().unwrap(),
                false,
            )?
            .ok_or_else(|| {
                attention("The original saved source is unavailable before its claim.")
            })?;
            let version = movement(&m)?
                .proof
                .as_ref()
                .unwrap()
                .version
                .as_ref()
                .unwrap();
            if !version.matches_opened(&source, false)? {
                return Err(attention(
                    "The saved source namespace changed before cleanup.",
                ));
            }
            only(&source, &["previous"])?;
            source_file(&m, &source, c"previous", false)?;
            current_parents(&m)?;
            at("root-intent", &m);
            rename(
                &source_parent,
                m.source.directory.file_name().unwrap(),
                &source_parent,
                &name,
            )?;
            root(&m, &source_parent, &name, false)?
                .ok_or_else(|| attention("The claimed saved source disappeared."))?
        };
        let version = movement(&m)?
            .proof
            .as_ref()
            .unwrap()
            .version
            .as_ref()
            .unwrap();
        if !version.matches_opened(&claimed, true)? {
            return Err(attention(
                "The claimed saved root changed before verification.",
            ));
        }
        only(&claimed, &["previous"])?;
        source_file(&m, &claimed, c"previous", false)?;
        at("root-ready", &m);
        Transition::save(state, &mut m, Phase::RootClaimed)?;
        at("root-claimed", &m);
    }
    // Verified source-root claims never look at the old source name again.
    let Some(claimed) = root(&m, &source_parent, &name, false)? else {
        return state.clear_intent();
    };
    if m.cleanup == Some(Phase::RootClaimed) {
        if optional(&claimed, c"folder")?.is_none() {
            only(&claimed, &["previous"])?;
            source_file(&m, &claimed, c"previous", false)?;
            rename(&claimed, c"previous", &claimed, c"folder")?;
        }
        only(&claimed, &["folder"])?;
        source_file(&m, &claimed, c"folder", true)?;
        at("file-ready", &m);
        Transition::save(state, &mut m, Phase::FileClaimed)?;
        at("file-claimed", &m);
    }
    if let Some(file) = optional(&claimed, c"folder")? {
        movement(&m)?
            .proof
            .as_ref()
            .unwrap()
            .previous
            .require(&file)?;
        source_file(&m, &claimed, c"folder", true)?;
        copied_file(&m, &target, c"previous")?;
        current_parents(&m)?;
        rustix::fs::unlinkat(&claimed, c"folder", AtFlags::empty())?;
        sync(&claimed)?;
        at("unlinked", &m);
    }
    // No new previous entry may replace the already consumed claimed payload.
    finish_empty(state, &m, &source_parent, &claimed)
}

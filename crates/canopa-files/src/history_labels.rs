//! Operation history and Recovery records keep English labels
//! (`create folder Notes`, `copy to Photos`, `Undo rename a to b`). They are
//! stored data, written once in a fixed grammar; this module turns them into
//! the person's language when they are shown.

/// `label` in the current language. Unknown labels are returned unchanged.
pub fn display(label: &str) -> String {
    phrase(label).unwrap_or_else(|| label.to_owned())
}

/// `label` as a heading: the translated phrase with a capital first letter.
pub fn title(label: &str) -> String {
    capitalize(&display(label))
}

fn capitalize(text: &str) -> String {
    let mut characters = text.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

fn phrase(label: &str) -> Option<String> {
    let nested = |rest: &str| display(rest);
    if let Some(rest) = label.strip_prefix("Undo ") {
        return Some(fl!("files-history-undo", operation = nested(rest)));
    }
    if let Some(rest) = label.strip_prefix("Interrupted ") {
        return Some(fl!("files-history-interrupted", operation = nested(rest)));
    }
    for (prefix, kind) in [
        (
            "Preserved after finishing an unavailable saved version during Undo ",
            Preserved::UnavailableSaved,
        ),
        (
            "Preserved after finishing an unavailable item during Undo ",
            Preserved::UnavailableItem,
        ),
        (
            "Preserved by keeping an item during Undo ",
            Preserved::KeptItem,
        ),
        ("Preserved by Undo ", Preserved::Undo),
    ] {
        if let Some(rest) = label.strip_prefix(prefix) {
            let operation = nested(rest);
            return Some(match kind {
                Preserved::UnavailableSaved => {
                    fl!(
                        "files-history-preserved-unavailable-saved",
                        operation = operation
                    )
                }
                Preserved::UnavailableItem => {
                    fl!(
                        "files-history-preserved-unavailable-item",
                        operation = operation
                    )
                }
                Preserved::KeptItem => {
                    fl!("files-history-preserved-kept-item", operation = operation)
                }
                Preserved::Undo => fl!("files-history-preserved-undo", operation = operation),
            });
        }
    }
    match label {
        "operation" => return Some(fl!("files-history-operation")),
        "Restore folder permissions" => return Some(fl!("files-history-restore-permissions")),
        "Kept interrupted Undo work folder" => return Some(fl!("files-history-kept-undo-folder")),
        _ => {}
    }
    if let Some(name) = label.strip_prefix("create folder ") {
        return Some(fl!("files-history-create-folder", name = name.to_owned()));
    }
    if let Some(names) = label.strip_prefix("rename ")
        && let Some((source, destination)) = names.split_once(" to ")
    {
        return Some(fl!(
            "files-history-rename",
            source = source.to_owned(),
            destination = destination.to_owned()
        ));
    }
    if let Some(destination) = label.strip_prefix("copy to ") {
        return Some(fl!(
            "files-history-copy",
            destination = destination.to_owned()
        ));
    }
    if let Some(destination) = label.strip_prefix("move to ") {
        return Some(fl!(
            "files-history-move",
            destination = destination.to_owned()
        ));
    }
    if let Some(count) = label
        .strip_prefix("move ")
        .and_then(|rest| {
            rest.strip_suffix(" items to Trash")
                .or_else(|| rest.strip_suffix(" item to Trash"))
        })
        .and_then(|count| count.parse::<u64>().ok())
    {
        return Some(fl!("files-history-trash", count = count));
    }
    if let Some(name) = label.strip_prefix("restore ") {
        return Some(fl!("files-history-restore", name = name.to_owned()));
    }
    if let Some(name) = label.strip_prefix("recover ") {
        return Some(fl!("files-history-recover", name = name.to_owned()));
    }
    None
}

enum Preserved {
    UnavailableSaved,
    UnavailableItem,
    KeptItem,
    Undo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_labels_read_as_stored() {
        assert_eq!(display("copy to Photos"), "copy to Photos");
        assert_eq!(display("Undo rename a to b"), "Undo rename a to b");
        assert_eq!(title("create folder Notes"), "Create folder Notes");
        assert_eq!(display("move 1 item to Trash"), "move 1 item to Trash");
        assert_eq!(display("something new"), "something new");
    }

    #[test]
    fn french_labels_are_translated_for_display() {
        if !crate::i18n::tests::isolated(
            "history_labels::tests::french_labels_are_translated_for_display",
        ) {
            return;
        }
        let _french = crate::i18n::tests::Selected::french();
        assert_eq!(
            display("Undo copy to Photos"),
            "Annuler\u{a0}: copie vers Photos"
        );
        assert_eq!(
            title("move 3 items to Trash"),
            "Mise à la corbeille de 3 éléments"
        );
        assert_eq!(
            display("Interrupted rename a to b"),
            "Interrompu\u{a0}: renommage de a en b"
        );
    }
}

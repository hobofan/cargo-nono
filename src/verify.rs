use fallible_iterator::FallibleIterator;
use object::Object;
use std::collections::BTreeMap;
use std::error::Error;
use std::path::PathBuf;
use std::{borrow, fs};

fn die_entry_is_namespace<T: gimli::read::Reader>(
    dwarf: &gimli::Dwarf<T>,
    unit: &gimli::read::Unit<T>,
    entry: &gimli::read::DebuggingInformationEntry<T>,
    namespace_name: &str,
) -> bool {
    if entry.tag() != gimli::DW_TAG_namespace {
        return false;
    }

    for attr in entry.attrs().iterator() {
        let attr_name = attr.clone().unwrap().name().static_string().unwrap();
        match attr_name {
            "DW_AT_name" => {
                let raw_attrs_str = dwarf.attr_string(&unit, attr.unwrap().value()).unwrap();
                let value = raw_attrs_str.to_string().unwrap();
                // dbg!(&value);
                return value == namespace_name;
            }
            _ => {}
        }
    }

    false
}

/// Check wether a object file contains a specified namespace.
fn object_file_contains_namespace(
    object: &object::File,
    namespace_name: &str,
) -> Result<bool, gimli::Error> {
    let endian = if object.is_little_endian() {
        gimli::RunTimeEndian::Little
    } else {
        gimli::RunTimeEndian::Big
    };

    // Load a section and return as `Cow<[u8]>`.
    let load_section = |id: gimli::SectionId| -> Result<borrow::Cow<[u8]>, gimli::Error> {
        Ok(object
            .section_data_by_name(id.name())
            .unwrap_or(borrow::Cow::Borrowed(&[][..])))
    };
    // Load a supplementary section. We don't have a supplementary object file,
    // so always return an empty slice.
    let load_section_sup = |_| Ok(borrow::Cow::Borrowed(&[][..]));

    // Load all of the sections.
    let dwarf_cow = gimli::Dwarf::load(&load_section, &load_section_sup)?;

    // Borrow a `Cow<[u8]>` to create an `EndianSlice`.
    let borrow_section: &dyn for<'a> Fn(
        &'a borrow::Cow<[u8]>,
    ) -> gimli::EndianSlice<'a, gimli::RunTimeEndian> =
        &|section| gimli::EndianSlice::new(&*section, endian);

    // Create `EndianSlice`s for all of the sections.
    let dwarf = dwarf_cow.borrow(&borrow_section);

    // Iterate over the compilation units.
    let mut iter = dwarf.units();
    while let Some(header) = iter.next()? {
        let unit = dwarf.unit(header)?;

        // Iterate over the Debugging Information Entries (DIEs) in the unit.
        let mut entries = unit.entries();
        while let Some((_, entry)) = entries.next_dfs()? {
            if die_entry_is_namespace(&dwarf, &unit, &entry, namespace_name) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub fn rlib_contains_namespace(rlib_path: &PathBuf, namespace_name: &str) -> bool {
    let contents = fs::read(&rlib_path).unwrap();
    let archive = goblin::archive::Archive::parse(&contents).unwrap();

    for entry in archive.members() {
        let entry_path: PathBuf = entry.into();
        if entry_path.extension().map(|n| n.to_str().unwrap()) != Some("o") {
            continue;
        }
        // dbg!(entry);
        let entry_bytes = archive.extract(entry, &contents).unwrap();
        let object = object::File::parse(&entry_bytes).unwrap();
        if object_file_contains_namespace(&object, namespace_name).unwrap() {
            return true;
        }
    }

    false
}

/// Returns a list of incomplete paths (no file ending; should be extended with .rlib or .rmeta) of dependency files that are referenced in the specified .rmeta file
#[allow(dead_code)]
fn guess_dependencies_for_rmeta(
    rmeta_path: &PathBuf,
) -> Result<Vec<PathBuf>, Box<dyn Error + Send + Sync>> {
    let contents = std::fs::read(rmeta_path)?;
    let files_in_dir = rmeta_path
        .parent()
        .unwrap()
        .read_dir()?
        .into_iter()
        .filter(|path| {
            path.as_ref()
                .unwrap()
                .path()
                .extension()
                .map(|n| n.to_str().unwrap())
                == Some("rmeta")
        })
        .map(|n| n.unwrap().path())
        .collect::<Vec<_>>();
    let hashes: Vec<_> = files_in_dir
        .clone()
        .into_iter()
        .map(|path| {
            let filename_no_ext = path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned()
                .replace(".rmeta", "");
            let filename_hash = filename_no_ext.split("-").last().unwrap();
            filename_hash.to_owned().as_bytes().to_owned()
        })
        .collect();
    let hashes_to_files: BTreeMap<_, _> = hashes
        .clone()
        .into_iter()
        .zip(files_in_dir.clone())
        .collect();

    let contained_hashes = hashes
        .clone()
        .into_iter()
        .filter(|hash| twoway::find_bytes(&contents, hash).is_some())
        .collect::<Vec<_>>();

    Ok(contained_hashes
        .into_iter()
        .map(|hash| hashes_to_files.get(&hash).unwrap().to_owned())
        .collect())
}

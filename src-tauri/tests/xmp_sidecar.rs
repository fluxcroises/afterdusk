//! Prototype XMP : écriture d'un fichier annexe à côté d'un vrai RAW.

mod common;

use afterdusk_lib::metadata::xmp::{
    ColorLabel, Flag, SidecarData, SidecarNaming, read_sidecar, sidecar_path, write_sidecar,
};

/// Règle absolue : écrire le XMP ne modifie pas l'original.
#[test]
fn ecrire_le_xmp_ne_modifie_pas_l_original() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("DSCF0001.RAF");
    std::fs::copy(common::sample("fujifilm_x-t3.RAF"), &original).unwrap();
    let before = common::sha256(&original);
    let modified = common::modified(&original);

    for naming in [
        SidecarNaming::ReplaceExtension,
        SidecarNaming::AppendExtension,
    ] {
        let xmp = sidecar_path(&original, naming);
        let data = SidecarData {
            rating: 3,
            flag: Flag::Picked,
            label: Some(ColorLabel::Blue),
            keywords: vec!["test".into()],
            ..SidecarData::default()
        };
        write_sidecar(&xmp, &data).unwrap();
        assert_eq!(read_sidecar(&xmp).unwrap(), Some(data));
    }

    assert_eq!(common::sha256(&original), before);
    assert_eq!(common::modified(&original), modified);
    let mut names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["DSCF0001.RAF", "DSCF0001.RAF.xmp", "DSCF0001.xmp"]);
}

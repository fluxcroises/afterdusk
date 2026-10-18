//! Compile LibRaw (sous-module `LibRaw/`, LGPL 2.1) et la couche `shim/` en bibliothèque statique.
//!
//! La liste des sources reprend `LIB_OBJECTS` de `LibRaw/Makefile.dist`.

use std::path::Path;

const LIBRAW_SOURCES: &[&str] = &[
    "decoders/canon_600.cpp",
    "decoders/crx.cpp",
    "decoders/decoders_dcraw.cpp",
    "decoders/decoders_libraw.cpp",
    "decoders/decoders_libraw_dcrdefs.cpp",
    "decoders/dng.cpp",
    "decoders/fp_dng.cpp",
    "decoders/fuji_compressed.cpp",
    "decoders/generic.cpp",
    "decoders/kodak_decoders.cpp",
    "decoders/load_mfbacks.cpp",
    "decoders/olympus14.cpp",
    "decoders/pana8.cpp",
    "decoders/smal.cpp",
    "decoders/sonycc.cpp",
    "decoders/unpack.cpp",
    "decoders/unpack_thumb.cpp",
    "decompressors/losslessjpeg.cpp",
    "demosaic/aahd_demosaic.cpp",
    "demosaic/ahd_demosaic.cpp",
    "demosaic/dcb_demosaic.cpp",
    "demosaic/dht_demosaic.cpp",
    "demosaic/misc_demosaic.cpp",
    "demosaic/xtrans_demosaic.cpp",
    "integration/dngsdk_glue.cpp",
    "integration/rawspeed_glue.cpp",
    "libraw_c_api.cpp",
    "libraw_datastream.cpp",
    "metadata/adobepano.cpp",
    "metadata/canon.cpp",
    "metadata/ciff.cpp",
    "metadata/cr3_parser.cpp",
    "metadata/epson.cpp",
    "metadata/exif_gps.cpp",
    "metadata/fuji.cpp",
    "metadata/hasselblad_model.cpp",
    "metadata/identify.cpp",
    "metadata/identify_tools.cpp",
    "metadata/kodak.cpp",
    "metadata/leica.cpp",
    "metadata/makernotes.cpp",
    "metadata/mediumformat.cpp",
    "metadata/minolta.cpp",
    "metadata/misc_parsers.cpp",
    "metadata/nikon.cpp",
    "metadata/normalize_model.cpp",
    "metadata/olympus.cpp",
    "metadata/p1.cpp",
    "metadata/pentax.cpp",
    "metadata/samsung.cpp",
    "metadata/sony.cpp",
    "metadata/tiff.cpp",
    "postprocessing/aspect_ratio.cpp",
    "postprocessing/dcraw_process.cpp",
    "postprocessing/mem_image.cpp",
    "postprocessing/postprocessing_aux.cpp",
    "postprocessing/postprocessing_utils.cpp",
    "postprocessing/postprocessing_utils_dcrdefs.cpp",
    "preprocessing/ext_preprocess.cpp",
    "preprocessing/raw2image.cpp",
    "preprocessing/subtract_black.cpp",
    "tables/cameralist.cpp",
    "tables/colorconst.cpp",
    "tables/colordata.cpp",
    "tables/wblists.cpp",
    "utils/curves.cpp",
    "utils/decoder_info.cpp",
    "utils/init_close_utils.cpp",
    "utils/open.cpp",
    "utils/phaseone_processing.cpp",
    "utils/read_utils.cpp",
    "utils/thumb_utils.cpp",
    "utils/utils_dcraw.cpp",
    "utils/utils_libraw.cpp",
    "write/apply_profile.cpp",
    "write/file_write.cpp",
    "write/tiff_writer.cpp",
    "x3f/x3f_parse_process.cpp",
    "x3f/x3f_utils_patched.cpp",
];

fn main() {
    let root = Path::new("LibRaw");
    if !root.join("libraw/libraw.h").exists() {
        panic!("Sources LibRaw absentes : lancez `git submodule update --init --recursive`.");
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .include(root)
        .warnings(false)
        .define("LIBRAW_NODLL", None)
        .define("NO_JASPER", None)
        .define("NO_LCMS", None)
        .file("shim/afterdusk_raw.cpp");

    for source in LIBRAW_SOURCES {
        build.file(root.join("src").join(source));
    }

    if build.get_compiler().is_like_msvc() {
        build
            .flag("/EHsc")
            .flag("/utf-8")
            .define("_CRT_SECURE_NO_WARNINGS", None)
            .define("WIN32", None);
    } else {
        build.flag("-std=c++14");
    }

    build.compile("afterdusk_libraw");

    if target_os == "windows" {
        println!("cargo:rustc-link-lib=ws2_32");
    }
    println!("cargo:rerun-if-changed=shim");
    println!("cargo:rerun-if-changed=build.rs");
}

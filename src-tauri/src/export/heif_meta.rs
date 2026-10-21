//! Ajout de l'EXIF et du XMP à un fichier HEIF (ISO/IEC 23008-12) : deux
//! éléments, « Exif » et « mime » (application/rdf+xml), qui décrivent
//! l'image principale (références cdsc). Leurs données vont dans une boîte
//! mdat ajoutée en fin de fichier ; la boîte meta grandit, et les décalages
//! absolus de la table iloc qui la suivent sont repris d'autant.

use super::ExportError;

fn bad(what: &str) -> ExportError {
    ExportError::Encode(format!("HEIF : {what}"))
}

/// Première position de `needle` dans `hay`.
#[cfg(test)]
pub(crate) fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Une boîte : type, position dans son parent, octets complets et contenu.
struct Bx<'a> {
    kind: [u8; 4],
    start: usize,
    all: &'a [u8],
    body: &'a [u8],
}

fn boxes(data: &[u8]) -> Result<Vec<Bx<'_>>, ExportError> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        let head = data
            .get(pos..pos + 8)
            .ok_or_else(|| bad("boîte tronquée"))?;
        let size32 = u32::from_be_bytes([head[0], head[1], head[2], head[3]]);
        let kind = [head[4], head[5], head[6], head[7]];
        let (size, header) = match size32 {
            0 => (data.len() - pos, 8),
            1 => {
                let large = data
                    .get(pos + 8..pos + 16)
                    .ok_or_else(|| bad("boîte tronquée"))?;
                let large = u64::from_be_bytes(large.try_into().map_err(|_| bad("taille"))?);
                (usize::try_from(large).map_err(|_| bad("taille"))?, 16)
            }
            n => (n as usize, 8),
        };
        if size < header || pos + size > data.len() {
            return Err(bad("taille de boîte incohérente"));
        }
        out.push(Bx {
            kind,
            start: pos,
            all: &data[pos..pos + size],
            body: &data[pos + header..pos + size],
        });
        pos += size;
    }
    Ok(out)
}

/// Lecture séquentielle en gros-boutiste.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], ExportError> {
        let s = self
            .data
            .get(self.pos..self.pos + n)
            .ok_or_else(|| bad("table tronquée"))?;
        self.pos += n;
        Ok(s)
    }

    /// Entier non signé sur `n` octets (0, 1, 2, 4 ou 8).
    fn uint(&mut self, n: usize) -> Result<u64, ExportError> {
        Ok(self
            .take(n)?
            .iter()
            .fold(0u64, |acc, b| (acc << 8) | u64::from(*b)))
    }

    fn rest(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }
}

fn put(out: &mut Vec<u8>, value: u64, n: usize) -> Result<(), ExportError> {
    if n < 8 && value >> (8 * n) != 0 {
        return Err(bad("valeur trop grande pour la table"));
    }
    for i in (0..n).rev() {
        out.push((value >> (8 * i)) as u8);
    }
    Ok(())
}

fn full_box(kind: &[u8; 4], version: u8, flags: [u8; 3], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 12);
    out.extend_from_slice(&((payload.len() + 12) as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.push(version);
    out.extend_from_slice(&flags);
    out.extend_from_slice(payload);
    out
}

struct Extent {
    index: u64,
    offset: u64,
    length: u64,
}

struct Location {
    id: u32,
    method: u8,
    data_ref: u16,
    base: u64,
    extents: Vec<Extent>,
}

/// Table iloc lue, avec ses tailles de champs.
struct Iloc {
    version: u8,
    flags: [u8; 3],
    offset_size: usize,
    length_size: usize,
    base_size: usize,
    index_size: usize,
    items: Vec<Location>,
}

impl Iloc {
    fn parse(body: &[u8]) -> Result<Self, ExportError> {
        let mut r = Reader::new(body);
        let version = r.uint(1)? as u8;
        let flags: [u8; 3] = r.take(3)?.try_into().map_err(|_| bad("iloc"))?;
        let a = r.uint(1)? as usize;
        let b = r.uint(1)? as usize;
        let (offset_size, length_size, base_size) = (a >> 4, a & 15, b >> 4);
        let index_size = if version >= 1 { b & 15 } else { 0 };
        let count = r.uint(if version < 2 { 2 } else { 4 })?;
        let mut items = Vec::new();
        for _ in 0..count {
            let id = r.uint(if version < 2 { 2 } else { 4 })? as u32;
            let method = if version >= 1 {
                (r.uint(2)? & 15) as u8
            } else {
                0
            };
            let data_ref = r.uint(2)? as u16;
            let base = r.uint(base_size)?;
            let n = r.uint(2)?;
            let mut extents = Vec::new();
            for _ in 0..n {
                extents.push(Extent {
                    index: if version >= 1 { r.uint(index_size)? } else { 0 },
                    offset: r.uint(offset_size)?,
                    length: r.uint(length_size)?,
                });
            }
            items.push(Location {
                id,
                method,
                data_ref,
                base,
                extents,
            });
        }
        Ok(Self {
            version,
            flags,
            offset_size,
            length_size,
            base_size,
            index_size,
            items,
        })
    }

    fn write(&self) -> Result<Vec<u8>, ExportError> {
        let mut p = Vec::new();
        p.push(((self.offset_size << 4) | self.length_size) as u8);
        p.push(((self.base_size << 4) | self.index_size) as u8);
        let id_size = if self.version < 2 { 2 } else { 4 };
        put(&mut p, self.items.len() as u64, id_size)?;
        for item in &self.items {
            put(&mut p, u64::from(item.id), id_size)?;
            if self.version >= 1 {
                put(&mut p, u64::from(item.method), 2)?;
            }
            put(&mut p, u64::from(item.data_ref), 2)?;
            put(&mut p, item.base, self.base_size)?;
            put(&mut p, item.extents.len() as u64, 2)?;
            for e in &item.extents {
                if self.version >= 1 {
                    put(&mut p, e.index, self.index_size)?;
                }
                put(&mut p, e.offset, self.offset_size)?;
                put(&mut p, e.length, self.length_size)?;
            }
        }
        Ok(full_box(b"iloc", self.version, self.flags, &p))
    }
}

/// Élément de métadonnées à ajouter.
struct NewItem {
    id: u32,
    kind: [u8; 4],
    content_type: Option<&'static str>,
    payload: Vec<u8>,
}

fn infe(item: &NewItem) -> Vec<u8> {
    let wide = item.id > u32::from(u16::MAX);
    let mut p = Vec::new();
    if wide {
        p.extend_from_slice(&item.id.to_be_bytes());
    } else {
        p.extend_from_slice(&(item.id as u16).to_be_bytes());
    }
    p.extend_from_slice(&[0, 0]);
    p.extend_from_slice(&item.kind);
    p.push(0);
    if let Some(ct) = item.content_type {
        p.extend_from_slice(ct.as_bytes());
        p.push(0);
    }
    full_box(b"infe", if wide { 3 } else { 2 }, [0; 3], &p)
}

fn iinf(body: &[u8], added: &[NewItem]) -> Result<Vec<u8>, ExportError> {
    let mut r = Reader::new(body);
    let version = r.uint(1)? as u8;
    let flags: [u8; 3] = r.take(3)?.try_into().map_err(|_| bad("iinf"))?;
    let size = if version == 0 { 2 } else { 4 };
    let count = r.uint(size)?;
    let mut p = Vec::new();
    put(&mut p, count + added.len() as u64, size)?;
    p.extend_from_slice(r.rest());
    for item in added {
        p.extend_from_slice(&infe(item));
    }
    Ok(full_box(b"iinf", version, flags, &p))
}

/// Références cdsc des nouveaux éléments vers l'image principale.
fn iref(existing: Option<&[u8]>, added: &[NewItem], primary: u32) -> Result<Vec<u8>, ExportError> {
    let (version, flags, mut p) = match existing {
        Some(body) => {
            let mut r = Reader::new(body);
            let version = r.uint(1)? as u8;
            let flags: [u8; 3] = r.take(3)?.try_into().map_err(|_| bad("iref"))?;
            (version, flags, r.rest().to_vec())
        }
        None => {
            let wide = primary > u32::from(u16::MAX) || added.iter().any(|i| i.id > 0xffff);
            (u8::from(wide), [0; 3], Vec::new())
        }
    };
    let id_size = if version == 0 { 2 } else { 4 };
    for item in added {
        let mut r = Vec::new();
        put(&mut r, u64::from(item.id), id_size)?;
        put(&mut r, 1, 2)?;
        put(&mut r, u64::from(primary), id_size)?;
        p.extend_from_slice(&((r.len() + 8) as u32).to_be_bytes());
        p.extend_from_slice(b"cdsc");
        p.extend_from_slice(&r);
    }
    Ok(full_box(b"iref", version, flags, &p))
}

fn primary_id(body: &[u8]) -> Result<u32, ExportError> {
    let mut r = Reader::new(body);
    let version = r.uint(1)?;
    r.take(3)?;
    Ok(r.uint(if version == 0 { 2 } else { 4 })? as u32)
}

/// Boîte meta reconstruite, les données des nouveaux éléments commençant à
/// `data_start` dans le fichier, les anciens décalages au-delà de
/// `meta_end` étant repris de `delta`.
fn rebuild_meta(
    meta: &Bx<'_>,
    added: &[NewItem],
    primary: u32,
    meta_end: u64,
    delta: i64,
    data_start: u64,
) -> Result<Vec<u8>, ExportError> {
    let version_flags = meta.body.get(..4).ok_or_else(|| bad("meta"))?;
    let children = boxes(&meta.body[4..])?;
    let existing_iref = children.iter().find(|b| &b.kind == b"iref").map(|b| b.body);
    let shift = |v: u64| -> Result<u64, ExportError> {
        u64::try_from(v as i64 + delta).map_err(|_| bad("décalage"))
    };
    let mut body = version_flags.to_vec();
    for child in &children {
        match &child.kind {
            b"iinf" => body.extend_from_slice(&iinf(child.body, added)?),
            b"iref" => {}
            b"iloc" => {
                let mut table = Iloc::parse(child.body)?;
                for item in &mut table.items {
                    if item.method != 0 || item.data_ref != 0 {
                        continue;
                    }
                    if table.base_size > 0 && item.base >= meta_end {
                        item.base = shift(item.base)?;
                    } else {
                        for e in &mut item.extents {
                            if item.base + e.offset >= meta_end {
                                e.offset = shift(e.offset)?;
                            }
                        }
                    }
                }
                // Les nouveaux décalages sont absolus : au moins 4 octets.
                table.offset_size = table.offset_size.max(4);
                table.length_size = table.length_size.max(4);
                let mut offset = data_start;
                for item in added {
                    table.items.push(Location {
                        id: item.id,
                        method: 0,
                        data_ref: 0,
                        base: 0,
                        extents: vec![Extent {
                            index: 0,
                            offset,
                            length: item.payload.len() as u64,
                        }],
                    });
                    offset += item.payload.len() as u64;
                }
                body.extend_from_slice(&table.write()?);
                body.extend_from_slice(&iref(existing_iref, added, primary)?);
            }
            _ => body.extend_from_slice(child.all),
        }
    }
    let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    out.extend_from_slice(b"meta");
    out.extend_from_slice(&body);
    Ok(out)
}

/// Ajoute l'EXIF (bloc TIFF) et le XMP à `file`, un HEIF sans métadonnées.
pub fn embed(file: &[u8], exif: Option<&[u8]>, xmp: Option<&str>) -> Result<Vec<u8>, ExportError> {
    if exif.is_none() && xmp.is_none() {
        return Ok(file.to_vec());
    }
    let top = boxes(file)?;
    let meta = top
        .iter()
        .find(|b| &b.kind == b"meta")
        .ok_or_else(|| bad("boîte meta absente"))?;
    let children = boxes(meta.body.get(4..).ok_or_else(|| bad("meta"))?)?;
    let child = |k: &[u8; 4]| children.iter().find(|b| &b.kind == k);
    let primary = primary_id(child(b"pitm").ok_or_else(|| bad("pitm absent"))?.body)?;
    let iloc = Iloc::parse(child(b"iloc").ok_or_else(|| bad("iloc absent"))?.body)?;
    child(b"iinf").ok_or_else(|| bad("iinf absent"))?;
    let mut next = iloc
        .items
        .iter()
        .map(|i| i.id)
        .max()
        .unwrap_or(0)
        .max(primary)
        + 1;
    let mut added = Vec::new();
    if let Some(exif) = exif {
        // Décalage de l'en-tête TIFF après ce champ : 0, le bloc suit.
        let mut payload = 0u32.to_be_bytes().to_vec();
        payload.extend_from_slice(exif);
        added.push(NewItem {
            id: next,
            kind: *b"Exif",
            content_type: None,
            payload,
        });
        next += 1;
    }
    if let Some(xmp) = xmp {
        added.push(NewItem {
            id: next,
            kind: *b"mime",
            content_type: Some("application/rdf+xml"),
            payload: xmp.as_bytes().to_vec(),
        });
    }
    let meta_end = (meta.start + meta.all.len()) as u64;
    // La taille de la nouvelle meta ne dépend pas des valeurs des décalages.
    let draft = rebuild_meta(meta, &added, primary, meta_end, 0, 0)?;
    let delta = draft.len() as i64 - meta.all.len() as i64;
    let data_start = (file.len() as i64 + delta) as u64 + 8;
    let new_meta = rebuild_meta(meta, &added, primary, meta_end, delta, data_start)?;
    let mut out = Vec::with_capacity(file.len() + new_meta.len() + 64);
    for b in &top {
        if b.start == meta.start {
            out.extend_from_slice(&new_meta);
        } else {
            out.extend_from_slice(b.all);
        }
    }
    let payload_len: usize = added.iter().map(|i| i.payload.len()).sum();
    out.extend_from_slice(&((payload_len + 8) as u32).to_be_bytes());
    out.extend_from_slice(b"mdat");
    for item in &added {
        out.extend_from_slice(&item.payload);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// HEIF de 96 × 64 écrit par l'encodeur de Windows, sans métadonnées.
    const SAMPLE: &[u8] = include_bytes!("testdata/degrade.heic");

    /// Données de chaque élément (méthode 0), par identifiant et type.
    fn items(file: &[u8]) -> Vec<(u32, [u8; 4], Vec<u8>)> {
        let top = boxes(file).unwrap();
        let meta = top.iter().find(|b| &b.kind == b"meta").unwrap();
        let children = boxes(&meta.body[4..]).unwrap();
        let iloc = Iloc::parse(children.iter().find(|b| &b.kind == b"iloc").unwrap().body).unwrap();
        let iinf = children.iter().find(|b| &b.kind == b"iinf").unwrap();
        let skip = if iinf.body[0] == 0 { 6 } else { 8 };
        let kinds: Vec<(u32, [u8; 4])> = boxes(&iinf.body[skip..])
            .unwrap()
            .iter()
            .map(|e| {
                let wide = e.body[0] >= 3;
                let id = if wide {
                    u32::from_be_bytes(e.body[4..8].try_into().unwrap())
                } else {
                    u32::from(u16::from_be_bytes(e.body[4..6].try_into().unwrap()))
                };
                let at = if wide { 10 } else { 8 };
                (id, e.body[at..at + 4].try_into().unwrap())
            })
            .collect();
        iloc.items
            .iter()
            .map(|loc| {
                let data = loc
                    .extents
                    .iter()
                    .flat_map(|e| {
                        let s = (loc.base + e.offset) as usize;
                        file[s..s + e.length as usize].to_vec()
                    })
                    .collect();
                let kind = kinds.iter().find(|(id, _)| *id == loc.id).unwrap().1;
                (loc.id, kind, data)
            })
            .collect()
    }

    #[test]
    fn exif_et_xmp_ajoutes_sans_toucher_l_image() {
        let before = items(SAMPLE);
        let exif = b"II*\0\x08\0\0\0\0\0\0\0\0\0";
        let xmp = "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">Afterdusk</x:xmpmeta>";
        let out = embed(SAMPLE, Some(exif), Some(xmp)).unwrap();
        let after = items(&out);
        // Les éléments d'origine lisent exactement les mêmes octets.
        for item in &before {
            assert!(after.contains(item), "élément {} déplacé", item.0);
        }
        let exif_item = after.iter().find(|i| &i.1 == b"Exif").unwrap();
        assert_eq!(&exif_item.2[..4], &[0, 0, 0, 0]);
        assert_eq!(&exif_item.2[4..], exif);
        let xmp_item = after.iter().find(|i| &i.1 == b"mime").unwrap();
        assert_eq!(xmp_item.2, xmp.as_bytes());
        // Les deux décrivent l'image principale.
        let meta = boxes(&out)
            .unwrap()
            .into_iter()
            .find(|b| &b.kind == b"meta")
            .unwrap();
        let children = boxes(&meta.body[4..]).unwrap();
        let primary =
            primary_id(children.iter().find(|b| &b.kind == b"pitm").unwrap().body).unwrap();
        let iref = children.iter().find(|b| &b.kind == b"iref").unwrap();
        let refs = boxes(&iref.body[4..]).unwrap();
        for id in [exif_item.0, xmp_item.0] {
            assert!(refs.iter().any(|r| {
                &r.kind == b"cdsc"
                    && u32::from(u16::from_be_bytes(r.body[0..2].try_into().unwrap())) == id
                    && u32::from(u16::from_be_bytes(r.body[4..6].try_into().unwrap())) == primary
            }));
        }
    }

    #[test]
    fn sans_metadonnees_le_fichier_est_inchange() {
        assert_eq!(embed(SAMPLE, None, None).unwrap(), SAMPLE);
    }
}

//! A stress set of synthetic publications (#25): `cargo xtask synth OUT [--count N] [--seed S]`.
//!
//! Only Publisher writes real .pub files, so these come from our own writer, built from the format
//! notes in `crates/foliolum-pub/docs/pub-format` (its low-level records are the importer tests'
//! `tests/support`). They test robustness, not fidelity to Publisher, and don't count toward the
//! Phase 1 gate's real files.
//!
//! Each file is made from its seed alone, so the set is the same on every machine: one to eight
//! pages of one of five sizes, a master page with a footer on most, text boxes in columns with
//! headlines and body text in mixed fonts, sizes, bold, italic and colours, aligned and bulleted
//! paragraphs, stories linked across boxes and pages, text in Wingdings and Symbol, filled shapes
//! and PNG pictures. Beside each `<name>.pub` goes `truth/<name>.json`, the publication as written
//! in the shape of Publisher's layout export, so the gate checks completeness against it.
use std::path::Path;

use serde_json::{Value, json};

#[path = "../../crates/foliolum-pub/tests/support/mod.rs"]
mod support;
use support::{compound, list, number, pairs, rec, record};

const EMU: f64 = 12_700.0;

/// The fonts a file's text can name, in the text store's font list order.
const FONTS: &[&str] = &[
    "Times New Roman",
    "Arial",
    "Calibri",
    "Cambria",
    "Georgia",
    "Garamond",
    "Century Gothic",
    "Franklin Gothic Medium",
    "Comic Sans MS",
    "Parish Script",
    "Wingdings",
    "Symbol",
];
const WINGDINGS: usize = 10;
const SYMBOL: usize = 11;
const TEXT_FONTS: usize = 10;

/// Page sizes in points: US Letter, A4, Legal, half-letter, tabloid.
const SIZES: &[(f64, f64)] = &[(612.0, 792.0), (595.0, 842.0), (612.0, 1008.0), (396.0, 612.0), (792.0, 1224.0)];

/// Characters stored in a symbol font, and what the importer should turn them into.
const WINGDINGS_CHARS: &[(char, char)] = &[('X', '✠'), ('U', '🕆'), ('(', '🕿'), ('*', '✉'), ('þ', '☑'), ('l', '●'), ('«', '★'), ('V', '✞')];
const SYMBOL_CHARS: &[(char, char)] = &[('p', 'π'), ('a', 'α'), ('b', 'β'), ('W', 'Ω'), ('Ó', '©'), ('°', '°'), ('±', '±'), ('®', '→')];

const WORDS: &[&str] = &[
    "parish",
    "choir",
    "rehearsal",
    "Sunday",
    "evening",
    "breakfast",
    "volunteers",
    "welcome",
    "families",
    "school",
    "garden",
    "festival",
    "candle",
    "prayer",
    "community",
    "library",
    "concert",
    "bake",
    "sale",
    "youth",
    "group",
    "meeting",
    "hall",
    "coffee",
    "after",
    "Mass",
    "newsletter",
    "notice",
    "children",
    "parents",
    "teachers",
    "outreach",
    "pantry",
    "donations",
    "thanks",
    "everyone",
    "who",
    "helped",
    "with",
    "the",
    "and",
    "for",
    "our",
    "will",
    "be",
    "held",
    "in",
    "on",
    "at",
    "this",
    "week",
    "next",
    "month",
    "please",
    "join",
    "us",
];

/// A small deterministic generator (xorshift64*), so the set needs no dependency and never changes.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + self.below(hi - lo + 1)
    }
    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

#[derive(Clone)]
struct Run {
    /// As stored in the file.
    text: String,
    /// As the importer should read it (symbol-font characters mapped to Unicode).
    expected: String,
    font: usize,
    size: f64,
    bold: bool,
    italic: bool,
    /// Index into the text colour table, or none for the default (black).
    color: Option<usize>,
}

struct Para {
    runs: Vec<Run>,
    /// 0 left, 1 right, 2 centre, 6 justified.
    align: u32,
    bullet: bool,
    space_after: f64,
}

struct Story {
    id: u32,
    paras: Vec<Para>,
}

enum Kind {
    /// A box showing `story`, the `link`-th box of its chain.
    Text { story: usize, link: u32 },
    /// Rectangle (1) or ellipse (3), filled with `fill` (RGB), with a line of `line` pt or none.
    Shape { kind: u16, fill: u32, line: Option<f64> },
    /// A picture frame showing picture `blip` (1-based).
    Picture { blip: u32 },
}

struct Object {
    rect: [f64; 4],
    kind: Kind,
}

/// One publication as the generator plans it.
struct Plan {
    width: f64,
    height: f64,
    pages: Vec<Vec<Object>>,
    master: Option<Vec<Object>>,
    stories: Vec<Story>,
    pictures: Vec<Vec<u8>>,
    colors: Vec<u32>,
}

fn words(r: &mut Rng, n: usize) -> String {
    (0..n).map(|_| *r.pick(WORDS)).collect::<Vec<_>>().join(" ")
}

fn sentence(r: &mut Rng) -> String {
    let n = r.range(6, 16);
    let mut s = words(r, n);
    if let Some(first) = s.get(..1) {
        s = first.to_uppercase() + &s[1..];
    }
    s + "."
}

fn plain(text: String, font: usize, size: f64) -> Run {
    Run { expected: text.clone(), text, font, size, bold: false, italic: false, color: None }
}

/// A run of characters in a symbol font, with what they should become.
fn symbols(r: &mut Rng, font: usize, size: f64) -> Run {
    let table = if font == WINGDINGS { WINGDINGS_CHARS } else { SYMBOL_CHARS };
    let picks: Vec<(char, char)> = (0..r.range(1, 4)).map(|_| *r.pick(table)).collect();
    Run {
        text: picks.iter().map(|p| p.0).collect(),
        expected: picks.iter().map(|p| p.1).collect(),
        font,
        size,
        bold: false,
        italic: false,
        color: None,
    }
}

fn headline(r: &mut Rng, colors: usize) -> Story {
    let n = r.range(2, 6);
    let mut run = plain(words(r, n), r.below(TEXT_FONTS), [18.0, 24.0, 28.0, 36.0][r.below(4)]);
    run.bold = r.chance(70);
    run.color = Some(r.below(colors));
    if let Some(first) = run.text.get(..1) {
        run.text = first.to_uppercase() + &run.text[1..];
        run.expected = run.text.clone();
    }
    Story { id: 0, paras: vec![Para { runs: vec![run], align: *r.pick(&[0, 1, 2]), bullet: false, space_after: 0.0 }] }
}

/// Body text for a box `w` × `h` pt: about as much as fits, and about one box in ten overfull
/// (overset), as real files have.
fn body(r: &mut Rng, colors: usize, (w, h): (f64, f64), symbol_text: bool) -> Story {
    let font = r.below(TEXT_FONTS);
    let size = [9.0, 10.0, 11.0, 12.0][r.below(4)];
    let bullets = r.chance(25);
    // Characters that fit: about half an em per character, lines 1.2 em apart.
    let fits = ((w / (0.5 * size)) * (h / (1.2 * size))) as usize;
    let budget = if r.chance(10) { fits * 2 } else { fits * 6 / 10 };
    let mut used = 0;
    let mut paras = Vec::new();
    while used < budget.max(40) {
        paras.push({
            let mut runs = vec![plain(sentence(r), font, size)];
            for _ in 0..r.range(0, 3) {
                let mut run = plain(format!(" {}", sentence(r)), if r.chance(20) { r.below(TEXT_FONTS) } else { font }, size);
                run.bold = r.chance(30);
                run.italic = r.chance(30);
                run.color = r.chance(30).then(|| r.below(colors));
                if r.chance(20) {
                    run.size = size + 2.0;
                }
                runs.push(run);
            }
            if symbol_text && r.chance(50) {
                runs.push(plain(" ".into(), font, size));
                let symbol_font = if r.chance(50) { WINGDINGS } else { SYMBOL };
                runs.push(symbols(r, symbol_font, size));
                runs.push(plain(format!(" {}", words(r, 3)), font, size));
            }
            used += runs.iter().map(|r| r.text.chars().count()).sum::<usize>() + 12;
            Para { runs, align: *r.pick(&[0, 0, 6, 2]), bullet: bullets, space_after: [0.0, 6.0][r.below(2)] }
        });
    }
    Story { id: 0, paras }
}

/// A small PNG: a two-colour pattern, so pictures differ and decode for real.
fn png(r: &mut Rng) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut c = !0u32;
        for b in data {
            c ^= u32::from(*b);
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let mut body = tag.to_vec();
        body.extend(data);
        out.extend(&body);
        out.extend(crc32(&body).to_be_bytes());
    }
    let (w, h) = (r.range(24, 64) as u32, r.range(24, 64) as u32);
    let (a, b) = ([r.below(256) as u8, r.below(256) as u8, r.below(256) as u8], [r.below(256) as u8, r.below(256) as u8, r.below(256) as u8]);
    let step = r.range(3, 9) as u32;
    let mut raw = Vec::new();
    for y in 0..h {
        raw.push(0); // filter: none
        for x in 0..w {
            raw.extend(if (x / step + y / step).is_multiple_of(2) { a } else { b });
        }
    }
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = w.to_be_bytes().to_vec();
    ihdr.extend(h.to_be_bytes());
    ihdr.extend([8, 2, 0, 0, 0]); // 8-bit RGB
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Plans publication `n` of the set from `seed`.
fn plan(seed: u64, n: usize) -> Plan {
    let r = &mut Rng::new(seed.wrapping_add(n as u64 * 7919));
    let (width, height) = *r.pick(SIZES);
    let colors: Vec<u32> = (0..r.range(2, 5)).map(|_| (r.next() & 0x00FF_FFFF) as u32).collect();
    let mut stories = Vec::new();
    let mut pictures = Vec::new();
    let symbol_text = r.chance(50);
    let margin = 36.0;
    let master = r.chance(70).then(|| {
        let mut footer = Story { id: 0, paras: vec![Para { runs: vec![plain(words(r, 4), 1, 8.0)], align: 2, bullet: false, space_after: 0.0 }] };
        footer.paras[0].runs[0].italic = true;
        stories.push(footer);
        vec![
            Object { rect: [margin, height - margin - 18.0, width - 2.0 * margin, 18.0], kind: Kind::Text { story: stories.len() - 1, link: 0 } },
            Object { rect: [margin, height - margin - 22.0, width - 2.0 * margin, 2.0], kind: Kind::Shape { kind: 1, fill: colors[0], line: None } },
        ]
    });
    let page_count = r.range(1, 8);
    let mut pages: Vec<Vec<Object>> = (0..page_count).map(|_| Vec::new()).collect();
    // A story that runs across boxes on consecutive pages, linked.
    let linked = (page_count > 1 && r.chance(60)).then(|| {
        // Two boxes of about a third of the page's width.
        stories.push(body(r, colors.len(), (width / 3.0, 2.0 * 160.0), symbol_text));
        (stories.len() - 1, r.below(page_count - 1))
    });
    let mut link = 0;
    for (p, objects) in pages.iter_mut().enumerate() {
        let columns = r.range(1, if width > 500.0 { 3 } else { 2 });
        let gutter = 12.0;
        let col_w = (width - 2.0 * margin - gutter * (columns as f64 - 1.0)) / columns as f64;
        let top = margin;
        let bottom = height - margin - 30.0;
        // Headline across the top.
        stories.push(headline(r, colors.len()));
        objects.push(Object { rect: [margin, top, width - 2.0 * margin, 48.0], kind: Kind::Text { story: stories.len() - 1, link: 0 } });
        if r.chance(50) {
            objects.push(Object {
                rect: [margin, top + 52.0, width - 2.0 * margin, 3.0],
                kind: Kind::Shape { kind: 1, fill: *r.pick(&colors), line: None },
            });
        }
        for c in 0..columns {
            let x = margin + c as f64 * (col_w + gutter);
            let mut y = top + 64.0;
            while y < bottom - 60.0 {
                let h = (r.range(60, 220) as f64).min(bottom - y);
                let roll = r.below(100);
                let rect = [x, y, col_w, h];
                if let Some((story, start)) = linked
                    && (p == start || p == start + 1)
                    && c == 0
                    && y == top + 64.0
                {
                    objects.push(Object { rect, kind: Kind::Text { story, link } });
                    link += 1;
                } else if roll < 55 {
                    stories.push(body(r, colors.len(), (col_w, h), symbol_text));
                    objects.push(Object { rect, kind: Kind::Text { story: stories.len() - 1, link: 0 } });
                } else if roll < 75 {
                    pictures.push(png(r));
                    objects.push(Object { rect, kind: Kind::Picture { blip: pictures.len() as u32 } });
                } else {
                    let line = r.chance(50).then(|| [0.75, 1.5, 3.0][r.below(3)]);
                    objects.push(Object { rect, kind: Kind::Shape { kind: *r.pick(&[1, 3]), fill: *r.pick(&colors), line } });
                }
                y += h + 10.0;
            }
        }
    }
    for (i, s) in stories.iter_mut().enumerate() {
        s.id = i as u32 + 2;
    }
    Plan { width, height, pages, master, stories, pictures, colors }
}

/// Every object in Contents order: master objects first, then each page's.
fn all_objects(p: &Plan) -> Vec<&Object> {
    p.master.iter().flatten().chain(p.pages.iter().flatten()).collect()
}

/// The `Contents` stream: the document, an optional master page, the pages, then the objects, with
/// a record directory whose first 256 entries are empty (contents-stream.md).
fn contents(p: &Plan) -> Vec<u8> {
    let first = 256u32;
    let entry = |seq: u32| {
        let mut e = vec![0x00, 0x70];
        e.extend(seq.to_le_bytes());
        e
    };
    let label = |text: &str| {
        let mut b = vec![0x0e, 0xc0];
        let units: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        b.extend(((units.len() + 4) as u32).to_le_bytes());
        b.extend(units);
        b
    };
    let objects = all_objects(p);
    let master_seq = p.master.as_ref().map(|_| first + 1);
    let first_page = first + 1 + u32::from(master_seq.is_some());
    let first_object = first_page + p.pages.len() as u32;
    let page_seqs: Vec<u32> = master_seq.into_iter().chain((0..p.pages.len() as u32).map(|i| first_page + i)).collect();
    let document = [
        list(0x02, 0xa0, &page_seqs.iter().flat_map(|s| entry(*s)).collect::<Vec<_>>()),
        list(0x12, 0x88, &[number(0x01, (p.width * EMU) as u32), number(0x02, (p.height * EMU) as u32)].concat()),
    ]
    .concat();
    let mut records: Vec<(u16, Vec<u8>)> = vec![(0x44, document)];
    let mut next = first_object;
    let mut page_record = |count: usize, master: Option<u32>, name: &str| {
        let seqs: Vec<u8> = (next..next + count as u32).flat_map(entry).collect();
        next += count as u32;
        let mut r = if count > 0 { list(0x02, 0xa0, &seqs) } else { Vec::new() };
        if let Some(m) = master {
            r.extend(number(0x0d, m));
        }
        r.extend(label(name));
        r
    };
    if let Some(m) = &p.master {
        records.push((0x43, page_record(m.len(), None, "A")));
    }
    for page in &p.pages {
        records.push((0x43, page_record(page.len(), master_seq, "")));
    }
    for o in &objects {
        let props = match o.kind {
            Kind::Text { story, link } => {
                let mut v = number(0x27, p.stories[story].id);
                if link > 0 {
                    v.extend(number(0x28, link));
                }
                v
            }
            _ => Vec::new(),
        };
        records.push((0x01, props));
    }

    let mut b = vec![0u8; 0x1e];
    b[..8].copy_from_slice(&[0xe8, 0xac, 0x2c, 0x00, 0xe8, 0x03, 0x0a, 0x00]);
    b[0x0c..0x10].copy_from_slice(&[0x1a, 0x00, 0x00, 0x01]);
    let mut directory = [0x00, 0x78].repeat(first as usize);
    for (ty, props) in &records {
        let at = b.len() as u32;
        b.extend(record(props));
        let mut fields = vec![0x02, 0x18];
        fields.extend(ty.to_le_bytes());
        fields.extend([0x04, 0xb8]);
        fields.extend(at.to_le_bytes());
        directory.extend(list(0x00, 0x88, &fields));
    }
    let entries = first + records.len() as u32;
    let trailer = [number(0x01, entries), number(0x02, entries - 1), list(0x03, 0x90, &directory)].concat();
    let at = b.len() as u32;
    b[0x1a..0x1e].copy_from_slice(&at.to_le_bytes());
    b.extend(record(&trailer));
    b
}

/// The drawing stream: the picture store, then one drawing holding every object's shape, each
/// tied to its `Contents` record by sequence number.
fn escher(p: &Plan) -> Vec<u8> {
    let objects = all_objects(p);
    let first_object = 256 + 1 + u32::from(p.master.is_some()) + p.pages.len() as u32;
    let mut shapes = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        let spid = 1025 + i as u32;
        let (kind, props): (u16, Vec<(u16, u32)>) = match o.kind {
            Kind::Text { .. } => (202, Vec::new()),
            Kind::Shape { kind, fill, line } => {
                let mut props = vec![(0x0181, red_lowest(fill))];
                match line {
                    Some(w) => props.extend([(0x01c0, 0), (0x01cb, (w * EMU) as u32), (0x01ff, 0x0008_0008)]),
                    None => props.push((0x01ff, 0x0008_0000)),
                }
                (kind, props)
            }
            Kind::Picture { blip } => (75, vec![(0x4104, blip)]),
        };
        let mut sp = rec(2, kind, 0xF00A, &[spid.to_le_bytes(), 0xA00u32.to_le_bytes()].concat());
        if !props.is_empty() {
            let data: Vec<u8> = props.iter().flat_map(|(id, v)| [id.to_le_bytes().to_vec(), v.to_le_bytes().to_vec()].concat()).collect();
            sp.extend(rec(3, props.len() as u16, 0xF00B, &data));
        }
        let [x, y, w, h] = o.rect;
        let (cx, cy) = (p.width / 2.0, p.height / 2.0);
        let e = |v: f64| (v * EMU).round() as i32;
        sp.extend(rec(0, 0, 0xF010, &pairs(&[(0x2001, e(x - cx)), (0x2002, e(y - cy)), (0x2003, e(x + w - cx)), (0x2004, e(y + h - cy))])));
        sp.extend(rec(0, 0, 0xF011, &pairs(&[(0x6801, (first_object + i as u32) as i32)])));
        shapes.push(rec(0xF, 0, 0xF004, &sp));
    }
    let mut store = Vec::new();
    for png in &p.pictures {
        let mut blip = vec![0u8; 16]; // UID
        blip.push(0xFF); // tag
        blip.extend(png);
        let blip = rec(0, 0x6E0, 0xF01E, &blip);
        let mut bse = vec![6u8, 6]; // PNG, PNG
        bse.extend([0u8; 16]);
        bse.extend(0xFFu16.to_le_bytes());
        bse.extend((blip.len() as u32).to_le_bytes());
        bse.extend(1u32.to_le_bytes());
        bse.extend(0u32.to_le_bytes());
        bse.extend([0u8; 4]);
        bse.extend(blip);
        store.extend(rec(2, 6, 0xF007, &bse));
    }
    let dgg = (!p.pictures.is_empty()).then(|| {
        let mut dgg = rec(0, 0, 0xF006, &[0u8; 16]);
        dgg.extend(rec(0xF, p.pictures.len() as u16, 0xF001, &store));
        dgg
    });
    support::escher(dgg.as_deref(), &shapes)
}

/// Writes the format pages (512 bytes each) for runs ending at `ends` (stream byte offsets), each
/// with its format record.
fn format_pages(runs: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < runs.len() {
        // Take as many runs as fit: 8 header bytes, 6 per run, and the records at the end.
        let mut n = 0;
        let mut records = 0;
        while i + n < runs.len() {
            let need = 8 + 6 * (n + 1) + records + runs[i + n].1.len() + 4;
            if need > 512 || n == 120 {
                break;
            }
            records += runs[i + n].1.len() + 4;
            n += 1;
        }
        let mut page = vec![0u8; 512];
        page[..2].copy_from_slice(&(n as u16).to_le_bytes());
        page[2..4].copy_from_slice(&1u16.to_le_bytes());
        let mut at = 512;
        for k in 0..n {
            let (end, props) = &runs[i + k];
            page[8 + 4 * k..12 + 4 * k].copy_from_slice(&end.to_le_bytes());
            let rec = record(props);
            at -= rec.len();
            page[at..at + rec.len()].copy_from_slice(&rec);
            let slot = 8 + 4 * n + 2 * k;
            page[slot..slot + 2].copy_from_slice(&(at as u16).to_le_bytes());
        }
        out.extend(page);
        i += n;
    }
    out
}

/// The Quill text store: every story's text, the fonts, the text colours, default styles and the
/// character and paragraph runs (text-store.md).
fn quill(p: &Plan) -> Vec<u8> {
    const TEXT_AT: u32 = 512;
    let mut units: Vec<u16> = Vec::new();
    let mut char_runs = Vec::new();
    let mut para_runs = Vec::new();
    let mut lengths = Vec::new();
    for s in &p.stories {
        let start = units.len();
        for para in &s.paras {
            for (k, run) in para.runs.iter().enumerate() {
                units.extend(run.text.encode_utf16());
                if k + 1 == para.runs.len() {
                    units.push(0x0d);
                }
                let mut f = Vec::new();
                if run.bold {
                    f.extend([0x02, 0x0a]);
                }
                if run.italic {
                    f.extend([0x03, 0x0a]);
                }
                f.extend(number(0x0c, (run.size * EMU) as u32));
                f.extend(list(0x24, 0x8a, &list(0, 0x88, &number(0, run.font as u32))));
                // Colour table entry 0 is black; Publisher writes it rather than leaving it out.
                f.extend(list(0x44, 0x8a, &number(0, run.color.map_or(0, |c| c as u32 + 1))));
                char_runs.push((TEXT_AT + 2 * units.len() as u32, f));
            }
            let mut f = number(4, para.align);
            if para.space_after > 0.0 {
                f.extend(number(0x13, (para.space_after * EMU) as u32));
            }
            if para.bullet {
                let descriptor = [number(0, 23), number(1, 0xB7), number(2, 0)].concat();
                f.extend([number(0x0c, (-18.0 * EMU) as i32 as u32), number(0x0d, (18.0 * EMU) as u32), vec![3, 0x1a, SYMBOL as u8, 0]].concat());
                f.extend([0x57, 0x8a]);
                f.extend(record(&descriptor));
            }
            para_runs.push((TEXT_AT + 2 * units.len() as u32, f));
        }
        lengths.push((units.len() - start) as u32);
    }
    let text: Vec<u8> = units.iter().flat_map(|u| u.to_le_bytes()).collect();

    let mut fonts = vec![0; 20];
    fonts[4..8].copy_from_slice(&(FONTS.len() as u32).to_le_bytes());
    let entries: Vec<Vec<u8>> = FONTS
        .iter()
        .map(|f| {
            let mut e = (f.encode_utf16().count() as u16).to_le_bytes().to_vec();
            e.extend(f.encode_utf16().flat_map(u16::to_le_bytes));
            e.extend([0; 4]);
            e
        })
        .collect();
    let mut offset = 4 * FONTS.len();
    for e in &entries {
        fonts.extend((offset as u32).to_le_bytes());
        offset += e.len();
    }
    entries.iter().for_each(|e| fonts.extend(e));

    // Text colours: entry 0 black, then the plan's colours (direct RGB, red lowest).
    let mut colors = (1 + p.colors.len() as u32).to_le_bytes().to_vec();
    colors.extend([0x28, 0, 0, 0, 0, 0, 0, 0]);
    colors.extend(record(&number(0x01, 0)));
    for c in &p.colors {
        colors.extend(record(&number(0x01, red_lowest(*c))));
    }

    let defaults_char = [number(0x0c, (10.0 * EMU) as u32), list(0x24, 0x8a, &list(0, 0x88, &number(0, 0)))].concat();
    let mut styles = vec![0; 28];
    styles[4..8].copy_from_slice(&2u32.to_le_bytes());
    styles[20..24].copy_from_slice(&8u32.to_le_bytes());
    let second = 8 + 2 + record(&defaults_char).len();
    styles[24..28].copy_from_slice(&(second as u32).to_le_bytes());
    styles.extend([0; 2]);
    styles.extend(record(&defaults_char));
    styles.extend([0; 2]);
    styles.extend(record(&[]));

    let ids =
        [p.stories.len() as u32 + 1, p.stories.len() as u32].into_iter().chain(p.stories.iter().map(|s| s.id)).flat_map(u32::to_le_bytes).collect();
    let lens = [lengths.len() as u32, 8, 255].into_iter().chain(lengths).flat_map(u32::to_le_bytes).collect();
    let sections: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"TEXT", text),
        (b"FDPC", format_pages(&char_runs)),
        (b"FDPP", format_pages(&para_runs)),
        (b"SYID", ids),
        (b"STRS", lens),
        (b"FONT", fonts),
        (b"STSH", vec![0; 20]),
        (b"STSH", styles),
        (b"PL  ", colors),
    ];
    let mut quill = vec![0; 512];
    quill[..8].copy_from_slice(b"CHNKINK ");
    quill[26..28].copy_from_slice(&(sections.len() as u16).to_le_bytes());
    quill[28..32].copy_from_slice(&u32::MAX.to_le_bytes());
    for (i, (tag, data)) in sections.into_iter().enumerate() {
        let e = 32 + 24 * i;
        quill[e + 2..e + 6].copy_from_slice(tag);
        let offset = quill.len() as u32;
        quill[e + 16..e + 20].copy_from_slice(&offset.to_le_bytes());
        quill[e + 20..e + 24].copy_from_slice(&(data.len() as u32).to_le_bytes());
        quill.extend(data);
    }
    quill
}

/// 0xRRGGBB as the file stores a colour: red in the lowest byte (contents-stream.md §8).
fn red_lowest(c: u32) -> u32 {
    (c >> 16) & 0xff | (c & 0xff00) | (c & 0xff) << 16
}

fn hex(c: u32) -> String {
    format!("#{:06X}", c & 0x00FF_FFFF)
}

/// The publication as written, in the shape of Publisher's layout export (truth.rs reads it):
/// each page's elements, its master's first, with their bounds and, for text, every paragraph.
fn truth(p: &Plan) -> Value {
    let element = |o: &Object| -> Value {
        let [x, y, w, h] = o.rect;
        let bounds = json!({ "x": x, "y": y, "width": w, "height": h });
        match o.kind {
            Kind::Text { story, .. } => {
                let paragraphs: Vec<Value> = p.stories[story]
                    .paras
                    .iter()
                    .map(|para| {
                        let runs: Vec<Value> = para
                            .runs
                            .iter()
                            .map(|r| {
                                json!({ "text": r.expected, "style": {
                                    "family": FONTS[r.font], "size": r.size, "bold": r.bold, "italic": r.italic,
                                    "color": r.color.map_or("#000000".to_string(), |c| hex(p.colors[c])),
                                }})
                            })
                            .collect();
                        json!({ "runs": runs })
                    })
                    .collect();
                json!({ "type": "text", "bounds": bounds, "paragraphs": paragraphs })
            }
            Kind::Shape { fill, line, .. } => {
                json!({ "type": "shape", "bounds": bounds, "fill": hex(fill), "stroke": { "width": line.unwrap_or(0.0) } })
            }
            Kind::Picture { .. } => json!({ "type": "image", "bounds": bounds }),
        }
    };
    let pages: Vec<Value> = p
        .pages
        .iter()
        .map(|objects| {
            let elements: Vec<Value> = p.master.iter().flatten().chain(objects).map(element).collect();
            json!({ "width": p.width, "height": p.height, "elements": elements })
        })
        .collect();
    json!({ "generator": "cargo xtask synth", "pages": pages })
}

/// Writes `count` publications and their references to `out` (`stress-NN.pub`, `truth/stress-NN.json`).
pub fn write_set(out: &Path, count: usize, seed: u64) -> Result<(), String> {
    std::fs::create_dir_all(out.join("truth")).map_err(|e| format!("{}: {e}", out.display()))?;
    for n in 1..=count {
        let p = plan(seed, n);
        let name = format!("stress-{n:02}");
        let bytes = compound(&contents(&p), &quill(&p), &escher(&p));
        std::fs::write(out.join(format!("{name}.pub")), bytes).map_err(|e| e.to_string())?;
        let truth = serde_json::to_string_pretty(&truth(&p)).map_err(|e| e.to_string())?;
        std::fs::write(out.join("truth").join(format!("{name}.json")), truth).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_set_is_the_same_every_time() {
        let a = plan(1, 7);
        let b = plan(1, 7);
        assert_eq!(compound(&contents(&a), &quill(&a), &escher(&a)), compound(&contents(&b), &quill(&b), &escher(&b)));
        assert_eq!(truth(&a), truth(&b));
    }

    #[test]
    fn a_generated_publication_imports_with_all_its_pages_and_text() {
        for n in 1..=6 {
            let p = plan(1, n);
            let imported =
                foliolum_pub::import_pub(&compound(&contents(&p), &quill(&p), &escher(&p))).unwrap_or_else(|e| panic!("stress-{n:02}: {e}"));
            assert_eq!(imported.document.page_count(), p.pages.len(), "stress-{n:02}");
            let texts: Vec<String> = imported.document.stories.values().map(|s| s.text.clone()).collect();
            for s in &p.stories {
                let want =
                    s.paras.iter().map(|para| para.runs.iter().map(|r| r.expected.as_str()).collect::<String>()).collect::<Vec<_>>().join("\n");
                assert!(texts.contains(&want), "stress-{n:02}: story {} missing: {want:?}", s.id);
            }
        }
    }
}

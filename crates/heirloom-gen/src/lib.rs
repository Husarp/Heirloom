//! Realistic test families for tests and performance runs (PLAN.md §6–§7): Polish names with gender forms,
//! maiden and married names, approximate and Julian double dates, remarriages (half-siblings), adoptions,
//! the occasional marriage between relatives (the same ancestor twice), sources, notes and photos.
//! Everything is fictional and deterministic for a given seed.

use heirloom_core::gedcom::{Document, Node};
use heirloom_core::{Archive, Result};
use std::path::Path;

pub struct Options {
    pub people: usize,
    pub media: usize,
    pub seed: u64,
    pub name: String,
}

const MALE: &[&str] = &[
    "Józef", "Jan", "Stanisław", "Antoni", "Wojciech", "Franciszek", "Kazimierz", "Władysław", "Tadeusz", "Łukasz",
    "Jakub", "Maciej", "Wawrzyniec", "Ignacy", "Bartłomiej", "Szczepan", "Mikołaj", "Zdzisław", "Bolesław", "Piotr",
    "Paweł", "Michał", "Andrzej", "Tomasz", "Walenty", "Wincenty", "Feliks", "Ludwik", "Henryk", "Zygmunt",
];
const FEMALE: &[&str] = &[
    "Marianna", "Agnieszka", "Jadwiga", "Zofia", "Katarzyna", "Małgorzata", "Helena", "Józefa", "Anna", "Bronisława",
    "Stanisława", "Genowefa", "Łucja", "Elżbieta", "Wiktoria", "Aniela", "Rozalia", "Franciszka", "Apolonia",
    "Julianna", "Teresa", "Barbara", "Weronika", "Tekla", "Ewa", "Magdalena", "Salomea", "Scholastyka",
];
/// (male form, female form)
const SURNAMES: &[(&str, &str)] = &[
    ("Kowalski", "Kowalska"), ("Wiśniewski", "Wiśniewska"), ("Wójcik", "Wójcik"), ("Kamiński", "Kamińska"),
    ("Lewandowski", "Lewandowska"), ("Zieliński", "Zielińska"), ("Woźniak", "Woźniak"), ("Dąbrowski", "Dąbrowska"),
    ("Kozłowski", "Kozłowska"), ("Mazur", "Mazur"), ("Nowak", "Nowak"), ("Król", "Król"), ("Wróbel", "Wróbel"),
    ("Zając", "Zając"), ("Pawłowski", "Pawłowska"), ("Jabłoński", "Jabłońska"), ("Bąk", "Bąk"),
    ("Szczepański", "Szczepańska"), ("Kaźmierczak", "Kaźmierczak"), ("Głowacki", "Głowacka"),
    ("Ziółkowski", "Ziółkowska"), ("Sikora", "Sikora"), ("Duda", "Duda"), ("Żak", "Żak"), ("Łuczak", "Łuczak"),
    ("Sobczak", "Sobczak"), ("Czerwiński", "Czerwińska"), ("Michalak", "Michalak"), ("Ostrowski", "Ostrowska"),
];
const PLACES: &[&str] = &[
    "Wólka, parafia Łęczna, powiat lubelski, gubernia lubelska",
    "Łęczna, powiat lubelski, gubernia lubelska",
    "Ciechanki, parafia Łęczna, powiat lubelski, gubernia lubelska",
    "Lublin, gubernia lubelska",
    "Żółkiewka, powiat krasnostawski, gubernia lubelska",
    "Wiskitki, powiat błoński, gubernia warszawska",
    "Łowicz, gubernia warszawska",
    "Kraków",
    "Tarnów, Galicja",
    "Poznań",
    "Warszawa",
    "Chełm, gubernia lubelska",
];
const MALE_JOBS: &[&str] = &["rolnik", "kowal", "kolejarz", "cieśla", "młynarz", "nauczyciel", "stolarz", "szewc"];
const FEMALE_JOBS: &[&str] = &["gospodyni", "krawcowa", "nauczycielka", "akuszerka", "służąca", "tkaczka"];
const MONTHS: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];
const MONTHS_PL: [&str; 12] = [
    "stycznia", "lutego", "marca", "kwietnia", "maja", "czerwca", "lipca", "sierpnia", "września", "października",
    "listopada", "grudnia",
];
const LAST_YEAR: i32 = 2024;

/// SplitMix64: small, fast and deterministic.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n.max(1) as u64).unwrap_or(0)
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + i32::try_from(self.below(usize::try_from(hi - lo + 1).unwrap_or(1))).unwrap_or(0)
    }
    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

struct Person {
    given: &'static str,
    surname: usize,
    female: bool,
    birth: i32,
    death: Option<i32>,
    birth_place: usize,
    death_place: usize,
    famc: Option<usize>,
    adopted: bool,
    fams: Vec<usize>,
    job: Option<&'static str>,
    note: bool,
}

struct Family {
    husb: Option<usize>,
    wife: Option<usize>,
    children: Vec<usize>,
    year: i32,
    place: usize,
}

struct Tree {
    rng: Rng,
    people: Vec<Person>,
    families: Vec<Family>,
    target: usize,
}

impl Tree {
    fn new_person(&mut self, female: bool, surname: usize, birth: i32, famc: Option<usize>) -> usize {
        let rng = &mut self.rng;
        let lifespan = if rng.chance(12) { rng.range(0, 5) } else { rng.range(38, 94) };
        let death = Some(birth + lifespan).filter(|d| *d <= LAST_YEAR);
        let adult = lifespan >= 16 && birth < 2006;
        let job = (adult && rng.chance(60)).then(|| *rng.pick(if female { FEMALE_JOBS } else { MALE_JOBS }));
        let person = Person {
            given: *rng.pick(if female { FEMALE } else { MALE }),
            surname,
            female,
            birth,
            death,
            birth_place: rng.below(PLACES.len()),
            death_place: rng.below(PLACES.len()),
            famc,
            adopted: famc.is_some() && rng.chance(1),
            fams: Vec::new(),
            job,
            note: rng.chance(10),
        };
        self.people.push(person);
        self.people.len() - 1
    }

    fn marry(&mut self, a: usize, b: usize, year: i32) {
        let (husb, wife) = if self.people[a].female { (b, a) } else { (a, b) };
        let place = self.rng.below(PLACES.len());
        self.families.push(Family { husb: Some(husb), wife: Some(wife), children: Vec::new(), year, place });
        let f = self.families.len() - 1;
        self.people[a].fams.push(f);
        self.people[b].fams.push(f);
    }

    /// Someone unmarried of the other sex and a similar age from the recent generations, not a sibling —
    /// sometimes a cousin, which makes the same ancestors appear twice in the tree.
    fn relative_to_marry(&mut self, person: usize) -> Option<usize> {
        let p = &self.people[person];
        let (female, birth, famc) = (p.female, p.birth, p.famc);
        let start = self.people.len().saturating_sub(400);
        for _ in 0..40 {
            let c = start + self.rng.below(self.people.len() - start);
            let q = &self.people[c];
            if c != person && q.female != female && q.fams.is_empty() && (q.birth - birth).abs() <= 6 && q.famc != famc {
                return Some(c);
            }
        }
        None
    }

    fn founding_couple(&mut self, husband_born: i32, wife_born: i32) {
        let (hs, ws) = (self.rng.below(SURNAMES.len()), self.rng.below(SURNAMES.len()));
        let husband = self.new_person(false, hs, husband_born, None);
        let wife = self.new_person(true, ws, wife_born, None);
        let year = husband_born.max(wife_born) + self.rng.range(20, 28);
        self.marry(husband, wife, year);
    }

    /// Real archives span ~1700 to today, so growth is paced: behind schedule for someone's birth year, more
    /// people marry (and have children in the tree); ahead of it, fewer.
    fn behind_schedule(&self, born: i32) -> bool {
        let progress = (f64::from(born - 1700) / 320.0).clamp(0.0, 1.0);
        (self.people.len() as f64) < self.target as f64 * progress * progress
    }

    fn grow(&mut self) {
        let target = self.target;
        let founders = (target / 1000).max(2);
        for _ in 0..founders {
            let born = self.rng.range(1680, 1720);
            let wife_born = born + self.rng.range(-5, 5);
            self.founding_couple(born, wife_born);
        }
        let mut next = 0;
        while self.people.len() < target {
            if next >= self.families.len() {
                // Every line ended: start another founding couple, a generation before the latest births.
                let latest = self.people.iter().map(|p| p.birth).max().unwrap_or(1700);
                let born = (latest - 25).clamp(1700, 1990);
                if target - self.people.len() < 2 {
                    // One place left: a single person, so the family has exactly `target` people.
                    let surname = self.rng.below(SURNAMES.len());
                    self.new_person(false, surname, born, None);
                    continue;
                }
                self.founding_couple(born, born);
            }
            let f = next;
            next += 1;
            let kids = *self.rng.pick(&[0, 1, 1, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 5, 5, 6, 7]);
            for k in 0..kids {
                if self.people.len() >= target {
                    break;
                }
                let family = &self.families[f];
                let born = family.year + 1 + k * 2 + self.rng.range(0, 1);
                if born > LAST_YEAR {
                    break;
                }
                let surname = match (family.husb, family.wife) {
                    (Some(h), _) | (None, Some(h)) => self.people[h].surname,
                    (None, None) => self.rng.below(SURNAMES.len()),
                };
                let female = self.rng.chance(50);
                let child = self.new_person(female, surname, born, Some(f));
                self.families[f].children.push(child);
                self.maybe_marry(child);
            }
        }
    }

    fn maybe_marry(&mut self, person: usize) {
        let p = &self.people[person];
        let (born, female, lived) = (p.birth, p.female, p.death.map_or(LAST_YEAR - p.birth, |d| d - p.birth));
        let percent = if self.behind_schedule(born) { 85 } else { 20 };
        if self.people.len() >= self.target || lived < 18 || born > 2004 || !self.rng.chance(percent) {
            return;
        }
        let spouse = match self.rng.chance(2).then(|| self.relative_to_marry(person)).flatten() {
            Some(s) => s,
            None => {
                let (surname, spouse_born) = (self.rng.below(SURNAMES.len()), born + self.rng.range(-6, 6));
                self.new_person(!female, surname, spouse_born, None)
            }
        };
        let year = born.max(self.people[spouse].birth) + self.rng.range(19, 28);
        if year > LAST_YEAR {
            return;
        }
        self.marry(person, spouse, year);
        // Some people marry again: children of the second marriage are half-siblings.
        let second = year + self.rng.range(8, 16);
        if self.rng.chance(8) && second < LAST_YEAR && self.people.len() < self.target {
            let (surname, other_born) = (self.rng.below(SURNAMES.len()), born + self.rng.range(-3, 10));
            let other = self.new_person(!female, surname, other_born, None);
            self.marry(person, other, second);
        }
    }
}

/// The GEDCOM date text for a year: often exact, sometimes approximate, occasionally a range.
/// Records from Congress Poland 1868–1917 sometimes carry the Julian date too (kept as a PHRASE).
fn date(rng: &mut Rng, year: i32) -> Node {
    let roll = rng.below(100);
    let (text, phrase) = match roll {
        0..=59 => {
            let (day, month) = (rng.range(1, 28), rng.below(12));
            let phrase = ((1868..=1917).contains(&year) && rng.chance(30)).then(|| julian_phrase(day, month, year));
            (format!("{day} {} {year}", MONTHS[month]), phrase)
        }
        60..=74 => (format!("ABT {year}"), None),
        75..=89 => (year.to_string(), None),
        90..=92 => (format!("BEF {year}"), None),
        93..=95 => (format!("AFT {year}"), None),
        _ => (format!("BET {year} AND {}", year + 2), None),
    };
    let mut node = Node::with_value("DATE", &text);
    if let Some(p) = phrase {
        node.children.push(Node::with_value("PHRASE", &p));
    }
    node
}

/// "28 lutego / 12 marca 1878": the Julian date first, as written in Russian-era records.
fn julian_phrase(day: i32, month: usize, year: i32) -> String {
    let shift = if year < 1900 { 12 } else { 13 };
    let (jd, jm) = if day > shift { (day - shift, month) } else { (day - shift + 28, (month + 11) % 12) };
    format!("{jd} {} / {day} {} {year}", MONTHS_PL[jm], MONTHS_PL[month])
}

fn uid() -> Node {
    Node::with_value("UID", &uuid::Uuid::new_v4().to_string())
}

/// Builds the document (and the list of photo files to create).
pub fn generate(options: &Options) -> (Document, Vec<(String, [u8; 3])>) {
    let mut tree = Tree { rng: Rng(options.seed), people: Vec::new(), families: Vec::new(), target: options.people };
    tree.grow();
    let mut rng = Rng(options.seed ^ 0xDEAD_BEEF);
    let mut doc = Document::new_v7();
    let trailer = doc.records.pop().unwrap_or_else(|| Node::new("TRLR"));

    let mut photos_for: Vec<Vec<usize>> = vec![Vec::new(); tree.people.len()];
    let mut media = Vec::new();
    for m in 0..options.media {
        let person = rng.below(tree.people.len());
        photos_for[person].push(m);
        let colour = [rng.below(256) as u8, rng.below(256) as u8, rng.below(256) as u8];
        media.push((format!("M{:04}.png", m + 1), colour));
    }

    for (i, p) in tree.people.iter().enumerate() {
        let (male_surname, female_surname) = SURNAMES[p.surname];
        let own = if p.female { female_surname } else { male_surname };
        let mut indi = Node::new("INDI").with_xref(&format!("@I{}@", i + 1)).with_child(uid());
        let mut birth_name = Node::with_value("NAME", &format!("{} /{own}/", p.given))
            .with_child(Node::with_value("GIVN", p.given))
            .with_child(Node::with_value("SURN", own));
        let husband = p.female.then(|| p.fams.first()).flatten().and_then(|f| tree.families[*f].husb);
        if let Some(h) = husband {
            let married = SURNAMES[tree.people[h].surname].1;
            birth_name.children.push(Node::with_value("TYPE", "BIRTH"));
            indi.children.push(birth_name);
            indi.children.push(
                Node::with_value("NAME", &format!("{} /{married}/", p.given))
                    .with_child(Node::with_value("GIVN", p.given))
                    .with_child(Node::with_value("SURN", married))
                    .with_child(Node::with_value("TYPE", "MARRIED")),
            );
        } else {
            indi.children.push(birth_name);
        }
        indi.children.push(Node::with_value("SEX", if p.female { "F" } else { "M" }));
        let mut birth = Node::new("BIRT").with_child(date(&mut rng, p.birth)).with_child(Node::with_value("PLAC", PLACES[p.birth_place]));
        if rng.chance(50) {
            birth.children.push(
                Node::with_value("SOUR", &format!("@S{}@", p.birth_place + 1))
                    .with_child(Node::with_value("PAGE", &format!("akt {}/{}", rng.range(1, 180), p.birth))),
            );
        }
        indi.children.push(birth);
        if let Some(d) = p.death {
            indi.children.push(Node::new("DEAT").with_child(date(&mut rng, d)).with_child(Node::with_value("PLAC", PLACES[p.death_place])));
        }
        if let Some(job) = p.job {
            indi.children.push(Node::with_value("OCCU", job));
        }
        if p.note {
            let mut note = Node::new("NOTE");
            let (born, worked) = if p.female { ("urodziła", "Pracowała") } else { ("urodził", "Pracował") };
            let town = PLACES[p.birth_place].split(',').next().unwrap_or("");
            note.set_text(
                &format!(
                    "{} {born} się w miejscowości {town}.\n{worked} jako {}.\nWedług relacji rodzinnej lubił{} opowiadać o dawnych czasach.",
                    p.given,
                    p.job.unwrap_or("gospodarz"),
                    if p.female { "a" } else { "" }
                ),
                heirloom_core::gedcom::Version::V7,
            );
            indi.children.push(note);
        }
        if let Some(f) = p.famc {
            let mut famc = Node::with_value("FAMC", &format!("@F{}@", f + 1));
            if p.adopted {
                famc.children.push(Node::with_value("PEDI", "ADOPTED"));
            }
            indi.children.push(famc);
        }
        for f in &p.fams {
            indi.children.push(Node::with_value("FAMS", &format!("@F{}@", f + 1)));
        }
        for m in &photos_for[i] {
            indi.children.push(Node::with_value("OBJE", &format!("@O{}@", m + 1)));
        }
        doc.records.push(indi);
    }

    for (i, f) in tree.families.iter().enumerate() {
        let mut fam = Node::new("FAM").with_xref(&format!("@F{}@", i + 1)).with_child(uid());
        if let Some(h) = f.husb {
            fam.children.push(Node::with_value("HUSB", &format!("@I{}@", h + 1)));
        }
        if let Some(w) = f.wife {
            fam.children.push(Node::with_value("WIFE", &format!("@I{}@", w + 1)));
        }
        for c in &f.children {
            fam.children.push(Node::with_value("CHIL", &format!("@I{}@", c + 1)));
        }
        fam.children.push(Node::new("MARR").with_child(date(&mut rng, f.year)).with_child(Node::with_value("PLAC", PLACES[f.place])));
        doc.records.push(fam);
    }

    for (i, place) in PLACES.iter().enumerate() {
        let parish = place.split(',').next().unwrap_or(place);
        doc.records.push(
            Node::new("SOUR")
                .with_xref(&format!("@S{}@", i + 1))
                .with_child(Node::with_value("TITL", &format!("Księgi metrykalne parafii {parish}"))),
        );
    }

    for (m, (file, _)) in media.iter().enumerate() {
        doc.records.push(
            Node::new("OBJE").with_xref(&format!("@O{}@", m + 1)).with_child(
                Node::with_value("FILE", &format!("media/{file}"))
                    .with_child(Node::with_value("FORM", "image/png"))
                    .with_child(Node::with_value("TITL", &format!("Zdjęcie {}", m + 1))),
            ),
        );
    }
    doc.records.push(trailer);
    (doc, media)
}

/// Writes a complete archive folder: `rodzina.ged`, `media/` with small coloured photos, `.heirloom/`.
pub fn write_archive(root: &Path, options: &Options) -> Result<Archive> {
    let (doc, media) = generate(options);
    let archive = Archive::create_from_document(root, &options.name, doc)?;
    for (file, colour) in media {
        let image = image::RgbImage::from_fn(64, 64, |x, y| {
            let shade = u8::try_from((x + y) / 4).unwrap_or(0);
            image::Rgb([colour[0].saturating_add(shade), colour[1], colour[2].saturating_sub(shade)])
        });
        image.save(root.join("media").join(file)).map_err(|e| std::io::Error::other(e.to_string()))?;
    }
    Ok(archive)
}

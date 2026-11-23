use super::*;
use cad_document::Rebuilder;

type Entities = BTreeMap<usize, (String, Vec<String>)>;

/// Splits at commas outside parentheses and quoted strings (`''` toggles twice).
fn split_args(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth, mut quoted) = (Vec::new(), String::new(), 0, false);
    for c in s.chars() {
        match c {
            '\'' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth -= 1,
            ',' if !quoted && depth == 0 => {
                out.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    out.push(cur);
    out
}

/// Class and top-level arguments of every line of the DATA section.
fn entities(ifc: &str) -> Entities {
    let data = ifc.split("DATA;\n").nth(1).unwrap().split("ENDSEC;").next().unwrap();
    data.lines()
        .map(|l| {
            let (k, text) = l.strip_prefix('#').unwrap().split_once('=').unwrap();
            let text = text.strip_suffix(';').unwrap();
            let (class, args) = text.split_once('(').unwrap();
            (k.parse().unwrap(), (class.to_string(), split_args(args.strip_suffix(')').unwrap())))
        })
        .collect()
}

/// Numbers after a `#` outside quoted strings.
fn refs(text: &str) -> Vec<usize> {
    let (mut out, mut quoted, mut chars) = (Vec::new(), false, text.chars().peekable());
    while let Some(c) = chars.next() {
        match c {
            '\'' => quoted = !quoted,
            '#' if !quoted => {
                let mut n = String::new();
                while let Some(d) = chars.next_if(char::is_ascii_digit) {
                    n.push(d);
                }
                out.push(n.parse().unwrap());
            }
            _ => {}
        }
    }
    out
}

fn count(map: &Entities, class: &str) -> usize {
    map.values().filter(|(c, _)| c == class).count()
}

fn of<'a>(map: &'a Entities, class: &str) -> Vec<&'a Vec<String>> {
    map.values().filter(|(c, _)| c == class).map(|(_, a)| a).collect()
}

fn num(s: &str) -> f64 {
    s.parse().unwrap()
}

fn house_ifc() -> (cad_document::Document, RebuildResult, String) {
    let d = cad_document::samples::house();
    let r = Rebuilder::default().rebuild(&d);
    let ifc = to_ifc(&d, &r).unwrap();
    (d, r, ifc)
}

#[test]
fn guid_and_string_encoding() {
    assert_eq!(ifc_guid(0), "0000000000000000000000");
    assert_eq!(ifc_guid(u128::MAX), "3$$$$$$$$$$$$$$$$$$$$$");
    assert!(ifc_guid(63).ends_with("0$"));
    assert_eq!(ifc_str("it's"), "'it''s'");
    assert_eq!(ifc_str("Küche"), "'K\\X2\\00FC\\X0\\che'");
    assert_eq!(ifc_str("a\\b"), "'a\\\\b'");
}

#[test]
fn house_entity_graph_is_closed() {
    let (_, _, ifc) = house_ifc();
    assert!(ifc.starts_with("ISO-10303-21;"));
    assert!(ifc.contains("FILE_SCHEMA(('IFC4'));"));
    assert!(ifc.ends_with("END-ISO-10303-21;\n"));
    let map = entities(&ifc);
    let data = ifc.split("DATA;\n").nth(1).unwrap();
    for n in refs(data) {
        assert!(map.contains_key(&n), "#{n} is not defined");
    }
    for (class, n) in [
        ("IFCPROJECT", 1),
        ("IFCSITE", 1),
        ("IFCBUILDING", 1),
        ("IFCBUILDINGSTOREY", 2),
        ("IFCWALL", 4),
        ("IFCOPENINGELEMENT", 2),
        ("IFCDOOR", 1),
        ("IFCWINDOW", 1),
        ("IFCRELVOIDSELEMENT", 2),
        ("IFCRELFILLSELEMENT", 2),
        ("IFCSLAB", 1),
        ("IFCROOF", 1),
        ("IFCSTAIR", 1),
        ("IFCSPACE", 1),
        ("IFCEXTRUDEDAREASOLID", 10),
        ("IFCFACETEDBREP", 2),
        ("IFCRELCONTAINEDINSPATIALSTRUCTURE", 2),
        ("IFCRELAGGREGATES", 4),
    ] {
        assert_eq!(count(&map, class), n, "{class}");
    }
}

#[test]
fn attribute_counts_match_ifc4() {
    let (_, _, ifc) = house_ifc();
    let table = [
        ("IFCPROJECT", 9),
        ("IFCSITE", 14),
        ("IFCBUILDING", 12),
        ("IFCBUILDINGSTOREY", 10),
        ("IFCWALL", 9),
        ("IFCSLAB", 9),
        ("IFCROOF", 9),
        ("IFCSTAIR", 9),
        ("IFCOPENINGELEMENT", 9),
        ("IFCDOOR", 13),
        ("IFCWINDOW", 13),
        ("IFCSPACE", 11),
        ("IFCPROPERTYSET", 5),
        ("IFCELEMENTQUANTITY", 6),
        ("IFCPROPERTYSINGLEVALUE", 4),
        ("IFCQUANTITYAREA", 5),
        ("IFCQUANTITYLENGTH", 5),
        ("IFCQUANTITYVOLUME", 5),
        ("IFCEXTRUDEDAREASOLID", 4),
        ("IFCSHAPEREPRESENTATION", 4),
        ("IFCPRODUCTDEFINITIONSHAPE", 3),
        ("IFCLOCALPLACEMENT", 2),
        ("IFCAXIS2PLACEMENT3D", 3),
        ("IFCGEOMETRICREPRESENTATIONSUBCONTEXT", 10),
    ];
    let mut seen = 0;
    for (class, args) in entities(&ifc).values() {
        let want = if class.starts_with("IFCREL") { Some(6) } else { table.iter().find(|(c, _)| c == class).map(|(_, n)| *n) };
        if let Some(n) = want {
            assert_eq!(args.len(), n, "{class}: {args:?}");
            seen += 1;
        }
    }
    assert!(seen > 100, "{seen}");
}

#[test]
fn global_ids_are_valid_unique_and_stable() {
    let (d, r, ifc) = house_ifc();
    let rooted = [
        "IFCPROJECT",
        "IFCSITE",
        "IFCBUILDING",
        "IFCBUILDINGSTOREY",
        "IFCWALL",
        "IFCDOOR",
        "IFCWINDOW",
        "IFCOPENINGELEMENT",
        "IFCSLAB",
        "IFCROOF",
        "IFCSTAIR",
        "IFCSPACE",
        "IFCRELAGGREGATES",
        "IFCRELCONTAINEDINSPATIALSTRUCTURE",
        "IFCRELVOIDSELEMENT",
        "IFCRELFILLSELEMENT",
        "IFCRELDEFINESBYPROPERTIES",
        "IFCPROPERTYSET",
        "IFCELEMENTQUANTITY",
    ];
    let mut ids = Vec::new();
    for (class, args) in entities(&ifc).values() {
        if rooted.contains(&class.as_str()) {
            let g = args[0].strip_prefix('\'').and_then(|s| s.strip_suffix('\'')).unwrap();
            assert!(g.len() == 22 && g.bytes().all(|b| IFC64.contains(&b)), "{class}: {g}");
            ids.push(g.to_string());
        }
    }
    let n = ids.len();
    assert!(n > 30);
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n);
    assert_eq!(to_ifc(&d, &r).unwrap(), ifc);
}

#[test]
fn storeys_chain_to_the_site() {
    let (_, _, ifc) = house_ifc();
    let map = entities(&ifc);
    let storeys: Vec<_> = of(&map, "IFCBUILDINGSTOREY");
    assert_eq!(storeys.iter().map(|a| num(&a[9])).collect::<Vec<_>>(), [0.0, 3000.0]);
    let (building, site) = (of(&map, "IFCBUILDING")[0], of(&map, "IFCSITE")[0]);
    let target = |placement: &str| &map[&refs(placement)[0]].1;
    // Every storey placement hangs off the building's, the building's off the site's, the site's is absolute.
    for s in &storeys {
        assert_eq!(target(&s[5])[0], building[5]);
    }
    assert_eq!(target(&building[5])[0], site[5]);
    assert_eq!(target(&site[5])[0], "$");
    for wall in of(&map, "IFCWALL") {
        let rel = &target(&wall[5])[0];
        assert!(storeys.iter().any(|s| &s[5] == rel), "{rel}");
    }
    let axis = &map[&refs(&target(&storeys[1][5])[1])[0]].1[0];
    let point = &map[&refs(axis)[0]].1[0];
    let xyz: Vec<f64> = point.trim_matches(|c| c == '(' || c == ')').split(',').map(num).collect();
    assert_eq!(xyz, [0.0, 0.0, 3000.0]);
}

#[test]
fn openings_void_walls_and_hold_doors() {
    let (_, _, ifc) = house_ifc();
    let map = entities(&ifc);
    let class = |s: &str| map[&refs(s)[0]].0.as_str();
    for a in of(&map, "IFCRELVOIDSELEMENT") {
        assert_eq!((class(&a[4]), class(&a[5])), ("IFCWALL", "IFCOPENINGELEMENT"));
    }
    for a in of(&map, "IFCRELFILLSELEMENT") {
        assert_eq!(class(&a[4]), "IFCOPENINGELEMENT");
        assert!(["IFCDOOR", "IFCWINDOW"].contains(&class(&a[5])));
    }
    let door = of(&map, "IFCDOOR")[0];
    assert_eq!((num(&door[8]), num(&door[9])), (2100.0, 900.0));
}

#[test]
fn property_sets_come_from_pset_metadata() {
    let (_, _, ifc) = house_ifc();
    let map = entities(&ifc);
    let walls: Vec<_> = of(&map, "IFCPROPERTYSET").into_iter().filter(|a| a[2] == "'Pset_WallCommon'").collect();
    assert_eq!(walls.len(), 1);
    assert_eq!(refs(&walls[0][4]).len(), 2);
    assert!(ifc.contains("IFCPROPERTYSINGLEVALUE('FireRating',$,IFCLABEL('EI60'),$)"));
    assert!(ifc.contains("IFCPROPERTYSINGLEVALUE('IsExternal',$,IFCBOOLEAN(.T.),$)"));
    assert!(ifc.contains("'Pset_DoorCommon'"));
    assert!(!ifc.contains("not exported"));
}

#[test]
fn space_name_and_quantities_come_from_the_room() {
    let (_, r, ifc) = house_ifc();
    let map = entities(&ifc);
    let space = of(&map, "IFCSPACE")[0];
    assert_eq!((space[2].as_str(), space[7].as_str()), ("'1'", "'Living'"));
    let region = &r.rooms[0].1;
    assert!((num(&of(&map, "IFCQUANTITYAREA")[0][3]) - region.area / 1e6).abs() < 1e-6);
    assert!((num(&of(&map, "IFCQUANTITYLENGTH")[0][3]) - region.perimeter).abs() < 1e-6);
}

#[test]
fn no_levels_is_an_error() {
    let d = cad_document::Document::new("x");
    assert!(to_ifc(&d, &Rebuilder::default().rebuild(&d)).is_err());
}

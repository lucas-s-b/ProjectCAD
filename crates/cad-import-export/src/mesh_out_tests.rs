use super::*;
use cad_geometry::{DVec2, Shape, profile::Profile};
use cad_modeler::{Plane3, extrude};

fn body(shape: Shape, z0: f64, height: f64) -> Body {
    let profile = Profile::from_shapes(&[&shape]).unwrap();
    let plane = Plane3 { origin: DVec3::new(0.0, 0.0, z0), ..Plane3::XY };
    Body::from_brep(&extrude(&profile, &plane, height).unwrap(), 0.01).unwrap()
}

fn cuboid() -> Body {
    body(Shape::rect(DVec2::ZERO, DVec2::new(40.0, 20.0)), 0.0, 10.0)
}

fn cylinder() -> Body {
    body(Shape::Circle { center: DVec2::new(5.0, 5.0), radius: 7.0 }, 0.0, 12.0)
}

fn volume(tris: &[[DVec3; 3]]) -> f64 {
    tris.iter().map(|[a, b, c]| a.dot(b.cross(*c)) / 6.0).sum()
}

fn read_stl(bytes: &[u8]) -> Vec<(DVec3, [DVec3; 3])> {
    let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    assert_eq!(bytes.len(), 84 + 50 * count);
    let f = |at: usize| f64::from(f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()));
    let v = |at: usize| DVec3::new(f(at), f(at + 4), f(at + 8));
    (0..count)
        .map(|i| {
            let o = 84 + 50 * i;
            assert_eq!(&bytes[o + 48..o + 50], &[0, 0]);
            (v(o), [v(o + 12), v(o + 24), v(o + 36)])
        })
        .collect()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

#[test]
fn stl_layout_volume_and_normals() {
    let b = cuboid();
    let bytes = to_stl(&[&b]);
    assert_eq!(bytes.len(), 84 + 50 * b.mesh.triangle_count());
    assert!(bytes.starts_with(STL_HEADER) && bytes[STL_HEADER.len()..80].iter().all(|&x| x == 0));
    let tris = read_stl(&bytes);
    assert_eq!(tris.len(), b.mesh.triangle_count());
    let corners: Vec<[DVec3; 3]> = tris.iter().map(|t| t.1).collect();
    assert!(rel(volume(&corners), b.volume()) < 1e-5);
    let center = b.centroid();
    for (n, [p, q, r]) in &tris {
        assert!((n.length() - 1.0).abs() < 1e-6);
        let winding = (*q - *p).cross(*r - *p);
        assert!(winding.dot(*n) > 0.0);
        assert!(n.dot((*p + *q + *r) / 3.0 - center) > 0.0, "normal points into the solid");
    }
}

#[test]
fn stl_of_several_bodies_and_none() {
    let (a, b) = (cuboid(), cylinder());
    let bytes = to_stl(&[&a, &b]);
    assert_eq!(read_stl(&bytes).len(), a.mesh.triangle_count() + b.mesh.triangle_count());
    let empty = to_stl(&[]);
    assert_eq!(empty.len(), 84);
    assert_eq!(read_stl(&empty).len(), 0);
}

#[test]
fn degenerate_triangle_has_zero_normal() {
    let p = DVec3::new(1.0, 2.0, 3.0);
    assert_eq!(facet_normal([p, p, p]), DVec3::ZERO);
    assert_eq!(facet_normal([p, p + DVec3::X, p + DVec3::X * 2.0]), DVec3::ZERO);
    assert_eq!(facet_normal([DVec3::ZERO, DVec3::X, DVec3::Y]), DVec3::Z);
}

/// (vertex lines, face lines, `o` lines) of an OBJ text, checking the shapes of the lines.
fn parse_obj(text: &str) -> (Vec<DVec3>, Vec<[usize; 3]>, usize) {
    let (mut v, mut f, mut o) = (Vec::new(), Vec::new(), 0);
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let w: Vec<&str> = line.split(' ').collect();
        match w[0] {
            "o" => o += 1,
            "v" => v.push(DVec3::new(w[1].parse().unwrap(), w[2].parse().unwrap(), w[3].parse().unwrap())),
            "f" => f.push([1, 2, 3].map(|k| w[k].parse::<usize>().unwrap())),
            other => panic!("unexpected line type {other}"),
        }
    }
    (v, f, o)
}

#[test]
fn obj_counts_indices_and_volume() {
    let b = cylinder();
    let text = to_obj(&[(7, &b)]);
    assert!(text.starts_with("# ProjectCAD OBJ, units mm\no Body_7\n"));
    let (v, f, o) = parse_obj(&text);
    assert_eq!((v.len(), f.len(), o), (b.mesh.positions.len(), b.mesh.triangle_count(), 1));
    assert!(f.iter().flatten().all(|&i| (1..=v.len()).contains(&i)));
    let tris: Vec<[DVec3; 3]> = f.iter().map(|t| t.map(|i| v[i - 1])).collect();
    assert!(rel(volume(&tris), b.volume()) < 1e-9);
}

#[test]
fn obj_indices_are_global_across_bodies() {
    let (a, b) = (cuboid(), cylinder());
    let (v, f, o) = parse_obj(&to_obj(&[(1, &a), (2, &b)]));
    assert_eq!(o, 2);
    let (na, nb) = (a.mesh.positions.len(), b.mesh.positions.len());
    assert_eq!((v.len(), f.len()), (na + nb, a.mesh.triangle_count() + b.mesh.triangle_count()));
    let (first, second) = f.split_at(a.mesh.triangle_count());
    assert!(first.iter().flatten().all(|&i| (1..=na).contains(&i)));
    assert!(second.iter().flatten().all(|&i| (na + 1..=na + nb).contains(&i)));
}

#[test]
fn obj_without_bodies_is_only_the_comment() {
    assert_eq!(to_obj(&[]), "# ProjectCAD OBJ, units mm\n");
}

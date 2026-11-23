//! IFC4 export (ISO 10303-21 text) of the architectural model: project, site, building, one storey per level; walls,
//! openings, doors, windows, slabs and spaces as extrusions, roofs and stairs as faceted B-Reps; property sets from
//! `Pset_X.Prop` metadata and base quantities. Lengths in mm, areas in m², volumes in m³.
use crate::step_out::{W, real, refs};
use cad_document::{Document, Entity, RebuildResult};
use cad_geometry::{DVec2, Opening, OpeningKind, RoofKind, Shape, Wall};
use cad_modeler::Body;
use std::collections::BTreeMap;
use std::fmt::Write;

const IFC64: &[u8; 64] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";

/// The 22-character IFC GlobalId of a 128-bit number: the first character holds the top 2 bits, each of the other 21 the next 6.
pub(crate) fn ifc_guid(v: u128) -> String {
    (0..22).map(|i| IFC64[((v >> (6 * (21 - i))) & 0x3f) as usize] as char).collect()
}

/// FNV-1a, 64 bits.
fn fnv1a(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// An IFC string literal: quotes and backslashes doubled, characters outside printable ASCII as \X2\<UTF-16 hex>\X0\.
pub(crate) fn ifc_str(s: &str) -> String {
    let mut out = String::from("'");
    for c in s.chars() {
        match c {
            '\'' => out.push_str("''"),
            '\\' => out.push_str("\\\\"),
            ' '..='~' => out.push(c),
            _ => {
                out.push_str("\\X2\\");
                for u in c.encode_utf16(&mut [0; 2]) {
                    let _ = write!(out, "{u:04X}");
                }
                out.push_str("\\X0\\");
            }
        }
    }
    out.push('\'');
    out
}

struct Ifc<'a> {
    w: W,
    doc: &'a Document,
    rels: u64,
    /// The shared `IFCDIRECTION((0.,0.,1.))`.
    z: usize,
    /// The Body subcontext.
    body: usize,
}

impl Ifc<'_> {
    /// GlobalId of object `id` of kind `tag` ("project", "site", "building", "storey", "element", "opening", "rel"):
    /// the document name and the kind hashed into the top 64 bits, the id in the bottom 64.
    // shortcut: a renamed document gets new GlobalIds; store a project GUID in the document if users merge IFC models across renames
    fn guid(&self, tag: &str, id: u64) -> String {
        ifc_str(&ifc_guid(u128::from(fnv1a(&format!("{}/{tag}", self.doc.name))) << 64 | u128::from(id)))
    }

    /// A relationship (any IfcRel* or property definition): its own GlobalId, no owner history, name or description.
    fn rel(&mut self, class: &str, args: &str) -> usize {
        self.rels += 1;
        let g = self.guid("rel", self.rels);
        self.w.add(&format!("{class}({g},$,$,$,{args})"))
    }

    /// IFCLOCALPLACEMENT relative to `to` (None: absolute), its origin `z` mm up.
    fn placement(&mut self, to: Option<usize>, z: f64) -> usize {
        let p = self.w.add(&format!("IFCCARTESIANPOINT((0.,0.,{}))", real(z)));
        let ax = self.w.add(&format!("IFCAXIS2PLACEMENT3D(#{p},$,$)"));
        self.w.add(&format!("IFCLOCALPLACEMENT({},#{ax})", to.map_or("$".into(), |t| format!("#{t}"))))
    }

    /// A closed IFCPOLYLINE through `pts` (the first point repeated at the end).
    fn polyline(&mut self, pts: &[DVec2]) -> usize {
        let mut ids: Vec<usize> = pts.iter().map(|p| self.w.add(&format!("IFCCARTESIANPOINT(({},{}))", real(p.x), real(p.y)))).collect();
        ids.push(ids[0]);
        self.w.add(&format!("IFCPOLYLINE(({}))", refs(&ids)))
    }

    /// The shape representation `rep` of type `kind` ('SweptSolid', 'Brep') of one item, as IFCPRODUCTDEFINITIONSHAPE.
    fn shape(&mut self, kind: &str, item: usize) -> usize {
        let rep = self.w.add(&format!("IFCSHAPEREPRESENTATION(#{},'Body','{kind}',(#{item}))", self.body));
        self.w.add(&format!("IFCPRODUCTDEFINITIONSHAPE($,$,(#{rep}))"))
    }

    /// IFCPRODUCTDEFINITIONSHAPE of `outer` (with `holes`) in the storey plane, from `z0` up by `depth`.
    fn extrusion(&mut self, outer: &[DVec2], holes: &[Vec<DVec2>], z0: f64, depth: f64) -> usize {
        let outline = self.polyline(outer);
        let profile = if holes.is_empty() {
            self.w.add(&format!("IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,$,#{outline})"))
        } else {
            let inner: Vec<usize> = holes.iter().map(|h| self.polyline(h)).collect();
            self.w.add(&format!("IFCARBITRARYPROFILEDEFWITHVOIDS(.AREA.,$,#{outline},({}))", refs(&inner)))
        };
        let p = self.w.add(&format!("IFCCARTESIANPOINT((0.,0.,{}))", real(z0)));
        let pos = self.w.add(&format!("IFCAXIS2PLACEMENT3D(#{p},$,$)"));
        let solid = self.w.add(&format!("IFCEXTRUDEDAREASOLID(#{profile},#{pos},#{},{})", self.z, real(depth)));
        self.shape("SweptSolid", solid)
    }

    /// IFCPRODUCTDEFINITIONSHAPE of `body` as one IFCFACETEDBREP (one IFCFACE per triangle), moved down by `elevation`.
    fn brep(&mut self, body: &Body, elevation: f64) -> usize {
        let points: Vec<usize> =
            body.mesh.positions.iter().map(|p| self.w.add(&format!("IFCCARTESIANPOINT(({},{},{}))", real(p.x), real(p.y), real(p.z - elevation)))).collect();
        let mut faces = Vec::new();
        for t in body.mesh.indices.as_chunks::<3>().0 {
            let corners: Vec<usize> = t.iter().map(|&i| points[i as usize]).collect();
            let lp = self.w.add(&format!("IFCPOLYLOOP(({}))", refs(&corners)));
            let bound = self.w.add(&format!("IFCFACEOUTERBOUND(#{lp},.T.)"));
            faces.push(self.w.add(&format!("IFCFACE((#{bound}))")));
        }
        let shell = self.w.add(&format!("IFCCLOSEDSHELL(({}))", refs(&faces)));
        let brep = self.w.add(&format!("IFCFACETEDBREP(#{shell})"));
        self.shape("Brep", brep)
    }

    /// IfcPropertySet per `Pset_X` among the `Pset_X.Prop` keys of `meta`, attached to `object`.
    fn psets(&mut self, meta: &BTreeMap<String, String>, object: usize) {
        let mut sets: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
        for (key, value) in meta {
            if key.starts_with("Pset_")
                && let Some((set, prop)) = key.split_once('.')
            {
                sets.entry(set).or_default().push((prop, value));
            }
        }
        for (set, props) in sets {
            let ids: Vec<usize> = props
                .iter()
                .map(|(name, value)| {
                    let v = match *value {
                        "true" => "IFCBOOLEAN(.T.)".to_string(),
                        "false" => "IFCBOOLEAN(.F.)".to_string(),
                        _ => format!("IFCLABEL({})", ifc_str(value)),
                    };
                    self.w.add(&format!("IFCPROPERTYSINGLEVALUE({},$,{v},$)", ifc_str(name)))
                })
                .collect();
            // A property set is a rooted object, not a relationship: five attributes.
            self.rels += 1;
            let g = self.guid("rel", self.rels);
            let pset = self.w.add(&format!("IFCPROPERTYSET({g},$,{},$,({}))", ifc_str(set), refs(&ids)));
            self.rel("IFCRELDEFINESBYPROPERTIES", &format!("(#{object}),#{pset}"));
        }
    }

    /// An IfcElementQuantity `name` of `quantities` (entity ids) attached to `object`.
    fn quantities(&mut self, name: &str, quantities: &[usize], object: usize) {
        self.rels += 1;
        let g = self.guid("rel", self.rels);
        let q = self.w.add(&format!("IFCELEMENTQUANTITY({g},$,{},$,$,({}))", ifc_str(name), refs(quantities)));
        self.rel("IFCRELDEFINESBYPROPERTIES", &format!("(#{object}),#{q}"));
    }

    fn volume(&mut self, body: &Body) -> usize {
        self.w.add(&format!("IFCQUANTITYVOLUME('NetVolume',$,$,{},$)", real(body.volume() / 1e9)))
    }

    /// One storey with the elements and spaces of the level `sketch`; returns the storey's entity.
    fn level(&mut self, r: &RebuildResult, sketch: u32, elevation: f64, building_placement: usize) -> usize {
        let doc = self.doc;
        let name = doc.sketch(sketch).map_or(String::new(), |s| s.name.clone());
        let stpl = self.placement(Some(building_placement), elevation);
        let g = self.guid("storey", u64::from(sketch));
        let storey = self.w.add(&format!("IFCBUILDINGSTOREY({g},$,{},$,$,#{stpl},$,$,.ELEMENT.,{})", ifc_str(&name), real(elevation)));
        // Elements and spaces sit at the storey's origin.
        let local = self.placement(Some(stpl), 0.0);
        let solid = |id: u64| r.walls.iter().find(|(i, _)| *i == id).map(|(_, b)| b.clone());
        let ents: Vec<&Entity> = doc.entities_in(sketch).filter(|e| !e.construction).collect();
        let (mut elements, mut spaces) = (Vec::new(), Vec::new());

        let walls: Vec<(&Entity, Wall)> = ents.iter().filter_map(|e| if let Shape::Wall(w) = &e.shape { Some((*e, *w)) } else { None }).collect();
        let outlines = cad_arch::footprints(&walls.iter().map(|(_, w)| *w).collect::<Vec<_>>());
        let mut written: BTreeMap<u64, (usize, Wall)> = BTreeMap::new();
        for ((e, w), outline) in walls.iter().zip(&outlines) {
            // A wall without a solid is left out, as in the rebuild.
            let Some(body) = solid(e.id) else { continue };
            let shape = self.extrusion(outline, &[], 0.0, w.height);
            let (g, n) = (self.guid("element", e.id), ifc_str(&element_name(e)));
            let wall = self.w.add(&format!("IFCWALL({g},$,{n},$,$,#{local},#{shape},$,.NOTDEFINED.)"));
            self.psets(&e.meta, wall);
            let q = self.volume(&body);
            self.quantities("Qto_WallBaseQuantities", &[q], wall);
            elements.push(wall);
            written.insert(e.id, (wall, *w));
        }

        for e in &ents {
            let Shape::Opening(o) = &e.shape else { continue };
            let Some(&(wall, host)) = written.get(&o.host) else { continue };
            let o = Opening { wall: host, ..*o };
            let (rect, z0, z1) = cad_arch::opening_box(&o);
            let shape = self.extrusion(&rect, &[], z0, z1 - z0);
            let (og, n) = (self.guid("opening", e.id), ifc_str(&format!("{} opening", element_name(e))));
            let opening = self.w.add(&format!("IFCOPENINGELEMENT({og},$,{n},$,$,#{local},#{shape},$,.OPENING.)"));
            self.rel("IFCRELVOIDSELEMENT", &format!("#{wall},#{opening}"));
            let shape = self.extrusion(&o.corners(), &[], o.sill, o.height);
            let (class, kind) = match o.kind {
                OpeningKind::Door => ("IFCDOOR", ".DOOR."),
                OpeningKind::Window => ("IFCWINDOW", ".WINDOW."),
            };
            let (g, n) = (self.guid("element", e.id), ifc_str(&element_name(e)));
            let size = format!("{},{}", real(o.height), real(o.width));
            let el = self.w.add(&format!("{class}({g},$,{n},$,$,#{local},#{shape},$,{size},{kind},.NOTDEFINED.,$)"));
            self.rel("IFCRELFILLSELEMENT", &format!("#{opening},#{el}"));
            self.psets(&e.meta, el);
            elements.push(el);
        }

        for e in &ents {
            let (g, n) = (self.guid("element", e.id), ifc_str(&element_name(e)));
            let el = match &e.shape {
                Shape::Slab(s) => {
                    let shape = self.extrusion(&s.outline, &[], -s.thickness, s.thickness);
                    let slab = self.w.add(&format!("IFCSLAB({g},$,{n},$,$,#{local},#{shape},$,.FLOOR.)"));
                    if let Some(body) = solid(e.id) {
                        let q = self.volume(&body);
                        self.quantities("Qto_SlabBaseQuantities", &[q], slab);
                    }
                    slab
                }
                Shape::Roof(roof) => {
                    let Some(body) = solid(e.id) else { continue };
                    let kind = match roof.kind {
                        RoofKind::Flat => ".FLAT_ROOF.",
                        RoofKind::Shed => ".SHED_ROOF.",
                        RoofKind::Gable => ".GABLE_ROOF.",
                        RoofKind::Hip => ".HIP_ROOF.",
                    };
                    let shape = self.brep(&body, elevation);
                    self.w.add(&format!("IFCROOF({g},$,{n},$,$,#{local},#{shape},$,{kind})"))
                }
                Shape::Stair(_) => {
                    let Some(body) = solid(e.id) else { continue };
                    let shape = self.brep(&body, elevation);
                    self.w.add(&format!("IFCSTAIR({g},$,{n},$,$,#{local},#{shape},$,.STRAIGHT_RUN_STAIR.)"))
                }
                _ => continue,
            };
            self.psets(&e.meta, el);
            elements.push(el);
        }

        for e in &ents {
            let Shape::Room(room) = &e.shape else { continue };
            let Some((_, region)) = r.rooms.iter().find(|(i, _)| *i == e.id) else { continue };
            let shape = match doc.level_height(sketch) {
                Ok(h) => format!("#{}", self.extrusion(&region.outline, &region.islands, 0.0, h)),
                Err(_) => "$".to_string(),
            };
            let g = self.guid("element", e.id);
            let space = self.w.add(&format!(
                "IFCSPACE({g},$,{},$,$,#{local},{shape},{},.ELEMENT.,.INTERNAL.,$)",
                ifc_str(&room.number),
                ifc_str(&room.name)
            ));
            self.psets(&e.meta, space);
            let area = self.w.add(&format!("IFCQUANTITYAREA('NetFloorArea',$,$,{},$)", real(region.area / 1e6)));
            let perimeter = self.w.add(&format!("IFCQUANTITYLENGTH('NetPerimeter',$,$,{},$)", real(region.perimeter)));
            self.quantities("Qto_SpaceBaseQuantities", &[area, perimeter], space);
            spaces.push(space);
        }

        if !elements.is_empty() {
            self.rel("IFCRELCONTAINEDINSPATIALSTRUCTURE", &format!("({}),#{storey}", refs(&elements)));
        }
        if !spaces.is_empty() {
            self.rel("IFCRELAGGREGATES", &format!("#{storey},({})", refs(&spaces)));
        }
        storey
    }
}

/// The entity name, or "<IFC class without Ifc> <id>" when it is empty.
fn element_name(e: &Entity) -> String {
    if e.name.is_empty() { format!("{} {}", cad_arch::ifc_class(&e.shape).map_or("Element", |c| c.trim_start_matches("Ifc")), e.id) } else { e.name.clone() }
}

/// The IFC of the levels of `doc` and the solids and room areas of rebuild `r`; Err when the document has no level.
pub fn to_ifc(doc: &Document, r: &RebuildResult) -> Result<String, String> {
    let levels = doc.levels();
    if levels.is_empty() {
        return Err("no levels to export; IFC holds the architectural model (walls, rooms, floors)".into());
    }
    let header = format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ViewDefinition [ReferenceView_V1.2]'),'2;1');\nFILE_NAME({},'',(''),(''),'ProjectCAD','ProjectCAD','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n",
        ifc_str(&format!("{}.ifc", doc.name))
    );
    let mut ifc = Ifc { w: W { out: header, next: 0 }, doc, rels: 0, z: 0, body: 0 };
    let origin = ifc.w.add("IFCCARTESIANPOINT((0.,0.,0.))");
    ifc.z = ifc.w.add("IFCDIRECTION((0.,0.,1.))");
    let x = ifc.w.add("IFCDIRECTION((1.,0.,0.))");
    let world = ifc.w.add(&format!("IFCAXIS2PLACEMENT3D(#{origin},#{},#{x})", ifc.z));
    let ctx = ifc.w.add(&format!("IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#{world},$)"));
    ifc.body = ifc.w.add(&format!("IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Body','Model',*,*,*,*,#{ctx},$,.MODEL_VIEW.,$)"));
    let units: Vec<usize> = [
        "IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.)",
        "IFCSIUNIT(*,.AREAUNIT.,$,.SQUARE_METRE.)",
        "IFCSIUNIT(*,.VOLUMEUNIT.,$,.CUBIC_METRE.)",
        "IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.)",
    ]
    .iter()
    .map(|u| ifc.w.add(u))
    .collect();
    let units = ifc.w.add(&format!("IFCUNITASSIGNMENT(({}))", refs(&units)));
    let g = ifc.guid("project", 0);
    let project = ifc.w.add(&format!("IFCPROJECT({g},$,{},$,$,$,$,(#{ctx}),#{units})", ifc_str(&doc.name)));
    let spl = ifc.placement(None, 0.0);
    let g = ifc.guid("site", 0);
    let site = ifc.w.add(&format!("IFCSITE({g},$,'Site',$,$,#{spl},$,$,.ELEMENT.,$,$,$,$,$)"));
    let bpl = ifc.placement(Some(spl), 0.0);
    let g = ifc.guid("building", 0);
    let building = ifc.w.add(&format!("IFCBUILDING({g},$,'Building',$,$,#{bpl},$,$,.ELEMENT.,$,$,$)"));
    ifc.rel("IFCRELAGGREGATES", &format!("#{project},(#{site})"));
    ifc.rel("IFCRELAGGREGATES", &format!("#{site},(#{building})"));
    let storeys: Vec<usize> = levels.iter().map(|&(sketch, elevation)| ifc.level(r, sketch, elevation, bpl)).collect();
    ifc.rel("IFCRELAGGREGATES", &format!("#{building},({})", refs(&storeys)));
    ifc.w.out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    Ok(ifc.w.out)
}

#[cfg(test)]
#[path = "ifc_out_tests.rs"]
mod tests;
